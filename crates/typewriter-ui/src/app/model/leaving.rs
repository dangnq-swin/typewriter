//! Putting the project away: asking first when work would be lost, then
//! quitting, or putting a new or opened project in.

use std::path::PathBuf;

use typewriter_core::Typewriter;

use super::{Model, Overlays, Project, View};
use crate::app::intent::Effect;
use crate::filing::{self, Filing};
use crate::picker::{Dialog, Picked};

/// Where to go once the project is dealt with.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Leaving {
    Quit,
    New,
    Open(PathBuf),
}

/// The leave dialog's answers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Answer {
    SaveAs,
    Save,
    Keep,
    Discard,
    DontSave,
    Cancel,
}

impl Model {
    /// Work would be put away unsaved: a draft with work, or changes with
    /// autosave off.
    fn must_ask(&self) -> bool {
        let filing = &self.project.filing;
        filing.is_draft_with_work(&self.project.machine)
            || (!self.settings.saving.autosave && filing.is_saved() && filing.has_unsaved_changes())
    }

    /// Leaves the project, asking first if needed.
    pub(super) fn leave(&mut self, leaving: Leaving, now: f64) {
        if self.must_ask() {
            self.leaving = Some(leaving);
        } else {
            self.go(leaving, now);
        }
    }

    fn go(&mut self, leaving: Leaving, now: f64) {
        match leaving {
            Leaving::Quit => self.effects.push(Effect::Close),
            Leaving::New => {
                let profile = self.machines.for_new(&self.settings.machine.profile);
                // The correction method in hand carries over.
                let erase = self.project.machine.constraints.erase;
                let constraints = self.settings.machine.rules.constraints(erase);
                match Typewriter::new(profile, constraints) {
                    Ok(machine) => self.put_in(machine, Filing::draft(), false, now),
                    Err(err) => self.notice.show(format!("No new project: {err}"), now),
                }
            }
            Leaving::Open(path) => match filing::open(&self.machines, &path) {
                Ok(machine) => self.put_in(machine, Filing::at(path), true, now),
                Err(err) => self
                    .notice
                    .show(format!("Could not open that project: {err:#}"), now),
            },
        }
    }

    /// Swaps in another project and winds its sheet in. Saves the old one
    /// first; anything needing a question was asked before.
    fn put_in(&mut self, mut machine: Typewriter, filing: Filing, reopened: bool, now: f64) {
        self.keep(now);
        let wind_back = reopened.then(|| machine.reinsert());
        // New session, same goal.
        let goal = self.project.session.goal();
        self.project = Project::new(machine, filing, goal);
        self.project.filing.remember();
        self.feed.restart(wind_back);
        self.overlays = Overlays::default();
        self.view = View::Typing;
        self.selected = 0;
        self.effects.push(Effect::MachineChanged);
    }

    /// The leave dialog's answer.
    pub(super) fn answer(&mut self, answer: Answer, now: f64) {
        let Some(leaving) = self.leaving.take() else {
            return;
        };
        match answer {
            Answer::SaveAs => {
                self.leaving_after_save_as = Some(leaving);
                self.effects.push(Effect::Ask(Dialog::SaveAs));
            }
            Answer::Save => {
                self.project.filing.changed(now);
                // Stay if it failed: the notice says why.
                match self.project.filing.save(&self.project.machine, now) {
                    Ok(()) => self.go(leaving, now),
                    Err(err) => self.notice.show(err, now),
                }
            }
            Answer::Keep | Answer::DontSave => self.go(leaving, now),
            Answer::Discard => {
                self.project.filing.discard_draft();
                self.go(leaving, now);
            }
            Answer::Cancel => {}
        }
    }

    /// A file dialog's answer.
    pub(super) fn picked(&mut self, picked: Picked, now: f64) {
        match picked {
            Picked::SaveAs(path) => {
                let saved = self
                    .project
                    .filing
                    .save_as(&self.project.machine, path, now);
                let (Ok(told) | Err(told)) = &saved;
                self.notice.show(told.clone(), now);
                if let Some(leaving) = self.leaving_after_save_as.take()
                    && saved.is_ok()
                {
                    self.go(leaving, now);
                }
            }
            Picked::Open(path) => self.leave(Leaving::Open(path), now),
            Picked::Texture(path) => self.texture_picked(path),
            Picked::Cancelled => self.leaving_after_save_as = None,
        }
    }

