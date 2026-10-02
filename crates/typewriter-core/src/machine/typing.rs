//! Striking keys: type, dead keys, the bell, and the backspace.

use super::{BlockReason, Event, Typewriter};
use crate::accents;
use crate::page::{Correction, Mark};

impl Typewriter {
    /// First column the carriage can't type at, and why.
    pub(super) fn right_limit(&self) -> (u16, BlockReason) {
        let c = &self.carriage;
        if self.constraints.lock_at_right_margin && !c.margin_released {
            (c.right_margin, BlockReason::RightMargin)
        } else {
            (self.document.current().columns(), BlockReason::PaperEdge)
        }
    }

    fn left_limit(&self) -> (u16, BlockReason) {
        let c = &self.carriage;
        if c.margin_released || c.column < c.left_margin {
            (0, BlockReason::PaperEdge)
        } else {
            (c.left_margin, BlockReason::LeftMargin)
        }
    }

    /// Moves right, ringing the bell on passing the trip point.
    pub(super) fn advance_to(&mut self, column: u16, events: &mut Vec<Event>) {
        let from = self.carriage.column;
        self.carriage.column = column;
        let offset = self.profile.bell_columns_before_margin;
        if self.carriage.crosses_bell(from, column, offset) {
            events.push(Event::Bell);
        }
    }

    pub(super) fn type_char(&mut self, c: char) -> Vec<Event> {
        // The typist's way to é: the dead ´, then e over it.
        if let Some((base, accent)) = accents::decompose(c)
            && self.profile.dead_keys.contains(&accent)
        {
            let mut events = self.type_char(accent);
            if !events.iter().any(|e| matches!(e, Event::Blocked(_))) {
                events.extend(self.type_char(base));
            }
            return events;
        }
        // No such key: nothing strikes.
        if accents::is_accented(c) {
            return vec![];
        }
        self.strike(c)
    }

    /// Strikes `c` as one glyph, key or no key: for a manuscript retyped
    /// with its accents kept.
    pub(crate) fn strike(&mut self, c: char) -> Vec<Event> {
        if c.is_control() {
            return vec![Event::Blocked(BlockReason::Unprintable)];
        }
        let (limit, reason) = self.right_limit();
        let column = self.carriage.column;
        if column >= limit {
            return vec![Event::Blocked(reason)];
        }
        let half_line = self.carriage.half_line;
        let dead = self.profile.dead_keys.contains(&c);
        let page = self.document.current_mut();
        let mut events = if c.is_whitespace() {
            vec![Event::Space]
        } else {
            let mark = if self.slip_in {
                Mark::Correction(Correction::Chalk(c))
            } else {
                Mark::Glyph(c)
            };
            // A held dead key strikes its own mark again and again, never
            // moving on: one is enough, or the cell grows without end.
            let again = dead
                && page
                    .cell(half_line, column)
                    .is_some_and(|cell| cell.marks().last() == Some(&mark));
            if !again {
                if self.slip_in {
                    page.cover(half_line, column, Correction::Chalk(c));
                } else {
                    page.strike(half_line, column, c);
                }
            }
            vec![Event::KeyStrike(c)]
        };
        // A dead key's typebar strikes, but the carriage doesn't escape.
        if !dead {
            self.advance_to(column + 1, &mut events);
        }
        events
    }

    /// One column back, or why not.
    pub(super) fn step_back(&mut self) -> Result<(), BlockReason> {
        let (limit, reason) = self.left_limit();
        if self.carriage.column <= limit {
            return Err(reason);
        }
        self.carriage.column -= 1;
        Ok(())
    }

