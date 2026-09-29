//! Changing sheets: feeding a blank one, rolling a finished one back in.

use super::{Event, Typewriter, blank_sheet};
use crate::document::Document;
use crate::page::Shift;

impl Typewriter {
    pub(super) fn feed_sheet(&mut self) -> Vec<Event> {
        let fresh = blank_sheet(&self.profile);
        self.change_sheet(|document| {
            document.feed(fresh);
            true
        })
    }

    pub(super) fn roll_in(&mut self, sheet: usize, shift: Shift) -> Vec<Event> {
        self.change_sheet(|document| document.roll_in(sheet, shift))
    }

    /// Takes the sheet out and puts another in by `change`, carriage at the
    /// top margin. Nothing happens if `change` finds nothing to put in.
    fn change_sheet(&mut self, change: impl FnOnce(&mut Document) -> bool) -> Vec<Event> {
        // Filed sheets have time to dry.
        self.document.current_mut().dry_all();
        if !change(&mut self.document) {
            return vec![];
        }
        let mut events = self.take_slip_out();
        let c = &mut self.carriage;
        c.column = c.left_margin;
        c.half_line = self.profile.margins.top_lines * 2;
        c.margin_released = false;
        events.push(Event::SheetFed);
        events
    }
}

#[cfg(test)]
mod tests {
    use super::super::testing::{sm9, type_str};
    use super::super::{Command, Event};
    use crate::carriage::LineSpacing;
    use crate::page::Shift;

    #[test]
    fn feeding_a_sheet_files_the_old_one_and_starts_at_the_top() {
        let mut tw = sm9();
        type_str(&mut tw, "done");
        tw.apply(Command::Return);
        tw.apply(Command::SetLineSpacing(LineSpacing::Double));
        tw.apply(Command::SetTabStop);
        assert_eq!(tw.apply(Command::FeedSheet), [Event::SheetFed]);
        assert_eq!((tw.carriage().column, tw.carriage().half_line), (10, 12));
        assert!(tw.page().is_blank());
        assert_eq!(tw.document().finished().len(), 1);
        assert_eq!(tw.document().finished()[0].line_text(12).trim(), "done");
        // Machine settings stay as they were.
        assert_eq!(tw.carriage().line_spacing, LineSpacing::Double);
        assert_eq!(tw.carriage().tab_stops().count(), 1);
    }

    #[test]
    fn a_finished_sheet_rolls_back_in_at_the_top_margin() {
        let mut tw = sm9();
        type_str(&mut tw, "one");
        tw.apply(Command::FeedSheet);
        type_str(&mut tw, "two");
        let shift = Shift {
            across: 25,
            down: -20,
        };
        assert_eq!(
            tw.apply(Command::RollIn { sheet: 0, shift }),
            [Event::SheetFed]
        );
        assert_eq!(tw.page().line_text(12).trim(), "one");
        assert_eq!((tw.carriage().column, tw.carriage().half_line), (10, 12));
        type_str(&mut tw, "ONE");
        assert_eq!(tw.page().shift(12, 10, 1), shift, "typed out of line");
        tw.apply(Command::FeedSheet);
        let sheets: Vec<String> = tw
            .document()
            .finished()
            .iter()
            .map(|page| page.line_text(12).trim().to_owned())
            .collect();
        assert_eq!(sheets, ["ONE", "two"], "back in its place");
        assert_eq!(tw.apply(Command::RollIn { sheet: 5, shift }), []);
    }
}
