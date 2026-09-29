//! Keeps the project saved: autosave, Save / Save As / Rename, Open and
//! Export. One project = one `*.folder.ron` file.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, TryRecvError};

use anyhow::Context as _;
use eframe::egui::{self, Color32, CornerRadius, FontId, Id, LayerId, Order, Rect, vec2};
use typewriter_core::{Typewriter, export};

use crate::machines::Machines;
use crate::render::{calendar, pdf};
use crate::storage;

/// Autosave after this pause in typing.
const AUTOSAVE_AFTER_SECONDS: f64 = 2.0;
const NOTICE_SECONDS: f64 = 4.0;
/// Hold the dot amber this long after a write, so it shows at all.
const WRITING_SECONDS: f64 = 0.4;
const NOTICE_FADE_SECONDS: f64 = 0.5;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExportFormat {
    Markdown,
    Text,
    Pdf,
}

/// Which desktop dialog is open.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Dialog {
    SaveAs,
    Open,
}

/// A desktop dialog's answer.
pub enum Picked {
    SaveAs(PathBuf),
    Open(PathBuf),
    Cancelled,
}

/// How the project is kept. Drives the Autosave plate.
#[derive(Debug, Clone, PartialEq)]
pub enum Keeping {
    /// Written to its file after each pause.
    Autosave(WriteStatus),
    /// Unnamed: cached in the drafts folder.
    Draft,
    /// Written only by Save.
    Off { unsaved: bool },
}

#[derive(Debug, Clone, PartialEq)]
pub enum WriteStatus {
    Saved,
    Writing,
    Failed(String),
}

pub struct Filing {
    /// The project's file or draft. `None` only without a data directory.
    path: Option<PathBuf>,
    /// First unsaved change.
    changed_at: Option<f64>,
    dialog: Option<(Dialog, Receiver<Option<PathBuf>>)>,
    notice: Option<(String, f64)>,
    /// (when, error if it failed)
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

    /// Where it is saved, for the folder view.
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

    /// A file exists, draft or not: it was opened, not started new.
    pub fn is_saved_somewhere(&self) -> bool {
        self.path.as_deref().is_some_and(Path::exists)
    }

    /// Saved under its own name, not a draft.
    pub fn is_saved(&self) -> bool {
        self.path.as_deref().is_some_and(|p| !storage::is_draft(p))
    }

    /// A draft with something typed: ask before putting it away.
    pub fn is_draft_with_work(&self, machine: &Typewriter) -> bool {
        !self.is_saved() && !is_untouched(machine)
    }

    /// Changes not yet written to the project's file.
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

    /// Deletes the draft file. Nothing is written afterwards.
    pub fn discard_draft(&mut self) {
        if let Some(path) = self.path.take().filter(|p| storage::is_draft(p)) {
            let _ = fs::remove_file(path);
        }
        self.changed_at = None;
    }

    pub fn changed(&mut self, now: f64) {
        self.changed_at.get_or_insert(now);
    }

    /// Writes after a pause in typing. With autosave off, still writes drafts:
    /// a crash must lose nothing.
    pub fn autosave(&mut self, machine: &Typewriter, now: f64, autosave: bool) {
        if self
            .changed_at
            .is_some_and(|at| now - at >= AUTOSAVE_AFTER_SECONDS)
        {
            self.keep(machine, now, autosave);
        }
    }

    /// Writes now if autosave would. Call on a sheet feed and on quit.
    pub fn keep(&mut self, machine: &Typewriter, now: f64, autosave: bool) {
        if autosave || !self.is_saved() {
            self.save(machine, now);
        }
    }

    /// Writes if anything changed. Skips untouched drafts so they don't
    /// pile up. False if the write failed.
    pub fn save(&mut self, machine: &Typewriter, now: f64) -> bool {
        if self.changed_at.is_none() {
            return true;
        }
        let Some(path) = self.path.clone() else {
            return true;
        };
        // On failure too: retry after the next change.
        self.changed_at = None;
        if is_untouched(machine) && storage::is_draft(&path) && !path.exists() {
            return true;
        }
        let error = write_project(machine, &path)
            .err()
            .map(|err| format!("{err:#}"));
        if let Some(err) = &error {
            self.notify(format!("Could not save the project: {err}"), now);
        }
        let saved = error.is_none();
        self.last_write = Some((now, error));
        saved
    }

    /// The Save command: writes now, or Save As for a draft.
    pub fn save_now(&mut self, machine: &Typewriter, ctx: &egui::Context, now: f64) {
        if !self.is_saved() {
            self.ask(Dialog::SaveAs, ctx);
            return;
        }
        self.changed(now);
        if self.save(machine, now) {
            self.notify("Saved".to_owned(), now);
        }
    }

    /// Saves under a chosen path; a draft moves there. True if saved.
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
            // Safe: only a draft, and the project is now written elsewhere.
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

    /// Opens the desktop dialog on a thread, so the window keeps drawing.
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

    /// The dialog's answer, once it has closed.
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

    /// Takes the notice down early, if it still says `text`.
    pub fn withdraw(&mut self, text: &str) {
        if self.notice.as_ref().is_some_and(|(shown, _)| shown == text) {
            self.notice = None;
        }
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

    /// A short fading note at the bottom of the window.
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

/// Nothing typed yet.
fn is_untouched(machine: &Typewriter) -> bool {
    let document = machine.document();
    document.finished().is_empty()
        && document.current().is_blank()
        && document.scratchpad().is_fresh()
}

/// Reads a project into the machine it was typed on.
pub fn open(machines: &Machines, path: &Path) -> anyhow::Result<Typewriter> {
    let text = fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
    let machine = |name: &str| machines.find(name);
    Ok(Typewriter::from_folder_ron(
        &text,
        machine,
        calendar::local_day,
    )?)
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
    fn a_notice_is_withdrawn_only_while_it_still_says_so() {
        let mut filing = Filing::draft();
        filing.notify("Jammed".into(), 0.0);
        filing.withdraw("Saved");
        assert!(filing.notice.is_some());
        filing.withdraw("Jammed");
        assert!(filing.notice.is_none());
    }

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
