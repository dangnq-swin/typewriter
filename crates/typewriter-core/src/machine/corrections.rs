//! Corrections: the slip, the eraser, fluid and Delete.

use super::{Event, Typewriter};
use crate::constraints::EraseMode;
use crate::page::Correction;

impl Typewriter {
    pub(super) fn set_erase_mode(&mut self, mode: EraseMode) -> Vec<Event> {
        self.constraints.erase = mode;
        self.take_slip_out()
    }

    pub(super) fn fluid_dried(&mut self, half_line: u16, column: u16) -> Vec<Event> {
        self.document.current_mut().dry(half_line, column);
        vec![]
    }

    pub(super) fn erase(&mut self) -> Vec<Event> {
        let mode = self.constraints.erase;
        let correction = match mode {
            EraseMode::Delete => None,
            EraseMode::Paper => {
                self.slip_in = !self.slip_in;
                return vec![if self.slip_in {
                    Event::SlipIn
                } else {
                    Event::SlipOut
                }];
            }
            EraseMode::Eraser => Some(Correction::Eraser),
            EraseMode::Fluid => Some(Correction::Fluid { wet: true }),
        };
        if let Err(reason) = self.step_back() {
            return vec![Event::Blocked(reason)];
        }
        let (row, col) = (self.carriage.half_line, self.carriage.column);
        let page = self.document.current_mut();
        match correction {
            Some(correction) => {
                page.cover(row, col, correction);
            }
            None => page.clear(row, col),
        }
        vec![Event::Erase(mode)]
    }

    pub(super) fn take_slip_out(&mut self) -> Vec<Event> {
        if std::mem::take(&mut self.slip_in) {
            vec![Event::SlipOut]
        } else {
            vec![]
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::testing::{sm9, type_str};
    use super::super::{Command, Event};
    use crate::constraints::EraseMode;
    use crate::page::{Correction, Mark};

    #[test]
    fn correction_paper_whitens_what_is_struck_through_the_slip() {
        let mut tw = sm9();
        assert_eq!(tw.constraints.erase, EraseMode::Paper);
        type_str(&mut tw, "cat");
        tw.apply(Command::Backspace);
        tw.apply(Command::Backspace);
        assert_eq!(tw.apply(Command::Erase), [Event::SlipIn]);
        assert!(tw.slip_in());
        // Retype the wrong letter through the slip: it prints chalk.
        assert_eq!(type_str(&mut tw, "a"), [Event::KeyStrike('a')]);
        assert_eq!(tw.apply(Command::Erase), [Event::SlipOut]);
        tw.apply(Command::Backspace);
        type_str(&mut tw, "u");
        assert_eq!(tw.page().line_text(12).trim_start(), "cut");
        assert_eq!(
            tw.page().cell(12, 11).unwrap().marks(),
            [
                Mark::Glyph('a'),
                Mark::Correction(Correction::Chalk('a')),
                Mark::Glyph('u')
            ]
        );
    }

    #[test]
    fn correction_paper_whitens_an_overstruck_character() {
        let mut tw = sm9();
        type_str(&mut tw, "'");
        tw.apply(Command::Backspace);
        type_str(&mut tw, ".");
        tw.apply(Command::Backspace);
        tw.apply(Command::Erase);
        // Both strikes again through the slip.
        type_str(&mut tw, "'");
        tw.apply(Command::Backspace);
        type_str(&mut tw, ".");
        tw.apply(Command::Erase);
        tw.apply(Command::Backspace);
        type_str(&mut tw, "?");
        assert_eq!(tw.page().line_text(12).trim_start(), "?");
    }

    #[test]
    fn the_slip_comes_out_with_the_sheet_or_another_method() {
        let mut tw = sm9();
        tw.apply(Command::Erase);
        assert_eq!(
            tw.apply(Command::SetEraseMode(EraseMode::Eraser)),
            [Event::SlipOut]
        );
        assert!(tw.apply(Command::SetEraseMode(EraseMode::Paper)).is_empty());
        tw.apply(Command::Erase);
        assert_eq!(
            tw.apply(Command::FeedSheet),
            [Event::SlipOut, Event::SheetFed]
        );
        assert!(!tw.slip_in());
    }

    #[test]
    fn eraser_covers_the_previous_character_and_allows_retyping() {
        let mut tw = sm9();
        tw.constraints.erase = EraseMode::Eraser;
        type_str(&mut tw, "x");
        assert_eq!(tw.apply(Command::Erase), [Event::Erase(EraseMode::Eraser)]);
        assert_eq!(tw.carriage().column, 10);
        type_str(&mut tw, "y");
        let cell = tw.page().cell(12, 10).unwrap();
        assert_eq!(cell.top_glyph(), Some('y'));
        assert_eq!(cell.marks().len(), 3);
    }

    #[test]
    fn typing_on_wet_fluid_smudges_until_the_app_says_it_dried() {
        let mut tw = sm9();
        tw.constraints.erase = EraseMode::Fluid;
        type_str(&mut tw, "xx");
        tw.apply(Command::Erase);
        tw.apply(Command::Erase);
        type_str(&mut tw, "y");
        assert_eq!(
            tw.page().cell(12, 10).unwrap().marks().last(),
            Some(&Mark::Smudged('y'))
        );
        tw.apply(Command::FluidDried {
            half_line: 12,
            column: 11,
        });
        type_str(&mut tw, "z");
        assert_eq!(
            tw.page().cell(12, 11).unwrap().marks().last(),
            Some(&Mark::Glyph('z'))
        );
        assert_eq!(tw.page().line_text(12).trim_start(), "yz");
    }

    #[test]
    fn delete_leaves_no_trace() {
        let mut tw = sm9();
        tw.constraints.erase = EraseMode::Delete;
        type_str(&mut tw, "xy");
        assert_eq!(tw.apply(Command::Erase), [Event::Erase(EraseMode::Delete)]);
        assert_eq!(tw.carriage().column, 11);
        assert!(tw.page().cell(12, 11).is_none());
        type_str(&mut tw, "z");
        assert_eq!(tw.page().line_text(12).trim_start(), "xz");
        assert_eq!(tw.page().cell(12, 11).unwrap().marks(), [Mark::Glyph('z')]);
    }
}
