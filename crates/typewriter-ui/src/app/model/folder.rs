//! The folder and what is done there: choosing, opening, moving and
//! scrunching up finished sheets, their notes, the copy holder, and the
//! notebook and writing log beside it.

use typewriter_core::{Command, Direction};

use super::feeding::refeed_shift;
use super::{Leaving, Model, View};
use crate::app::intent::{Effect, Sound};
use crate::picker::Dialog;
use crate::render::folder::{FolderAction, Printout};
use crate::render::{calendar, holder, pad, splitmix64};

impl Model {
    /// Opens the folder with the newest sheet chosen.
    pub(super) fn open_folder(&mut self) {
        self.view = View::Folder;
        self.log_back = 0;
        self.overlays.log_open = false;
        self.selected = self.sheet_count().saturating_sub(1);
    }

    fn sheet_count(&self) -> usize {
        self.project.machine.document().finished().len()
    }

    /// Steps through finished sheets; negative is older.
    fn browse(&mut self, step: isize) {
        let count = self.sheet_count();
        match self.view {
            View::Typing | View::Settings => {}
            View::Folder => self.selected = stepped(self.selected, step, count),
            View::Sheet(i) => self.view = View::Sheet(stepped(i, step, count)),
        }
    }

    /// Outside the typing view, arrows and Enter browse instead. True if
    /// used up.
    pub(super) fn browse_command(&mut self, command: Command) -> bool {
        if self.view == View::Typing {
            return false;
        }
        match command {
            Command::Move(Direction::Up | Direction::Left) => self.browse(-1),
            Command::Move(Direction::Down | Direction::Right) => self.browse(1),
            // Swallow a held Enter that opened a sheet.
            Command::LineFeed => {}
            Command::Return if self.view == View::Folder => {
                if self.sheet_count() > 0 {
                    self.view = View::Sheet(self.selected);
                }
            }
            _ => return false,
        }
        true
    }

    pub(super) fn page_up(&mut self) {
        match self.view {
            View::Typing => self.open_folder(),
            View::Folder | View::Sheet(_) => self.browse(-1),
            // Unreachable: the card keeps the keys.
            View::Settings => {}
        }
    }

    pub(super) fn page_down(&mut self) {
        self.browse(1);
    }

    pub(super) fn escape(&mut self) {
        self.view = match self.view {
            View::Typing => {
                self.calm = !self.calm;
                View::Typing
            }
            View::Folder | View::Settings => View::Typing,
            View::Sheet(i) => {
                self.selected = i;
                View::Folder
            }
        };
    }

    pub(super) fn folder_action(&mut self, action: FolderAction, now: f64) {
        match action {
            FolderAction::Save => self.save_now(now),
            FolderAction::SaveAs => self.effects.push(Effect::Ask(Dialog::SaveAs)),
            FolderAction::Rename => self.overlays.renaming = Some(self.project.filing.name()),
            FolderAction::RenameTo(name) => {
                self.overlays.renaming = None;
                let told = self.project.filing.rename(&self.project.machine, &name);
                self.tell(told, now);
            }
            FolderAction::CancelRename => self.overlays.renaming = None,
            FolderAction::New => self.leave(Leaving::New, now),
            FolderAction::Open => self.effects.push(Effect::Ask(Dialog::Open)),
            FolderAction::Renumber => self.overlays.renumbering = Some(String::new()),
            FolderAction::RenumberTo(number) => {
                self.overlays.renumbering = None;
                self.renumber(&number, now);
            }
            FolderAction::CancelRenumber => self.overlays.renumbering = None,
            FolderAction::Scrunch => {
                if self.selected < self.sheet_count() {
                    self.overlays.confirm_scrunch = Some(self.selected);
                }
            }
            FolderAction::PutOnHolder => {
                let sheets = self.project.machine.document().finished();
                if let Some(page) = sheets.get(self.selected) {
                    self.overlays.holder = Some(holder::Holder::new(page.clone()));
                    self.view = View::Typing;
                }
            }
            FolderAction::RollIn => {
                let shift = refeed_shift(splitmix64(now.to_bits()));
                let sheet = self.selected;
                self.apply(Command::RollIn { sheet, shift }, now);
            }
            FolderAction::Print(Printout::Project) => self.effects.push(Effect::Print(None)),
            FolderAction::Print(Printout::ChosenSheet) => {
                if self.selected < self.sheet_count() {
                    self.effects.push(Effect::Print(Some(self.selected)));
                }
            }
            FolderAction::Export(format) => {
                let ink_realism = self.settings.look.ink_realism;
                let project = &self.project;
                let told = project.filing.export(&project.machine, format, ink_realism);
                self.notice.show(told, now);
            }
        }
    }

