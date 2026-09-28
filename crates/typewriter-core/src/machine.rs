//! The typewriter as a whole: commands in, events out.

use crate::carriage::{Carriage, LineSpacing};
use crate::constraints::{Constraints, EraseMode};
use crate::page::{Correction, Page};
use crate::profile::{Profile, ProfileError};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Command {
    Type(char),
    Backspace,
    /// Erase or cover the character before the carriage, per [`EraseMode`].
    Erase,
    /// Carriage return and line feed in one throw of the lever.
    Return,
    Tab,
    SetTabStop,
    /// Clears the tab stop nearest the carriage, within a few columns.
    ClearTabStop,
    ClearAllTabStops,
    SetLeftMargin,
    SetRightMargin,
    MarginRelease,
    SetLineSpacing(LineSpacing),
    /// Carriage release lever / platen knob. Vertical moves are in half lines.
    Move(Direction),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    Left,
    Right,
    Up,
    Down,
}

/// What happened, for the app to turn into sound and animation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Event {
    KeyStrike(char),
    Space,
    Backspace,
    Tab,
    Erase(EraseMode),
    Bell,
    CarriageReturn,
    /// A line feed would roll the paper past its bottom edge.
    PageEnd,
    Blocked(BlockReason),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlockReason {
    RightMargin,
    LeftMargin,
    PaperEdge,
    /// Turned off in [`Constraints`].
    NotAllowed,
    Unprintable,
    InvalidStop,
}

#[derive(Debug, Clone)]
pub struct Typewriter {
    profile: Profile,
    pub constraints: Constraints,
    carriage: Carriage,
    page: Page,
}

impl Typewriter {
    pub fn new(profile: Profile, constraints: Constraints) -> Result<Self, ProfileError> {
        profile.validate()?;
        let m = &profile.margins;
        let mut carriage = Carriage::new(m.left_column, m.right_column, m.top_lines * 2);
        for &stop in &profile.tab_stops {
            carriage.set_tab_stop(stop);
        }
        let page = Page::new(profile.columns(), profile.half_lines());
        Ok(Self {
            profile,
            constraints,
            carriage,
            page,
        })
    }

    pub fn profile(&self) -> &Profile {
        &self.profile
    }

    pub fn carriage(&self) -> &Carriage {
        &self.carriage
    }

    pub fn page(&self) -> &Page {
        &self.page
    }

    pub fn apply(&mut self, command: Command) -> Vec<Event> {
        match command {
            Command::Type(c) => self.type_char(c),
            Command::Backspace => self.backspace(),
            Command::Erase => self.erase(),
            Command::Return => self.carriage_return(),
            Command::Tab => self.tab(),
            Command::SetTabStop => self.set_tab_stop(),
            Command::ClearTabStop => match self.carriage.clear_nearest_tab_stop() {
                Some(_) => vec![],
                None => vec![Event::Blocked(BlockReason::InvalidStop)],
            },
            Command::ClearAllTabStops => {
                self.carriage.clear_all_tab_stops();
                vec![]
            }
            Command::SetLeftMargin => self.set_left_margin(),
            Command::SetRightMargin => self.set_right_margin(),
            Command::MarginRelease => {
                self.carriage.margin_released = true;
                vec![]
            }
            Command::SetLineSpacing(spacing) => {
                self.carriage.line_spacing = spacing;
                vec![]
            }
            Command::Move(direction) => self.move_freely(direction),
        }
    }

