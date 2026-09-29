//! The typewriter as a whole: commands in, events out.

use crate::carriage::{Carriage, LineSpacing};
use crate::constraints::{Constraints, EraseMode};
use crate::document::{Document, FORMAT_VERSION, FolderError, FolderFile};
use crate::page::{Correction, Page};
use crate::profile::{Profile, ProfileError};
use crate::scratchpad::Scratchpad;
use crate::session::SessionStats;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Command {
    Type(char),
    Backspace,
    /// Per [`EraseMode`]: cover the character before the carriage, or put
    /// the slip in or out.
    Erase,
    SetEraseMode(EraseMode),
    /// Fluid on this cell has dried. The core has no clock: the app says when.
    FluidDried {
        half_line: u16,
        column: u16,
    },
    /// Return and line feed, one throw of the lever.
    Return,
    /// Platen knob: roll on by the line spacing; the carriage stays put.
    LineFeed,
    /// Platen knob, one notch (a half-line). Winds a reopened sheet down.
    PlatenNotch,
    Tab,
    SetTabStop,
    /// Clear the stop nearest the carriage, within a few columns.
    ClearTabStop,
    ClearAllTabStops,
    SetLeftMargin,
    SetRightMargin,
    MarginRelease,
    SetLineSpacing(LineSpacing),
    /// Release lever / platen knob. Vertical steps are half-lines.
    Move(Direction),
    /// File the sheet and feed a blank one, carriage at the top margin.
    FeedSheet,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    Left,
    Right,
    Up,
    Down,
}

/// What happened. The app turns these into sound and animation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Event {
    KeyStrike(char),
    Space,
    Backspace,
    Tab,
    /// Eraser or fluid covered the character before the carriage.
    Erase(EraseMode),
    /// Slip in front of the ribbon: strikes now print chalk.
    SlipIn,
    SlipOut,
    Bell,
    CarriageReturn,
    /// Rolled on without a return.
    LineFeed,
    /// A feed would roll past the bottom edge. The app feeds a new sheet.
    PageEnd,
    SheetFed,
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
    document: Document,
    slip_in: bool,
    /// This project's sessions, oldest first.
    sessions: Vec<SessionStats>,
}

impl Typewriter {
    pub fn new(profile: Profile, constraints: Constraints) -> Result<Self, ProfileError> {
        profile.validate()?;
        let m = &profile.margins;
        let mut carriage = Carriage::new(m.left_column, m.right_column, m.top_lines * 2);
        for &stop in &profile.tab_stops {
            carriage.set_tab_stop(stop);
        }
        let document = Document::new(blank_sheet(&profile));
        Ok(Self {
            profile,
            constraints,
            carriage,
            document,
            slip_in: false,
            sessions: Vec::new(),
        })
    }

    /// The project as folder-file RON.
    pub fn to_folder_ron(&self) -> Result<String, FolderError> {
        FolderFile {
            version: FORMAT_VERSION,
            profile: self.profile.name.clone(),
            constraints: self.constraints.clone(),
            carriage: self.carriage.clone(),
            document: self.document.clone(),
            sessions: self.sessions.clone(),
        }
        .to_ron()
    }

    /// Loads a project into its own machine (found by name via `machine`),
    /// where typing stopped. The hand-held slip starts out.
    pub fn from_folder_ron(
        text: &str,
        machine: impl FnOnce(&str) -> Option<Profile>,
    ) -> Result<Self, FolderError> {
        let file = FolderFile::from_ron(text)?;
        let Some(profile) = machine(&file.profile) else {
            return Err(FolderError::UnknownMachine(file.profile));
        };
        let mut machine = Self::new(profile, file.constraints)
            .map_err(|err| FolderError::DoesNotFit(err.to_string()))?;
        let (columns, half_lines) = (machine.profile.columns(), machine.profile.half_lines());
        let document = &file.document;
        let fits = document.current().fits(columns, half_lines)
            && document
                .finished()
                .iter()
                .all(|p| p.fits(columns, half_lines))
            && file.carriage.fits(columns, half_lines);
        if !fits {
            return Err(FolderError::DoesNotFit(machine.profile.name));
        }
        machine.carriage = file.carriage;
        machine.document = file.document;
        machine.sessions = file.sessions;
        Ok(machine)
    }

