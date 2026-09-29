//! Keeping the project in the machine saved: autosave, Save / Save As /
//! Rename, opening another project and exporting it.
//!
//! A project is one folder file (`*.folder.ron`), shown as the manila
//! folder.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, TryRecvError};

use anyhow::Context as _;
use eframe::egui::{self, Color32, CornerRadius, FontId, Id, LayerId, Order, Rect, vec2};
use typewriter_core::{Typewriter, export};

use crate::machines::Machines;
use crate::render::pdf;
use crate::storage;

/// Saved once typing has paused this long.
const AUTOSAVE_AFTER_SECONDS: f64 = 2.0;
const NOTICE_SECONDS: f64 = 4.0;
/// How long the status dot shows a write, so it can be seen at all.
const WRITING_SECONDS: f64 = 0.4;
const NOTICE_FADE_SECONDS: f64 = 0.5;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExportFormat {
    Markdown,
    Text,
    /// The sheets as they look typed.
    Pdf,
}

/// Which desktop dialog is open.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Dialog {
    SaveAs,
    Open,
}

/// A path chosen in a desktop dialog, or the dialog closed without one.
pub enum Picked {
    SaveAs(PathBuf),
    Open(PathBuf),
    Cancelled,
}

/// How the project is being kept, for the Autosave plate.
#[derive(Debug, Clone, PartialEq)]
pub enum Keeping {
    /// Written to its own file after each pause, with how the last write
    /// went.
    Autosave(WriteStatus),
    /// Not yet saved under a name: cached in the drafts folder.
    Draft,
    /// Autosave is off: written only by Save.
    Off { unsaved: bool },
}

#[derive(Debug, Clone, PartialEq)]
pub enum WriteStatus {
    Saved,
    Writing,
    Failed(String),
}

pub struct Filing {
    /// The project's file, or its draft until it is saved under a name.
    /// `None` only when there is no data directory to keep a draft in.
    path: Option<PathBuf>,
    /// When the machine last changed without being saved.
    changed_at: Option<f64>,
    dialog: Option<(Dialog, Receiver<Option<PathBuf>>)>,
    notice: Option<(String, f64)>,
    /// When the project was last written, and why that failed if it did.
    last_write: Option<(f64, Option<String>)>,
}

impl Filing {
    /// A new, unsaved project.
    pub fn draft() -> Self {
        Self::with_path(storage::new_draft_path())
    }

    /// A project opened from `path`.
    pub fn at(path: PathBuf) -> Self {
        Self::with_path(Some(path))
    }

    fn with_path(path: Option<PathBuf>) -> Self {
        Self {
            path,
            changed_at: None,
            dialog: None,
            notice: None,
            last_write: None,
        }
    }

    /// The name on the folder's tab.
    pub fn name(&self) -> String {
        self.path
            .as_deref()
            .map_or_else(|| storage::UNTITLED.to_owned(), storage::display_name)
    }

    /// Where the project is saved, for the folder view.
    pub fn location(&self) -> String {
        match self.path.as_deref() {
            Some(path) if !storage::is_draft(path) => storage::home_relative(path),
            Some(path) => format!(
                "Unsaved draft, kept in {}",
                path.parent()
                    .map_or_else(String::new, storage::home_relative)
            ),
            None => "Unsaved: no data folder to keep a draft in".to_owned(),
        }
    }

    /// There is a file for the project, draft or not: it was opened, not
    /// started new.
    pub fn is_saved_somewhere(&self) -> bool {
        self.path.as_deref().is_some_and(Path::exists)
    }

    /// Saved under a name of its own, not a draft.
    pub fn is_saved(&self) -> bool {
        self.path.as_deref().is_some_and(|p| !storage::is_draft(p))
    }

    /// A draft with something typed in it: worth asking about before it
    /// is put away.
    pub fn is_draft_with_work(&self, machine: &Typewriter) -> bool {
        !self.is_saved() && !is_untouched(machine)
    }