    /// A project handed in from outside, unless it is the one in already.
    pub(super) fn open_file(&mut self, path: PathBuf, now: f64) {
        if !self.project.filing.is_at(&path) {
            self.leave(Leaving::Open(path), now);
        }
    }

    /// Closing puts the project away too: holds the window back to ask, if
    /// needed.
    pub(super) fn close_window(&mut self) {
        if !self.must_ask() {
            return;
        }
        self.effects.push(Effect::CancelClose);
        if self.leaving.is_none() && self.leaving_after_save_as.is_none() {
            self.leaving = Some(Leaving::Quit);
        }
    }

    /// On quit: writes, pause or not, unless the user opted out (autosave
    /// off, or a discarded draft). `Err`: why it failed.
    pub fn put_away(&mut self) -> Result<(), String> {
        let autosave = self.settings.saving.autosave;
        let filing = &mut self.project.filing;
        if autosave || !filing.is_saved() {
            filing.changed(0.0);
        }
        filing.keep(&self.project.machine, 0.0, autosave)
    }
}

// Tests never type into a project that New put in: its draft is a real
// path in the data folder, and would be written.
#[cfg(test)]
mod tests {
    use super::super::testing::{model, type_text};
    use super::*;
    use crate::app::intent::Intent;
    use crate::render::folder::FolderAction;

    fn is_blank(model: &Model) -> bool {
        model.project.machine.page().is_blank()
    }

    #[test]
    fn a_project_from_the_file_manager_asks_first_like_open() {
        let mut model = model();
        type_text(&mut model, "unsaved", 10.0);
        let path = PathBuf::from("/nowhere/novel.typr");
        model.update(Intent::OpenFile(path.clone()), 11.0);
        assert_eq!(model.leaving, Some(Leaving::Open(path)));
    }

    #[test]
    fn the_project_already_in_is_not_opened_again() {
        let mut model = model();
        let path = PathBuf::from("/nowhere/novel.typr");
        model.project.filing = Filing::at(path.clone());
        type_text(&mut model, "kept", 10.0);
        model.update(Intent::OpenFile(path), 11.0);
        assert!(model.leaving.is_none());
        assert!(!is_blank(&model), "still the same sheet");
    }

    #[test]
    fn an_untouched_draft_is_left_without_asking() {
        let mut model = model();
        model.update(Intent::Folder(FolderAction::New), 10.0);
        assert_eq!(model.leaving, None);
        assert!(model.take_effects().contains(&Effect::MachineChanged));
    }

    #[test]
    fn a_draft_with_work_is_not_put_away_without_asking() {
        let mut model = model();
        type_text(&mut model, "chapter one", 10.0);
        model.update(Intent::Folder(FolderAction::New), 20.0);
        assert_eq!(model.leaving, Some(Leaving::New));
        assert!(model.keys_to_fields());
        model.update(Intent::Leave(Answer::Cancel), 21.0);
        assert!(model.leaving.is_none() && !is_blank(&model));

        model.update(Intent::Folder(FolderAction::New), 22.0);
        model.update(Intent::Leave(Answer::Discard), 23.0);
        assert!(is_blank(&model));
        assert_eq!(model.view, View::Typing);
        model.start_frame(24.0);
        assert!(model.is_busy(24.5), "its sheet winds in");
    }

    #[test]
    fn closing_the_window_asks_about_a_draft_with_work_first() {
        let mut model = model();
        model.update(Intent::CloseWindow, 10.0);
        assert!(model.take_effects().is_empty(), "nothing to ask: it closes");
        type_text(&mut model, "notes", 11.0);
        model.take_effects();
        model.update(Intent::CloseWindow, 12.0);
        assert_eq!(model.take_effects(), [Effect::CancelClose]);
        assert_eq!(model.leaving, Some(Leaving::Quit));
        model.update(Intent::Leave(Answer::Keep), 13.0);
        assert_eq!(model.take_effects(), [Effect::Close]);
    }

    #[test]
    fn save_as_asks_where_and_leaves_once_saved() {
        let mut model = model();
        type_text(&mut model, "notes", 11.0);
        model.update(Intent::CloseWindow, 12.0);
        model.take_effects();
        model.update(Intent::Leave(Answer::SaveAs), 13.0);
        assert_eq!(model.take_effects(), [Effect::Ask(Dialog::SaveAs)]);
        model.update(Intent::Picked(Picked::Cancelled), 14.0);
        assert!(model.take_effects().is_empty(), "cancelled: stays open");
    }
}