    /// Reinserts the sheet as a typist would: carriage at the left margin,
    /// paper at the top margin. Returns where typing stopped: wind down to it.
    pub fn reinsert(&mut self) -> u16 {
        let c = &mut self.carriage;
        let stopped_at = c.half_line;
        c.half_line = stopped_at.min(self.profile.margins.top_lines * 2);
        c.column = c.left_margin;
        c.margin_released = false;
        stopped_at
    }

    pub fn profile(&self) -> &Profile {
        &self.profile
    }

    pub fn carriage(&self) -> &Carriage {
        &self.carriage
    }

    /// The sheet in the machine.
    pub fn page(&self) -> &Page {
        self.document.current()
    }

    pub fn document(&self) -> &Document {
        &self.document
    }

    /// Scrunches up finished sheet `index`. Returns it for the animation.
    pub fn scrunch(&mut self, index: usize) -> Option<Page> {
        self.document.remove(index)
    }

    /// Moves finished sheet `from` to position `to`.
    pub fn renumber(&mut self, from: usize, to: usize) -> bool {
        self.document.move_sheet(from, to)
    }

    /// Pencils a note in finished sheet `index`'s top margin.
    pub fn annotate(&mut self, index: usize, note: &str) -> bool {
        self.document.annotate(index, note)
    }

    pub fn scratchpad_mut(&mut self) -> &mut Scratchpad {
        self.document.scratchpad_mut()
    }

    pub fn sessions(&self) -> &[SessionStats] {
        &self.sessions
    }

    /// Records the session, replacing its entry if already recorded.
    pub fn record_session(&mut self, stats: SessionStats) {
        match self.sessions.last_mut() {
            Some(last) if last.started == stats.started => *last = stats,
            _ => self.sessions.push(stats),
        }
    }

    /// The correction slip is in front of the ribbon.
    pub fn slip_in(&self) -> bool {
        self.slip_in
    }

    pub fn apply(&mut self, command: Command) -> Vec<Event> {
        match command {
            Command::Type(c) => self.type_char(c),
            Command::Backspace => self.backspace(),
            Command::Erase => self.erase(),
            Command::SetEraseMode(mode) => {
                self.constraints.erase = mode;
                self.take_slip_out()
            }
            Command::FluidDried { half_line, column } => {
                self.document.current_mut().dry(half_line, column);
                vec![]
            }
            Command::Return => self.carriage_return(),
            Command::LineFeed => {
                if self.roll_one_line() {
                    vec![Event::LineFeed]
                } else {
                    vec![Event::PageEnd]
                }
            }
            Command::PlatenNotch => {
                let c = &mut self.carriage;
                if c.half_line + 1 < self.document.current().half_lines() {
                    c.half_line += 1;
                    vec![Event::LineFeed]
                } else {
                    vec![Event::PageEnd]
                }
            }
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
            Command::FeedSheet => self.feed_sheet(),
        }
    }

    /// First column the carriage can't type at, and why.
    fn right_limit(&self) -> (u16, BlockReason) {
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
        let half_line = self.carriage.half_line;
        let page = self.document.current_mut();
        let mut events = if c.is_whitespace() {
            vec![Event::Space]
        } else {
            if self.slip_in {
                page.cover(half_line, column, Correction::Chalk(c));
            } else {
                page.strike(half_line, column, c);
            }
            vec![Event::KeyStrike(c)]
        };
        self.advance_to(column + 1, &mut events);
        events
    }

