//! The typewriter as a whole: commands in, events out. Each mechanism has its
//! own submodule; `apply` hands every command to one.

mod corrections;
mod folder;
mod movement;
mod sheets;
#[cfg(test)]
mod testing;
mod typing;

use crate::carriage::{Carriage, LineSpacing};
use crate::constraints::{Constraints, EraseMode};
use crate::document::Document;
use crate::notebook::Notebook;
use crate::page::{Page, Shift};
use crate::profile::{Profile, ProfileError};
use crate::session::WritingLog;

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
    /// Margin-set keys: the stop to the carriage.
    SetLeftMargin,
    SetRightMargin,
    /// A stop slid along the scale by hand.
    MoveMargin {
        side: Side,
        column: u16,
    },
    MarginRelease,
    SetLineSpacing(LineSpacing),
    /// Left / right: release lever, if free movement is allowed. Up / down:
    /// platen knob, a half-line notch, always.
    Move(Direction),
    /// File the sheet and feed a blank one, carriage at the top margin.
    FeedSheet,
    /// File the sheet and roll finished sheet `sheet` back in, `shift` out
    /// of line, carriage at the top margin. It goes back to its place in
    /// the folder when fed out.
    RollIn {
        sheet: usize,
        shift: Shift,
    },
    /// Two strikes came too close: their typebars tangle, if type jams are on.
    /// The core has no clock: the app says when.
    Jam,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Side {
    Left,
    Right,
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
    /// Backspace pulled tangled typebars apart.
    Freed,
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
    /// Tangled typebars: only Backspace frees them.
    Jammed,
}

#[derive(Debug, Clone)]
pub struct Typewriter {
    profile: Profile,
    pub constraints: Constraints,
    carriage: Carriage,
    document: Document,
    slip_in: bool,
    /// Typebars tangled. Not saved: a reopened machine is freed.
    jammed: bool,
    log: WritingLog,
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
            jammed: false,
            log: WritingLog::default(),
        })
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

    pub fn notebook_mut(&mut self) -> &mut Notebook {
        self.document.notebook_mut()
    }

    /// Words per day on this project.
    pub fn log(&self) -> &WritingLog {
        &self.log
    }

    /// Adds `words` (negative: corrections) to `day` in the log.
    pub fn log_words(&mut self, day: jiff::civil::Date, words: i64) {
        self.log.add(day, words);
    }

    /// Typebars tangled: only Backspace does anything.
    pub fn is_jammed(&self) -> bool {
        self.jammed
    }

    /// The correction slip is in front of the ribbon.
    pub fn slip_in(&self) -> bool {
        self.slip_in
    }

    pub fn apply(&mut self, command: Command) -> Vec<Event> {
        if self.jammed {
            match command {
                Command::Backspace => {
                    self.jammed = false;
                    return vec![Event::Freed];
                }
                Command::Type(_)
                | Command::Erase
                | Command::Return
                | Command::LineFeed
                | Command::Tab
                | Command::Move(_)
                | Command::FeedSheet
                | Command::RollIn { .. } => return vec![Event::Blocked(BlockReason::Jammed)],
                _ => {}
            }
        }
        match command {
            Command::Type(c) => self.type_char(c),
            Command::Backspace => self.backspace(),
            Command::Erase => self.erase(),
            Command::SetEraseMode(mode) => self.set_erase_mode(mode),
            Command::FluidDried { half_line, column } => self.fluid_dried(half_line, column),
            Command::Return => self.carriage_return(),
            Command::LineFeed => self.line_feed(),
            Command::PlatenNotch => self.platen_notch(),
            Command::Tab => self.tab(),
            Command::SetTabStop => self.set_tab_stop(),
            Command::ClearTabStop => self.clear_tab_stop(),
            Command::ClearAllTabStops => self.clear_all_tab_stops(),
            Command::SetLeftMargin => self.move_margin(Side::Left, self.carriage.column),
            Command::SetRightMargin => self.move_margin(Side::Right, self.carriage.column),
            Command::MoveMargin { side, column } => self.move_margin(side, column),
            Command::MarginRelease => self.release_margins(),
            Command::SetLineSpacing(spacing) => self.set_line_spacing(spacing),
            Command::Move(direction) => self.move_freely(direction),
            Command::FeedSheet => self.feed_sheet(),
            Command::RollIn { sheet, shift } => self.roll_in(sheet, shift),
            Command::Jam => self.jam(),
        }
    }

    fn jam(&mut self) -> Vec<Event> {
        if !self.constraints.type_jams {
            return vec![];
        }
        self.jammed = true;
        vec![Event::Blocked(BlockReason::Jammed)]
    }
}

fn blank_sheet(profile: &Profile) -> Page {
    Page::new(profile.columns(), profile.half_lines())
}

#[cfg(test)]
mod tests {
    use super::testing::{sm9, type_str};
    use super::*;

    #[test]
    fn starts_at_left_margin_below_top_margin() {
        let tw = sm9();
        assert_eq!(tw.carriage().column, 10);
        assert_eq!(tw.carriage().half_line, 12);
    }

    #[test]
    fn tangled_typebars_block_the_keys_until_backspace_frees_them() {
        let mut tw = sm9();
        assert_eq!(tw.apply(Command::Jam), [], "off by default");
        tw.constraints.type_jams = true;
        type_str(&mut tw, "a");
        assert_eq!(
            tw.apply(Command::Jam),
            [Event::Blocked(BlockReason::Jammed)]
        );
        assert!(tw.is_jammed());
        for command in [Command::Type('b'), Command::Return, Command::Tab] {
            assert_eq!(tw.apply(command), [Event::Blocked(BlockReason::Jammed)]);
        }
        assert_eq!(tw.apply(Command::Backspace), [Event::Freed]);
        assert_eq!(
            tw.carriage().column,
            11,
            "freeing doesn't move the carriage"
        );
        type_str(&mut tw, "b");
        assert_eq!(tw.page().line_text(12).trim(), "ab");
    }
}