    /// Changes not written to the project's own file yet.
    pub fn has_unsaved_changes(&self) -> bool {
        self.changed_at.is_some()
    }

    pub fn keeping(&self, autosave: bool, now: f64) -> Keeping {
        if !self.is_saved() {
            return Keeping::Draft;
        }
        if !autosave {
            return Keeping::Off {
                unsaved: self.has_unsaved_changes(),
            };
        }
        Keeping::Autosave(match &self.last_write {
            Some((_, Some(err))) => WriteStatus::Failed(err.clone()),
            Some((at, None)) if now - at < WRITING_SECONDS => WriteStatus::Writing,
            _ => WriteStatus::Saved,
        })
    }

    /// Throws the draft away, file and all. Nothing is written afterwards.
    pub fn discard_draft(&mut self) {
        if let Some(path) = self.path.take().filter(|p| storage::is_draft(p)) {
            let _ = fs::remove_file(path);
        }
        self.changed_at = None;
    }

    pub fn changed(&mut self, now: f64) {
        self.changed_at.get_or_insert(now);
    }

    /// Writes after a pause in typing. With autosave off, only a draft is
    /// written (to the drafts folder), so a crash still loses nothing.
    pub fn autosave(&mut self, machine: &Typewriter, now: f64, autosave: bool) {
        if self
            .changed_at
            .is_some_and(|at| now - at >= AUTOSAVE_AFTER_SECONDS)
        {
            self.keep(machine, now, autosave);
        }
    }

    /// Writes now if autosave would (e.g. on a sheet feed or on quit).
    pub fn keep(&mut self, machine: &Typewriter, now: f64, autosave: bool) {
        if autosave || !self.is_saved() {
            self.save(machine, now);
        }
    }

    /// Writes the project if anything changed. A draft is not written until
    /// something has been typed, so drafts do not pile up.
    pub fn save(&mut self, machine: &Typewriter, now: f64) {
        if self.changed_at.is_none() {
            return;
        }
        let Some(path) = self.path.clone() else {
            return;
        };
        if is_untouched(machine) && storage::is_draft(&path) && !path.exists() {
            self.changed_at = None;
            return;
        }
        // Tried again after the next change if it fails.
        self.changed_at = None;
        let written = write_project(machine, &path);
        self.last_write = Some((now, written.as_ref().err().map(|err| format!("{err:#}"))));
        if let Err(err) = written {
            self.notify(format!("Could not save the project: {err:#}"), now);
        }
    }

    /// Save, from the menu: writes now, or asks for a name and place if the
    /// project is still a draft.
    pub fn save_now(&mut self, machine: &Typewriter, ctx: &egui::Context, now: f64) {
        if !self.is_saved() {
            self.ask(Dialog::SaveAs, ctx);
            return;
        }
        self.changed(now);
        self.save(machine, now);
        if self.changed_at.is_none() {
            self.notify("Saved".to_owned(), now);
        }
    }

    /// Saves the project under a chosen name and place. A draft moves there.
    /// Returns whether it was saved.
    pub fn save_as(&mut self, machine: &Typewriter, chosen: PathBuf, now: f64) -> bool {
        let path = storage::with_extension(chosen);
        if let Err(err) = write_project(machine, &path) {
            self.notify(format!("Could not save the project: {err:#}"), now);
            return false;
        }
        self.last_write = Some((now, None));
        if let Some(old) = self.path.replace(path.clone())
            && storage::is_draft(&old)
            && old != path
        {
            // Only a draft: the project is safe in its new place.
            let _ = fs::remove_file(old);
        }
        self.changed_at = None;
        self.notify(format!("Saved as {}", storage::home_relative(&path)), now);
        true
    }