    /// One column back, or why not.
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
            EraseMode::Paper => {
                self.slip_in = !self.slip_in;
                return vec![if self.slip_in {
                    Event::SlipIn
                } else {
                    Event::SlipOut
                }];
            }
            EraseMode::Eraser => Correction::Eraser,
            EraseMode::Fluid => Correction::Fluid { wet: true },
        };
        if let Err(reason) = self.step_back() {
            return vec![Event::Blocked(reason)];
        }
        let (row, col) = (self.carriage.half_line, self.carriage.column);
        self.document.current_mut().cover(row, col, correction);
        vec![Event::Erase(mode)]
    }

    fn take_slip_out(&mut self) -> Vec<Event> {
        if std::mem::take(&mut self.slip_in) {
            vec![Event::SlipOut]
        } else {
            vec![]
        }
    }

    fn carriage_return(&mut self) -> Vec<Event> {
        let c = &mut self.carriage;
        c.column = c.left_margin;
        c.margin_released = false;
        if self.roll_one_line() {
            vec![Event::CarriageReturn]
        } else {
            vec![Event::CarriageReturn, Event::PageEnd]
        }
    }

    /// Rolls on by the line spacing. False, not moving, past the bottom.
    fn roll_one_line(&mut self) -> bool {
        let c = &mut self.carriage;
        let next = c.half_line + c.line_spacing.half_lines();
        let fits = next < self.document.current().half_lines();
        if fits {
            c.half_line = next;
        }
        fits
    }

    fn tab(&mut self) -> Vec<Event> {
        let (limit, reason) = self.right_limit();
        let column = self.carriage.column;
        if column >= limit {
            return vec![Event::Blocked(reason)];
        }
        // No stop ahead: fly to the margin.
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
        if self.carriage.column >= self.document.current().columns() {
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
        let columns = self.document.current().columns();
        let c = &mut self.carriage;
        if c.column <= c.left_margin || c.column > columns {
            return vec![Event::Blocked(BlockReason::InvalidStop)];
        }
        c.right_margin = c.column;
        vec![]
    }

    fn feed_sheet(&mut self) -> Vec<Event> {
        let mut events = self.take_slip_out();
        // Filed sheets have time to dry.
        self.document.current_mut().dry_all();
        self.document.feed(blank_sheet(&self.profile));
        let c = &mut self.carriage;
        c.column = c.left_margin;
        c.half_line = self.profile.margins.top_lines * 2;
        c.margin_released = false;
        events.push(Event::SheetFed);
        events
    }

    fn move_freely(&mut self, direction: Direction) -> Vec<Event> {
        if !self.constraints.free_movement {
            return vec![Event::Blocked(BlockReason::NotAllowed)];
        }
        let columns = self.document.current().columns();
        let half_lines = self.document.current().half_lines();
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

fn blank_sheet(profile: &Profile) -> Page {
    Page::new(profile.columns(), profile.half_lines())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::page::Mark;

    fn sm9() -> Typewriter {
        let profile =
            Profile::from_toml_str(include_str!("../../../profiles/olympia-sm9.toml")).unwrap();
        Typewriter::new(profile, Constraints::default()).unwrap()
    }

    /// Finds only `profile`, as if it were the one machine there is.
    fn by_name(profile: &Profile) -> impl FnOnce(&str) -> Option<Profile> + '_ {
        move |name| (name == profile.name).then(|| profile.clone())
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
    fn a_folder_file_puts_everything_back_where_it_was() {
        let mut tw = sm9();
        tw.constraints.erase = EraseMode::Fluid;
        type_str(&mut tw, "first sheet");
        tw.apply(Command::FeedSheet);
        tw.apply(Command::SetLineSpacing(LineSpacing::Double));
        type_str(&mut tw, "ab");
        tw.apply(Command::Erase);
        tw.apply(Command::Tab);
        let session = |started, words| SessionStats {
            started,
            seconds: 90,
            words,
        };
        tw.record_session(session(100, 5));
        tw.record_session(session(100, 7));
        tw.record_session(session(200, 1));
        assert!(tw.annotate(0, "tighten\nthe opening"));
        assert!(!tw.annotate(1, "the sheet in the machine"));
        assert!(tw.scratchpad_mut().write(3, "call the printer"));
        assert!(tw.scratchpad_mut().open_at(2));
        let text = tw.to_folder_ron().unwrap();
        let back = Typewriter::from_folder_ron(&text, by_name(tw.profile())).unwrap();
        assert_eq!(back.sessions(), [session(100, 7), session(200, 1)]);
        assert_eq!(back.document(), tw.document());
        assert_eq!(back.document().scratchpad().page(3), "call the printer");
        assert_eq!(back.document().scratchpad().spread(), 2);
        assert_eq!(back.carriage(), tw.carriage());
        assert_eq!(back.constraints, tw.constraints);
    }

    #[test]
    fn a_reinserted_sheet_winds_back_notch_by_notch() {
        let mut tw = sm9();
        tw.apply(Command::SetLineSpacing(LineSpacing::OneAndHalf));
        type_str(&mut tw, "one");
        tw.apply(Command::Return);
        type_str(&mut tw, "two");
        let stopped_at = tw.reinsert();
        assert_eq!(stopped_at, 15);
        assert_eq!((tw.carriage().half_line, tw.carriage().column), (12, 10));
        let notches: Vec<Event> = (12..stopped_at)
            .flat_map(|_| tw.apply(Command::PlatenNotch))
            .collect();
        assert_eq!(notches, [Event::LineFeed; 3]);
        assert_eq!((tw.carriage().half_line, tw.carriage().column), (15, 10));
    }

    #[test]
    fn the_platen_stops_at_the_bottom_of_the_sheet() {
        let mut tw = sm9();
        for _ in 0..200 {
            tw.apply(Command::PlatenNotch);
        }
        assert_eq!(tw.carriage().half_line, 139);
        assert_eq!(tw.apply(Command::PlatenNotch), [Event::PageEnd]);
    }

    #[test]
    fn a_folder_file_from_before_session_stats_opens() {
        let mut tw = sm9();
        type_str(&mut tw, "old");
        let text = tw.to_folder_ron().unwrap();
        let start = text.find("sessions:").unwrap();
        let old = text[..start]
            .trim_end()
            .replacen("version: 4", "version: 1", 1)
            + "\n)";
        let back = Typewriter::from_folder_ron(&old, by_name(tw.profile())).unwrap();
        assert_eq!(back.document(), tw.document());
        assert!(back.sessions().is_empty());
    }

    #[test]
    fn a_folder_file_from_before_the_scratchpad_opens() {
        let mut tw = sm9();
        type_str(&mut tw, "old");
        let text = tw.to_folder_ron().unwrap();
        // Drop the scratchpad's lines, from its field to its closing one.
        let mut skipping = false;
        let old: Vec<&str> = text
            .lines()
            .filter(|line| {
                if line.trim_start().starts_with("scratchpad:") {
                    skipping = true;
                }
                let keep = !skipping;
                if skipping && line.trim() == ")," {
                    skipping = false;
                }
                keep
            })
            .collect();
        let old = old.join("\n").replacen("version: 4", "version: 3", 1);
        assert!(!old.contains("pages:"));
        let back = Typewriter::from_folder_ron(&old, by_name(tw.profile())).unwrap();
        assert_eq!(back.document(), tw.document());
        assert!(back.document().scratchpad().is_fresh());
    }

    #[test]
    fn folder_files_from_elsewhere_are_refused() {
        let tw = sm9();
        let text = tw.to_folder_ron().unwrap();
        let profile = tw.profile().clone();
        let newer = text.replacen("version: 4", "version: 99", 1);
        assert!(matches!(
            Typewriter::from_folder_ron(&newer, by_name(&profile)),
            Err(FolderError::NewerVersion(99))
        ));
        let other = text.replacen("Olympia SM9", "Hermes 3000", 1);
        assert!(matches!(
            Typewriter::from_folder_ron(&other, by_name(&profile)),
            Err(FolderError::UnknownMachine(_))
        ));
        let off_the_sheet = text.replacen("column: 10", "column: 900", 1);
        assert!(matches!(
            Typewriter::from_folder_ron(&off_the_sheet, by_name(&profile)),
            Err(FolderError::DoesNotFit(_))
        ));
        assert!(matches!(
            Typewriter::from_folder_ron("a shopping list", by_name(&profile)),
            Err(FolderError::Unreadable(_))
        ));
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
    fn line_feed_rolls_the_paper_but_not_the_carriage() {
        let mut tw = sm9();
        type_str(&mut tw, "abc");
        assert_eq!(tw.apply(Command::LineFeed), [Event::LineFeed]);
        assert_eq!((tw.carriage().column, tw.carriage().half_line), (13, 14));
        tw.apply(Command::SetLineSpacing(LineSpacing::Double));
        tw.apply(Command::LineFeed);
        assert_eq!(tw.carriage().half_line, 18);
    }

    #[test]
    fn line_feed_stops_at_the_bottom_of_the_sheet() {
        let mut tw = sm9();
        while tw.apply(Command::LineFeed) == [Event::LineFeed] {}
        let last = tw.carriage().half_line;
        assert!(last + 2 >= tw.page().half_lines());
        assert_eq!(tw.apply(Command::LineFeed), [Event::PageEnd]);
        assert_eq!(tw.carriage().half_line, last);
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