    pub(super) fn backspace(&mut self) -> Vec<Event> {
        if !self.constraints.backspace {
            return vec![Event::Blocked(BlockReason::NotAllowed)];
        }
        match self.step_back() {
            Ok(()) => vec![Event::Backspace],
            Err(reason) => vec![Event::Blocked(reason)],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::testing::{sm9, type_str, with_dead_keys};
    use super::super::{BlockReason, Command, Event};

    #[test]
    fn typing_strikes_and_advances() {
        let mut tw = sm9();
        let events = type_str(&mut tw, "a b");
        assert_eq!(
            events,
            [Event::KeyStrike('a'), Event::Space, Event::KeyStrike('b')]
        );
        assert_eq!(tw.carriage().column, 13);
        assert_eq!(tw.page().line_text(12).trim_start(), "a b");
    }

    #[test]
    fn control_characters_are_refused() {
        let mut tw = sm9();
        assert_eq!(
            tw.apply(Command::Type('\n')),
            [Event::Blocked(BlockReason::Unprintable)]
        );
        assert_eq!(tw.carriage().column, 10);
    }

    #[test]
    fn backspace_does_not_erase_so_next_strike_overtypes() {
        let mut tw = sm9();
        type_str(&mut tw, "no");
        tw.apply(Command::Backspace);
        tw.apply(Command::Backspace);
        type_str(&mut tw, "--");
        for (col, original) in [(10, 'n'), (11, 'o')] {
            let glyphs: String = tw.page().cell(12, col).unwrap().visible_glyphs().collect();
            assert_eq!(glyphs, format!("{original}-"));
        }
    }

    #[test]
    fn backspace_stops_at_left_margin_unless_released() {
        let mut tw = sm9();
        assert_eq!(
            tw.apply(Command::Backspace),
            [Event::Blocked(BlockReason::LeftMargin)]
        );
        tw.apply(Command::MarginRelease);
        assert_eq!(tw.apply(Command::Backspace), [Event::Backspace]);
        assert_eq!(tw.carriage().column, 9);
    }

    #[test]
    fn backspace_can_be_disabled() {
        let mut tw = sm9();
        tw.constraints.backspace = false;
        type_str(&mut tw, "a");
        assert_eq!(
            tw.apply(Command::Backspace),
            [Event::Blocked(BlockReason::NotAllowed)]
        );
    }

    #[test]
    fn a_dead_key_waits_for_its_letter() {
        let mut tw = with_dead_keys();
        assert_eq!(type_str(&mut tw, "^"), [Event::KeyStrike('^')]);
        assert_eq!(tw.carriage().column, 10);
        type_str(&mut tw, "o");
        assert_eq!(tw.carriage().column, 11);
        let cell = tw.page().cell(12, 10).unwrap();
        assert_eq!(cell.visible_glyphs().collect::<String>(), "^o");
        assert_eq!(cell.reads_as(), Some('\u{f4}'));
    }

    #[test]
    fn a_held_dead_key_leaves_one_mark() {
        let mut tw = with_dead_keys();
        let events = type_str(&mut tw, &"^".repeat(100));
        assert_eq!(events.len(), 100, "every repeat still strikes");
        assert_eq!(tw.page().cell(12, 10).unwrap().marks().len(), 1);
        type_str(&mut tw, "o^");
        assert_eq!(tw.page().cell(12, 10).unwrap().marks().len(), 2);
        assert_eq!(tw.page().cell(12, 11).unwrap().marks().len(), 1);
    }

    #[test]
    fn a_precomposed_letter_is_typed_as_accent_then_base() {
        let mut tw = with_dead_keys();
        let events = type_str(&mut tw, "\u{e9}t\u{e9}");
        assert_eq!(
            events
                .iter()
                .filter(|e| matches!(e, Event::KeyStrike(_)))
                .count(),
            5
        );
        assert_eq!(tw.carriage().column, 13);
        assert_eq!(tw.page().line_text(12).trim(), "\u{e9}t\u{e9}");
    }

    #[test]
    fn without_the_dead_key_an_accented_letter_is_not_typed() {
        let mut tw = with_dead_keys();
        assert_eq!(
            type_str(&mut tw, "\u{e8}\u{e7}"),
            [],
            "no dead ` or cedilla"
        );
        let mut tw = sm9();
        assert_eq!(type_str(&mut tw, "\u{e9}"), []);
        assert_eq!(tw.carriage().column, 10);
        // Accent keys print like any other.
        assert_eq!(type_str(&mut tw, "^"), [Event::KeyStrike('^')]);
        assert_eq!(tw.carriage().column, 11);
    }
}
