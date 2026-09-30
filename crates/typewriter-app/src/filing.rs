//! Keeps the project saved: autosave, Save / Save As / Rename, Open and
//! Export. One project = one `*.typr` file. Reports what the user
//! should hear of it; the app shows it.

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::Context as _;
use typewriter_core::{Typewriter, export};

use crate::machines::Machines;
use crate::render::pdf;
use crate::storage;

/// Autosave after this pause in typing.
const AUTOSAVE_AFTER_SECONDS: f64 = 2.0;
/// Hold the dot amber this long after a write, so it shows at all.
const WRITING_SECONDS: f64 = 0.4;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExportFormat {
    Markdown,
    Text,
    Pdf,
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

    /// A draft with nowhere to keep it: never written.
    #[cfg(test)]
    pub fn nowhere() -> Self {
        Self::with_path(None)
    }

    fn with_path(path: Option<PathBuf>) -> Self {
        Self {
            path,
            changed_at: None,
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
    /// The project's file is `path`, however either is spelled.
    pub fn is_at(&self, path: &Path) -> bool {
        let same = |a: &Path, b: &Path| match (fs::canonicalize(a), fs::canonicalize(b)) {
            (Ok(a), Ok(b)) => a == b,
            _ => a == b,
        };
        self.path.as_deref().is_some_and(|own| same(own, path))
    }

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
    /// a crash must lose nothing. `Err`: why it failed.
    pub fn autosave(
        &mut self,
        machine: &Typewriter,
        now: f64,
        autosave: bool,
    ) -> Result<(), String> {
        if self
            .changed_at
            .is_some_and(|at| now - at >= AUTOSAVE_AFTER_SECONDS)
        {
            self.keep(machine, now, autosave)?;
        }
        Ok(())
    }

    /// Writes now if autosave would. Call on a sheet feed and on quit.
    pub fn keep(&mut self, machine: &Typewriter, now: f64, autosave: bool) -> Result<(), String> {
        if autosave || !self.is_saved() {
            self.save(machine, now)?;
        }
        Ok(())
    }

    /// Writes if anything changed. Skips untouched drafts so they don't
    /// pile up. `Err`: why the write failed.
    pub fn save(&mut self, machine: &Typewriter, now: f64) -> Result<(), String> {
        if self.changed_at.is_none() {
            return Ok(());
        }
        let Some(path) = self.path.clone() else {
            return Ok(());
        };
        // On failure too: retry after the next change.
        self.changed_at = None;
        if is_untouched(machine) && storage::is_draft(&path) && !path.exists() {
            return Ok(());
        }
        let error = write_project(machine, &path)
            .err()
            .map(|err| format!("{err:#}"));
        self.last_write = Some((now, error.clone()));
        match error {
            Some(err) => Err(format!("Could not save the project: {err}")),
            None => Ok(()),
        }
    }

    /// The Save command for a saved project: writes now. What to tell.
    pub fn save_now(&mut self, machine: &Typewriter, now: f64) -> String {
        self.changed(now);
        match self.save(machine, now) {
            Ok(()) => "Saved".to_owned(),
            Err(err) => err,
        }
    }

    /// Saves under a chosen path; a draft moves there. What to tell either
    /// way; `Ok` if saved.
    pub fn save_as(
        &mut self,
        machine: &Typewriter,
        chosen: PathBuf,
        now: f64,
    ) -> Result<String, String> {
        let path = storage::with_extension(chosen);
        if let Err(err) = write_project(machine, &path) {
            return Err(format!("Could not save the project: {err:#}"));
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
        Ok(format!("Saved as {}", storage::home_relative(&path)))
    }

    /// Renames the project's file where it is. What to tell, if anything.
    pub fn rename(&mut self, machine: &Typewriter, name: &str) -> Option<String> {
        let old = self.path.clone().filter(|_| self.is_saved())?;
        if name.is_empty() || name.contains(['/', '\0']) {
            return Some("A name cannot be empty or contain a slash.".to_owned());
        }
        let path = old.with_file_name(format!("{name}{}", storage::EXTENSION));
        if path == old {
            return None;
        }
        if path.exists() {
            let taken = storage::home_relative(&path);
            return Some(format!("{taken} already exists."));
        }
        let renamed = write_project(machine, &old)
            .and_then(|()| fs::rename(&old, &path).context("renaming the file"))
            .and_then(|()| storage::remember_last(&path).context("remembering it"));
        Some(match renamed {
            Ok(()) => {
                self.path = Some(path);
                self.changed_at = None;
                format!("Renamed to {name}")
            }
            Err(err) => format!("Could not rename the project: {err:#}"),
        })
    }

    /// Writes an export beside the project. What to tell.
    pub fn export(&self, machine: &Typewriter, format: ExportFormat, ink_realism: bool) -> String {
        let Some(project) = self.path.clone().filter(|_| self.is_saved()) else {
            return "Save the project first: exports go beside it.".into();
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
            Ok(()) => format!("Exported to {}", storage::home_relative(&path)),
            Err(err) => format!("Could not export: {err:#}"),
        }
    }

    /// Where a file dialog opens: beside the project, unless it is a draft.
    pub fn dialog_folder(&self) -> Option<PathBuf> {
        self.path
            .as_deref()
            .filter(|p| !storage::is_draft(p))
            .and_then(Path::parent)
            .map(Path::to_path_buf)
    }

    /// The project's file name, for Save As to suggest.
    pub fn file_name(&self) -> String {
        format!("{}{}", self.name(), storage::EXTENSION)
    }

    /// The project to reopen next time.
    pub fn remember(&self) {
        if let Some(path) = self.path.as_deref().filter(|p| p.exists())
            && let Err(err) = storage::remember_last(path)
        {
            eprintln!("could not remember the last project: {err}");
        }
    }

    pub fn is_animating(&self, now: f64) -> bool {
        self.last_write
            .as_ref()
            .is_some_and(|(at, _)| now - at < WRITING_SECONDS)
    }
}

/// Nothing typed yet.
fn is_untouched(machine: &Typewriter) -> bool {
    let document = machine.document();
    document.finished().is_empty()
        && document.current().is_blank()
        && document.notebook().is_fresh()
}

/// Reads a project into the machine it was typed on.
pub fn open(machines: &Machines, path: &Path) -> anyhow::Result<Typewriter> {
    let text = fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
    let machine = |name: &str| machines.find(name);
    Ok(Typewriter::from_folder_ron(&text, machine)?)
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
        let mut filing = Filing::at(PathBuf::from("/nowhere/novel.typr"));
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