    /// Renames the project's file where it is.
    pub fn rename(&mut self, machine: &Typewriter, name: &str, now: f64) {
        let Some(old) = self.path.clone().filter(|_| self.is_saved()) else {
            return;
        };
        if name.is_empty() || name.contains(['/', '\0']) {
            self.notify("A name cannot be empty or contain a slash.".to_owned(), now);
            return;
        }
        let path = old.with_file_name(format!("{name}{}", storage::EXTENSION));
        if path == old {
            return;
        }
        if path.exists() {
            let taken = storage::home_relative(&path);
            self.notify(format!("{taken} already exists."), now);
            return;
        }
        let renamed = write_project(machine, &old)
            .and_then(|()| fs::rename(&old, &path).context("renaming the file"))
            .and_then(|()| storage::remember_last(&path).context("remembering it"));
        match renamed {
            Ok(()) => {
                self.path = Some(path);
                self.changed_at = None;
                self.notify(format!("Renamed to {name}"), now);
            }
            Err(err) => self.notify(format!("Could not rename the project: {err:#}"), now),
        }
    }

    pub fn export(
        &mut self,
        machine: &Typewriter,
        format: ExportFormat,
        ink_realism: bool,
        now: f64,
    ) {
        let Some(project) = self.path.clone().filter(|_| self.is_saved()) else {
            self.notify("Save the project first: exports go beside it.".into(), now);
            return;
        };
        let document = machine.document();
        let (extension, contents) = match format {
            ExportFormat::Markdown => ("md", Ok(export::markdown(document).into_bytes())),
            ExportFormat::Text => ("txt", Ok(export::plain_text(document).into_bytes())),
            ExportFormat::Pdf => (
                "pdf",
                pdf::render(machine.profile(), document, &self.name(), ink_realism),
            ),
        };
        let path = storage::export_path(&project, extension);
        let written = contents
            .and_then(|bytes| storage::write_atomic(&path, &bytes).context("writing the file"));
        match written {
            Ok(()) => self.notify(
                format!("Exported to {}", storage::home_relative(&path)),
                now,
            ),
            Err(err) => self.notify(format!("Could not export: {err:#}"), now),
        }
    }

    pub fn ask_save_as(&mut self, ctx: &egui::Context) {
        self.ask(Dialog::SaveAs, ctx);
    }

    pub fn ask_open(&mut self, ctx: &egui::Context) {
        self.ask(Dialog::Open, ctx);
    }