    /// Moves the chosen sheet to the typed number. Out of range: no change.
    fn renumber(&mut self, number: &str, now: f64) {
        let count = self.sheet_count();
        if let Ok(n) = number.parse::<usize>()
            && (1..=count).contains(&n)
            && self.project.machine.renumber(self.selected, n - 1)
        {
            self.selected = n - 1;
            self.project.filing.changed(now);
        }
    }

    /// Moves the chosen sheet one place: up/left is older.
    pub(super) fn move_chosen(&mut self, direction: Direction, now: f64) {
        let count = self.sheet_count();
        let to = match direction {
            Direction::Up | Direction::Left => self.selected.checked_sub(1),
            Direction::Down | Direction::Right => Some(self.selected + 1).filter(|&to| to < count),
        };
        if let Some(to) = to
            && self.project.machine.renumber(self.selected, to)
        {
            self.selected = to;
            self.project.filing.changed(now);
        }
    }

    /// Scrunches up the finished sheet `index`, for good.
    pub(super) fn scrunch(&mut self, index: usize, now: f64) {
        if self.project.machine.scrunch(index).is_none() {
            return;
        }
        self.effects.push(Effect::Sound(Sound::Crumple));
        self.effects.push(Effect::Scrunched(index));
        self.selected = self.selected.min(self.sheet_count().saturating_sub(1));
        let project = &mut self.project;
        project.session.recount(project.machine.document());
        project.record_words();
        self.project.filing.changed(now);
    }

    /// The open sheet's note, to write on.
    pub(super) fn start_note(&mut self) {
        let View::Sheet(i) = self.view else {
            return;
        };
        if let Some(page) = self.project.machine.document().finished().get(i) {
            self.overlays.annotating = Some(page.note().to_owned());
        }
    }

    pub(super) fn note_written(&mut self, sheet: usize, note: &str, now: f64) {
        self.overlays.annotating = None;
        if self.project.machine.annotate(sheet, note) {
            self.project.filing.changed(now);
        }
    }

    pub(super) fn open_notebook(&mut self) {
        let book = self.project.machine.document().notebook();
        self.overlays.notebook = Some(pad::Writing::open(book));
    }

    pub(super) fn open_log(&mut self) {
        self.overlays.log_open = true;
        self.overlays.notebook = None;
    }

    /// The writing log's month as shown.
    pub fn log_month(&self) -> calendar::Month {
        calendar::Month::of(self.project.machine.log(), self.log_back, calendar::today())
    }

    /// Turns the log back (-1) or on (1) a month, where there is one.
    pub(super) fn turn_log(&mut self, step: isize) {
        let month = self.log_month();
        if step < 0 && month.earlier {
            self.log_back += 1;
        } else if step > 0 && month.later {
            self.log_back -= 1;
        }
    }
}

/// `index` moved `step`, kept within `count`.
fn stepped(index: usize, step: isize, count: usize) -> usize {
    index
        .saturating_add_signed(step)
        .min(count.saturating_sub(1))
}

#[cfg(test)]
mod tests {
    use super::super::testing::{model, press, type_text};
    use super::*;
    use crate::app::intent::Intent;
    use crate::input::Action;

    /// A model with `count` finished sheets, typed "1", "2"... in order.
    fn with_sheets(count: usize) -> Model {
        let mut model = model();
        for n in 1..=count {
            let now = 100.0 * n as f64;
            type_text(&mut model, &n.to_string(), now);
            press(
                &mut model,
                &[Action::Machine(Command::FeedSheet)],
                now + 1.0,
            );
            model.tick(now + 50.0);
        }
        model
    }

    /// What finished sheet `index` says.
    fn sheet(model: &Model, index: usize) -> String {
        let page = &model.project.machine.document().finished()[index];
        page.line_text(12).trim().to_owned()
    }