    /// Column the carriage cannot type at or pass, and why.
    fn right_limit(&self) -> (u16, BlockReason) {
        let c = &self.carriage;
        if self.constraints.lock_at_right_margin && !c.margin_released {
            (c.right_margin, BlockReason::RightMargin)
        } else {
            (self.page.columns(), BlockReason::PaperEdge)
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

    /// Moves the carriage right, ringing the bell if it passes the trip point.
    fn advance_to(&mut self, column: u16, events: &mut Vec<Event>) {
        let from = self.carriage.column;
        self.carriage.column = column;
        let offset = self.profile.bell_columns_before_margin;
        if self.carriage.crosses_bell(from, column, offset) {
            events.push(Event::Bell);
        }
    }

    fn type_char(&mut self, c: char) -> Vec<Event> {
        if c.is_control() {
            return vec![Event::Blocked(BlockReason::Unprintable)];
        }
        let (limit, reason) = self.right_limit();
        let column = self.carriage.column;
        if column >= limit {
            return vec![Event::Blocked(reason)];
        }
        let mut events = if c.is_whitespace() {
            vec![Event::Space]
        } else {
            self.page.strike(self.carriage.half_line, column, c);
            vec![Event::KeyStrike(c)]
        };
        self.advance_to(column + 1, &mut events);
        events
    }

    /// Steps the carriage back one column, or says why it cannot.
    fn step_back(&mut self) -> Result<(), BlockReason> {
        let (limit, reason) = self.left_limit();
        if self.carriage.column <= limit {
            return Err(reason);
        }
        self.carriage.column -= 1;
        Ok(())
    }

    fn backspace(&mut self) -> Vec<Event> {
        if !self.constraints.backspace {
            return vec![Event::Blocked(BlockReason::NotAllowed)];
        }
        match self.step_back() {
            Ok(()) => vec![Event::Backspace],
            Err(reason) => vec![Event::Blocked(reason)],
        }
    }

    fn erase(&mut self) -> Vec<Event> {
        let mode = self.constraints.erase;
        let correction = match mode {
            EraseMode::Off => return vec![Event::Blocked(BlockReason::NotAllowed)],
            EraseMode::Digital => None,
            EraseMode::WhiteOut => Some(Correction::WhiteOut),
            EraseMode::CorrectionTape => Some(Correction::CorrectionTape),
        };
        if let Err(reason) = self.step_back() {
            return vec![Event::Blocked(reason)];
        }
        let (row, col) = (self.carriage.half_line, self.carriage.column);
        match correction {
            None => self.page.erase(row, col),
            Some(correction) => {
                self.page.cover(row, col, correction);
            }
        }
        vec![Event::Erase(mode)]
    }

    fn carriage_return(&mut self) -> Vec<Event> {
        let c = &mut self.carriage;
        c.column = c.left_margin;
        c.margin_released = false;
        let next = c.half_line + c.line_spacing.half_lines();
        if next < self.page.half_lines() {
            c.half_line = next;
            vec![Event::CarriageReturn]
        } else {
            vec![Event::CarriageReturn, Event::PageEnd]
        }
    }

    fn tab(&mut self) -> Vec<Event> {
        let (limit, reason) = self.right_limit();
        let column = self.carriage.column;
        if column >= limit {
            return vec![Event::Blocked(reason)];
        }
        // With no stop ahead the carriage flies until the margin stops it.
        let target = self
            .carriage
            .next_tab_stop()
            .map_or(self.carriage.right_margin, |stop| stop.min(limit));
        if target <= column {
            return vec![Event::Blocked(reason)];
        }
        let mut events = vec![Event::Tab];
        self.advance_to(target, &mut events);
        events
    }

    fn set_tab_stop(&mut self) -> Vec<Event> {
        if self.carriage.column >= self.page.columns() {
            return vec![Event::Blocked(BlockReason::InvalidStop)];
        }
        self.carriage.set_tab_stop(self.carriage.column);
        vec![]
    }

    fn set_left_margin(&mut self) -> Vec<Event> {
        let c = &mut self.carriage;
        if c.column >= c.right_margin {
            return vec![Event::Blocked(BlockReason::InvalidStop)];
        }
        c.left_margin = c.column;
        vec![]
    }

    fn set_right_margin(&mut self) -> Vec<Event> {
        let columns = self.page.columns();
        let c = &mut self.carriage;
        if c.column <= c.left_margin || c.column > columns {
            return vec![Event::Blocked(BlockReason::InvalidStop)];
        }
        c.right_margin = c.column;
        vec![]
    }

    fn move_freely(&mut self, direction: Direction) -> Vec<Event> {
        if !self.constraints.free_movement {
            return vec![Event::Blocked(BlockReason::NotAllowed)];
        }
        let columns = self.page.columns();
        let half_lines = self.page.half_lines();
        let c = &mut self.carriage;
        let (position, max) = match direction {
            Direction::Left | Direction::Right => (&mut c.column, columns),
            Direction::Up | Direction::Down => (&mut c.half_line, half_lines.saturating_sub(1)),
        };
        let target = match direction {
            Direction::Left | Direction::Up => position.checked_sub(1),
            Direction::Right | Direction::Down => position.checked_add(1).filter(|&p| p <= max),
        };
        match target {
            Some(p) => {
                *position = p;
                vec![]
            }
            None => vec![Event::Blocked(BlockReason::PaperEdge)],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sm9() -> Typewriter {
        let profile =
            Profile::from_toml_str(include_str!("../../../profiles/olympia-sm9.toml")).unwrap();
        Typewriter::new(profile, Constraints::default()).unwrap()
    }

    fn type_str(tw: &mut Typewriter, s: &str) -> Vec<Event> {
        s.chars().flat_map(|c| tw.apply(Command::Type(c))).collect()
    }

    #[test]
    fn starts_at_left_margin_below_top_margin() {
        let tw = sm9();
        assert_eq!(tw.carriage().column, 10);
        assert_eq!(tw.carriage().half_line, 12);
    }

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
        type_str(&mut tw, "o");
        assert_eq!(tw.apply(Command::Backspace), [Event::Backspace]);
        type_str(&mut tw, "/");
        let cell = tw.page().cell(12, 10).unwrap();
        assert_eq!(cell.visible_glyphs().collect::<String>(), "o/");
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
    fn digital_erase_removes_the_previous_character() {
        let mut tw = sm9();
        type_str(&mut tw, "ab");
        assert_eq!(tw.apply(Command::Erase), [Event::Erase(EraseMode::Digital)]);
        assert_eq!(tw.carriage().column, 11);
        assert!(tw.page().cell(12, 11).is_none());
    }

    #[test]
    fn white_out_covers_and_allows_retyping() {
        let mut tw = sm9();
        tw.constraints.erase = EraseMode::WhiteOut;
        type_str(&mut tw, "x");
        tw.apply(Command::Erase);
        type_str(&mut tw, "y");
        let cell = tw.page().cell(12, 10).unwrap();
        assert_eq!(cell.top_glyph(), Some('y'));
        assert_eq!(cell.marks().len(), 3);
    }

    #[test]
    fn erase_can_be_disabled() {
        let mut tw = sm9();
        tw.constraints.erase = EraseMode::Off;
        type_str(&mut tw, "x");
        assert_eq!(
            tw.apply(Command::Erase),
            [Event::Blocked(BlockReason::NotAllowed)]
        );
    }

    #[test]
    fn return_goes_to_left_margin_and_feeds_by_spacing() {
        let mut tw = sm9();
        type_str(&mut tw, "abc");
        assert_eq!(tw.apply(Command::Return), [Event::CarriageReturn]);
        assert_eq!((tw.carriage().column, tw.carriage().half_line), (10, 14));
        tw.apply(Command::SetLineSpacing(LineSpacing::OneAndHalf));
        tw.apply(Command::Return);
        assert_eq!(tw.carriage().half_line, 17);
        tw.apply(Command::SetLineSpacing(LineSpacing::Double));
        tw.apply(Command::Return);
        assert_eq!(tw.carriage().half_line, 21);
    }

    #[test]
    fn return_resets_margin_release() {
        let mut tw = sm9();
        tw.apply(Command::MarginRelease);
        tw.apply(Command::Return);
        assert!(!tw.carriage().margin_released);
    }

    #[test]
    fn tab_jumps_to_next_stop_or_margin() {
        let mut tw = sm9();
        tw.carriage.set_tab_stop(20);
        assert_eq!(tw.apply(Command::Tab), [Event::Tab]);
        assert_eq!(tw.carriage().column, 20);
        assert_eq!(tw.apply(Command::Tab), [Event::Tab, Event::Bell]);
        assert_eq!(tw.carriage().column, 72);
        assert_eq!(
            tw.apply(Command::Tab),
            [Event::Blocked(BlockReason::RightMargin)]
        );
    }

    #[test]
    fn tab_stops_are_set_and_cleared_at_the_carriage() {
        let mut tw = sm9();
        type_str(&mut tw, "     ");
        tw.apply(Command::SetTabStop);
        assert_eq!(tw.carriage().tab_stops().collect::<Vec<_>>(), [15]);
        type_str(&mut tw, "  ");
        assert_eq!(tw.apply(Command::ClearTabStop), []);
        assert_eq!(tw.carriage().tab_stops().count(), 0);
        assert_eq!(
            tw.apply(Command::ClearTabStop),
            [Event::Blocked(BlockReason::InvalidStop)]
        );
        tw.apply(Command::SetTabStop);
        tw.apply(Command::ClearAllTabStops);
        assert_eq!(tw.carriage().tab_stops().count(), 0);
    }

    #[test]
    fn margins_are_set_at_the_carriage() {
        let mut tw = sm9();
        type_str(&mut tw, "     ");
        tw.apply(Command::SetLeftMargin);
        tw.apply(Command::Return);
        assert_eq!(tw.carriage().column, 15);
        type_str(&mut tw, "     ");
        tw.apply(Command::SetRightMargin);
        assert_eq!(tw.carriage().right_margin, 20);
        assert_eq!(
            type_str(&mut tw, "x"),
            [Event::Blocked(BlockReason::RightMargin)]
        );
    }

    #[test]
    fn margin_cannot_cross_the_other() {
        let mut tw = sm9();
        assert_eq!(
            tw.apply(Command::SetRightMargin),
            [Event::Blocked(BlockReason::InvalidStop)]
        );
    }

    #[test]
    fn free_movement_is_off_by_default() {
        let mut tw = sm9();
        assert_eq!(
            tw.apply(Command::Move(Direction::Up)),
            [Event::Blocked(BlockReason::NotAllowed)]
        );
    }

    #[test]
    fn free_movement_moves_in_columns_and_half_lines() {
        let mut tw = sm9();
        tw.constraints.free_movement = true;
        tw.apply(Command::Move(Direction::Up));
        tw.apply(Command::Move(Direction::Left));
        assert_eq!((tw.carriage().column, tw.carriage().half_line), (9, 11));
        tw.apply(Command::Move(Direction::Down));
        tw.apply(Command::Move(Direction::Right));
        assert_eq!((tw.carriage().column, tw.carriage().half_line), (10, 12));
    }

    #[test]
    fn free_movement_stops_at_paper_edge() {
        let mut tw = sm9();
        tw.constraints.free_movement = true;
        for _ in 0..12 {
            tw.apply(Command::Move(Direction::Up));
        }
        assert_eq!(
            tw.apply(Command::Move(Direction::Up)),
            [Event::Blocked(BlockReason::PaperEdge)]
        );
    }
}