    /// Opens the desktop's own dialog without holding up the window.
    fn ask(&mut self, dialog: Dialog, ctx: &egui::Context) {
        if self.dialog.is_some() {
            return;
        }
        let (sender, receiver) = mpsc::channel();
        let directory = self
            .path
            .as_deref()
            .filter(|p| !storage::is_draft(p))
            .and_then(Path::parent)
            .map(Path::to_path_buf);
        let file_name = format!("{}{}", self.name(), storage::EXTENSION);
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            let mut picker = rfd::FileDialog::new().add_filter("Typewriter project", &["ron"]);
            if let Some(directory) = directory {
                picker = picker.set_directory(directory);
            }
            let picked = match dialog {
                Dialog::SaveAs => picker
                    .set_title("Save the project as")
                    .set_file_name(file_name)
                    .save_file(),
                Dialog::Open => picker.set_title("Open a project").pick_file(),
            };
            let _ = sender.send(picked);
            ctx.request_repaint();
        });
        self.dialog = Some((dialog, receiver));
    }

    /// What was chosen in the dialog, once it has closed.
    pub fn picked(&mut self) -> Option<Picked> {
        let (dialog, receiver) = self.dialog.as_ref()?;
        let picked = match receiver.try_recv() {
            Err(TryRecvError::Empty) => return None,
            Ok(picked) => picked,
            Err(TryRecvError::Disconnected) => None,
        };
        let dialog = *dialog;
        self.dialog = None;
        Some(match (picked, dialog) {
            (Some(path), Dialog::SaveAs) => Picked::SaveAs(path),
            (Some(path), Dialog::Open) => Picked::Open(path),
            (None, _) => Picked::Cancelled,
        })
    }

    /// The project to reopen next time.
    pub fn remember(&self) {
        if let Some(path) = self.path.as_deref().filter(|p| p.exists())
            && let Err(err) = storage::remember_last(path)
        {
            eprintln!("could not remember the last project: {err}");
        }
    }

    pub fn notify(&mut self, text: String, now: f64) {
        self.notice = Some((text, now));
    }

    pub fn is_animating(&self, now: f64) -> bool {
        self.dialog.is_some()
            || self
                .last_write
                .as_ref()
                .is_some_and(|(at, _)| now - at < WRITING_SECONDS)
            || self
                .notice
                .as_ref()
                .is_some_and(|(_, at)| now - at < NOTICE_SECONDS)
    }

    /// A short note at the bottom of the window, fading out.
    pub fn paint_notice(&self, ctx: &egui::Context, view: Rect, now: f64) {
        let Some((text, at)) = &self.notice else {
            return;
        };
        let age = now - at;
        if age >= NOTICE_SECONDS {
            return;
        }
        let opacity = ((NOTICE_SECONDS - age) / NOTICE_FADE_SECONDS).min(1.0) as f32;
        let painter = ctx.layer_painter(LayerId::new(Order::Foreground, Id::new("notice")));
        let galley = painter.layout(
            text.clone(),
            FontId::proportional(13.0),
            Color32::from_rgb(0xEE, 0xE8, 0xDC).gamma_multiply(opacity),
            view.width() - 80.0,
        );
        let size = galley.size() + vec2(24.0, 14.0);
        let rect =
            Rect::from_center_size(view.center_bottom() - vec2(0.0, 90.0 + size.y / 2.0), size);
        painter.rect_filled(
            rect,
            CornerRadius::same(4),
            Color32::from_rgba_unmultiplied(0x2A, 0x26, 0x22, 0xE0).gamma_multiply(opacity),
        );
        painter.galley(rect.min + vec2(12.0, 7.0), galley, Color32::WHITE);
    }
}

/// Nothing typed yet: no finished sheets and a blank one in the machine.
fn is_untouched(machine: &Typewriter) -> bool {
    let document = machine.document();
    document.finished().is_empty() && document.current().is_blank()
}

/// Reads a project file into the machine it was typed on.
pub fn open(machines: &Machines, path: &Path) -> anyhow::Result<Typewriter> {
    let text = fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
    Ok(Typewriter::from_folder_ron(&text, |name| {
        machines.find(name)
    })?)
}

fn write_project(machine: &Typewriter, path: &Path) -> anyhow::Result<()> {
    let text = machine.to_folder_ron()?;
    storage::write_atomic(path, &text).with_context(|| format!("writing {}", path.display()))?;
    storage::remember_last(path).context("remembering it for next time")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_autosave_plate_follows_the_last_write() {
        let mut filing = Filing::at(PathBuf::from("/nowhere/novel.folder.ron"));
        assert_eq!(filing.keeping(false, 0.0), Keeping::Off { unsaved: false });
        filing.changed(1.0);
        assert_eq!(filing.keeping(false, 1.0), Keeping::Off { unsaved: true });
        assert_eq!(
            filing.keeping(true, 1.0),
            Keeping::Autosave(WriteStatus::Saved)
        );
        filing.last_write = Some((5.0, None));
        assert_eq!(
            filing.keeping(true, 5.1),
            Keeping::Autosave(WriteStatus::Writing)
        );
        assert_eq!(
            filing.keeping(true, 6.0),
            Keeping::Autosave(WriteStatus::Saved)
        );
        filing.last_write = Some((7.0, Some("disk full".into())));
        assert_eq!(
            filing.keeping(true, 9.0),
            Keeping::Autosave(WriteStatus::Failed("disk full".into()))
        );
    }
}