    #[test]
    fn print_asks_for_every_sheet_or_the_chosen_one() {
        let mut model = with_sheets(3);
        press(&mut model, &[Action::PageUp], 1000.0);
        model.take_effects();
        model.update(
            Intent::Folder(FolderAction::Print(Printout::Project)),
            1000.0,
        );
        assert_eq!(model.take_effects(), [Effect::Print(None)]);
        press(
            &mut model,
            &[Action::Machine(Command::Move(Direction::Up))],
            1001.0,
        );
        let chosen = Intent::Folder(FolderAction::Print(Printout::ChosenSheet));
        model.update(chosen.clone(), 1001.0);
        assert_eq!(model.take_effects(), [Effect::Print(Some(1))]);

        let mut empty = super::super::testing::model();
        empty.update(chosen, 10.0);
        assert!(empty.take_effects().is_empty(), "no sheet to choose");
    }

    #[test]
    fn browsing_stays_within_the_folder() {
        assert_eq!(stepped(2, -1, 5), 1);
        assert_eq!(stepped(0, -1, 5), 0);
        assert_eq!(stepped(4, 1, 5), 4);
        assert_eq!(stepped(0, 1, 0), 0);
    }

    #[test]
    fn page_up_opens_the_newest_sheet_and_esc_goes_back_the_way_it_came() {
        let mut model = with_sheets(3);
        let now = 1000.0;
        press(&mut model, &[Action::PageUp], now);
        assert_eq!((model.view, model.selected), (View::Folder, 2));
        press(
            &mut model,
            &[Action::Machine(Command::Move(Direction::Up))],
            now,
        );
        press(&mut model, &[Action::Machine(Command::Return)], now);
        assert_eq!(model.view, View::Sheet(1));
        press(&mut model, &[Action::PageDown], now);
        assert_eq!(model.view, View::Sheet(2));
        press(&mut model, &[Action::Escape], now);
        assert_eq!((model.view, model.selected), (View::Folder, 2));
        press(&mut model, &[Action::Escape], now);
        assert_eq!(model.view, View::Typing);
    }

    #[test]
    fn typing_in_the_folder_goes_back_to_the_typewriter() {
        let mut model = with_sheets(1);
        press(&mut model, &[Action::PageUp], 1000.0);
        type_text(&mut model, "a", 1001.0);
        assert_eq!(model.view, View::Typing);
    }

    #[test]
    fn fluid_in_the_machine_still_dries_after_a_filed_sheet_is_scrunched() {
        let mut model = with_sheets(1);
        type_text(&mut model, "x", 1000.0);
        model.project.machine.constraints.erase = typewriter_core::EraseMode::Fluid;
        press(&mut model, &[Action::Delete], 1001.0);
        model.update(Intent::OpenFolder, 1001.0);
        model.update(Intent::ScrunchUp(0), 1001.0);
        assert!(model.project.is_drying());
        model.tick(1010.0);
        assert!(!model.project.is_drying());
    }

    #[test]
    fn delete_in_the_folder_asks_before_scrunching_the_chosen_sheet() {
        let mut model = with_sheets(3);
        press(&mut model, &[Action::PageUp], 1000.0);
        press(&mut model, &[Action::Delete], 1000.0);
        assert_eq!(model.overlays.confirm_scrunch, Some(2));
        assert!(model.keys_to_fields(), "the question has the keys");
        model.update(Intent::KeepSheet, 1001.0);
        assert_eq!(model.sheet_count(), 3);
        press(&mut model, &[Action::Delete], 1002.0);
        model.update(Intent::ScrunchUp(2), 1003.0);
        assert_eq!(model.sheet_count(), 2);
        assert_eq!(model.selected, 1, "the chosen one stays in the folder");
        let effects = model.take_effects();
        assert!(effects.contains(&Effect::Scrunched(2)));
        assert!(effects.contains(&Effect::Sound(Sound::Crumple)));
    }

    #[test]
    fn a_sheet_moves_to_a_number_in_range_and_by_shift_arrows() {
        let mut model = with_sheets(3);
        press(&mut model, &[Action::PageUp], 1000.0);
        let renumber = |model: &mut Model, to: &str| {
            model.update(Intent::Folder(FolderAction::Renumber), 1000.0);
            let to = FolderAction::RenumberTo(to.to_owned());
            model.update(Intent::Folder(to), 1000.0);
        };
        renumber(&mut model, "9");
        assert_eq!((sheet(&model, 2), model.selected), ("3".to_owned(), 2));
        renumber(&mut model, "1");
        assert_eq!((sheet(&model, 0), model.selected), ("3".to_owned(), 0));
        press(&mut model, &[Action::ShiftArrow(Direction::Right)], 1001.0);
        assert_eq!((sheet(&model, 1), model.selected), ("3".to_owned(), 1));
        assert_eq!(model.overlays.renumbering, None);
    }
}
