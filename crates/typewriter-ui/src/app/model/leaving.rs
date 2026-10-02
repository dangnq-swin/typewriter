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

// The test desktop (see `storage::test_desktop`) stands in for the data
// folder, so even a project New puts in — whose draft is a real path —
// could be typed into and written without touching anyone's home.
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

    #[test]
    fn an_answer_without_a_question_does_nothing() {
        let mut model = model();
        model.update(Intent::Leave(Answer::Save), 10.0);
        assert!(model.take_effects().is_empty());
        assert!(!model.notice.is_animating(10.0));
    }

    #[test]
    fn a_project_from_the_desktop_reopens_whole() {
        let path = crate::storage::test_desktop().join("leave-reopen/novel.typr");
        let mut writer = model();
        writer.project.filing = Filing::at(path.clone());
        type_text(&mut writer, "reopened", 10.0);
        writer.put_away().unwrap();
        assert!(path.exists());
        let mut model = model();
        type_text(&mut model, "unsaved work", 20.0);
        model.update(Intent::OpenFile(path.clone()), 21.0);
        assert!(model.leaving.is_some(), "asks about the work in hand");
        model.take_effects();
        model.update(Intent::Leave(Answer::DontSave), 22.0);
        assert_eq!(model.view, View::Typing);
        assert!(model.take_effects().contains(&Effect::MachineChanged));
        assert!(
            model
                .project
                .machine
                .page()
                .line_text(12)
                .contains("reopened"),
            "the opened project is in, whole"
        );
        assert!(model.project.filing.is_at(&path));
        std::fs::remove_dir_all(path.parent().unwrap()).unwrap();
    }

    #[test]
    fn an_unopenable_project_says_so_and_stays() {
        let garbage = crate::storage::test_desktop().join("leave-garbage/broken.typr");
        std::fs::create_dir_all(garbage.parent().unwrap()).unwrap();
        std::fs::write(&garbage, "not a project").unwrap();
        let mut model = model();
        type_text(&mut model, "mine", 10.0);
        model.update(Intent::OpenFile(garbage.clone()), 11.0);
        assert!(model.leaving.is_some());
        model.update(Intent::Leave(Answer::DontSave), 12.0);
        assert!(
            model.notice.is_animating(12.0),
            "told why it could not open"
        );
        assert!(
            !model.take_effects().contains(&Effect::MachineChanged),
            "still the same project"
        );
        assert!(model.project.machine.page().line_text(12).contains("mine"));
        std::fs::remove_dir_all(garbage.parent().unwrap()).unwrap();
    }

    #[test]
    fn save_as_from_the_dialog_finishes_the_leave() {
        let dir = crate::storage::test_desktop().join("leave-saveas");
        let mut model = model();
        type_text(&mut model, "notes", 11.0);
        model.take_effects();
        model.update(Intent::CloseWindow, 12.0);
        assert_eq!(model.take_effects(), [Effect::CancelClose]);
        model.update(Intent::Leave(Answer::SaveAs), 13.0);
        assert_eq!(model.take_effects(), [Effect::Ask(Dialog::SaveAs)]);
        model.update(Intent::Picked(Picked::SaveAs(dir.join("novel"))), 14.0);
        assert!(dir.join("novel.typr").exists(), "the chosen name, written");
        assert!(model.notice.is_animating(14.0), "and told");
        assert_eq!(
            model.take_effects(),
            [Effect::Close],
            "the pending quit goes through"
        );
        assert!(model.project.filing.is_saved());
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn a_save_that_fails_on_leaving_keeps_the_project() {
        let dir = crate::storage::test_desktop().join("leave-fail");
        std::fs::create_dir_all(&dir).unwrap();
        let blocker = dir.join("is-a-file");
        std::fs::write(&blocker, "x").unwrap();
        let mut model = model();
        model.settings.saving.autosave = false;
        model.project.filing = Filing::at(blocker.join("novel.typr"));
        type_text(&mut model, "work", 10.0);
        model.take_effects();
        model.update(Intent::CloseWindow, 11.0);
        assert_eq!(model.take_effects(), [Effect::CancelClose]);
        model.update(Intent::Leave(Answer::Save), 12.0);
        assert!(
            model.notice.is_animating(12.0),
            "the failure is told, and the app stays open"
        );
        assert!(model.take_effects().is_empty(), "no Close: stays put");
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn put_away_writes_the_draft_it_was_given() {
        let draft = crate::storage::test_desktop().join("drafts/leave-putaway.typr");
        std::fs::create_dir_all(draft.parent().unwrap()).unwrap();
        std::fs::remove_file(&draft).ok();
        let mut model = model();
        model.project.filing = Filing::at(draft.clone());
        type_text(&mut model, "kept on closing", 10.0);
        model.put_away().unwrap();
        assert!(draft.exists(), "even a draft is written on quit");
        let reopened = filing::open(&model.machines, &draft).unwrap();
        assert!(reopened.page().line_text(12).contains("kept"));
        std::fs::remove_file(draft).unwrap();
    }
}
