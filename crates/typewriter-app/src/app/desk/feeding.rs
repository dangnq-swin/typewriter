//! Sheets on the move: a feed rolling one out and the next in, a typed one
//! flying into the folder, a project's first sheet winding in, a reopened
//! one winding back to where typing stopped. Keys wait for them.

use typewriter_core::page::{Page, Shift};
use typewriter_core::{Command, Event};

use super::Desk;
use crate::app::intent::{Effect, Sound};
use crate::render::feed::FeedMotion;
use crate::render::folder::Flight;
use crate::render::smoothstep;

/// Wind-back pace for a short way; a long one speeds up to fit
/// [`WIND_BACK_MAX_SECONDS`].
const WIND_BACK_LINE_SECONDS: f64 = 0.1;
const WIND_BACK_MIN_SECONDS: f64 = 0.3;
const WIND_BACK_MAX_SECONDS: f64 = 3.0;

/// A feed in progress.
pub struct Feeding {
    pub started: f64,
    /// The finished sheet rolling out and its half-line, if any.
    pub outgoing: Option<(Page, u16)>,
    pub motion: FeedMotion,
}

/// A reopened sheet winding down to where typing stopped.
struct WindBack {
    to_half_line: u16,
    /// When and from which half-line. Set once the sheet has wound in.
    started: Option<(f64, u16)>,
}

/// The sheets' motion.
pub struct Feed {
    /// Timed from the feed sound.
    motion: FeedMotion,
    pub feeding: Option<Feeding>,
    /// The last filed sheet, flying into the folder or just landed.
    pub flight: Option<Flight>,
    /// Every project, new or reopened, starts by winding its sheet in...
    first_sheet_pending: bool,
    /// ...and a reopened one then winds down to where typing stopped.
    wind_back: Option<WindBack>,
}

impl Feed {
    /// For a project coming in. `wind_back_to`: where a reopened one stopped.
    pub fn new(motion: FeedMotion, wind_back_to: Option<u16>) -> Self {
        let mut feed = Self {
            motion,
            feeding: None,
            flight: None,
            first_sheet_pending: true,
            wind_back: None,
        };
        feed.restart(wind_back_to);
        feed
    }

    /// Another project is in.
    pub(super) fn restart(&mut self, wind_back_to: Option<u16>) {
        self.feeding = None;
        self.flight = None;
        self.first_sheet_pending = true;
        self.wind_back = wind_back_to.map(|to_half_line| WindBack {
            to_half_line,
            started: None,
        });
    }

    fn is_feeding(&self, now: f64) -> bool {
        self.feeding
            .as_ref()
            .is_some_and(|f| now - f.started < f.motion.duration())
    }

    /// A sheet moves, or is about to.
    pub fn is_moving(&self) -> bool {
        self.feeding.is_some() || self.wind_back.is_some()
    }

    /// A filed sheet flies, or the folder icon still answers its landing.
    pub fn is_filing(&self, now: f64) -> bool {
        self.flight.is_some_and(|flight| !flight.is_over(now))
    }
}

impl Desk {
    /// Input is locked while a sheet feeds or winds back.
    pub fn is_busy(&self, now: f64) -> bool {
        self.feed.is_feeding(now) || self.feed.wind_back.is_some()
    }

    /// Before the frame's input: a project's first sheet winds in, with
    /// nothing to wind out.
    pub fn start_frame(&mut self, now: f64) {
        if !std::mem::take(&mut self.feed.first_sheet_pending) {
            return;
        }
        self.feed.feeding = Some(Feeding {
            started: now,
            outgoing: None,
            motion: self.feed.motion.clone().after_wind_out(0.0),
        });
        self.effects.push(Effect::Sound(Sound::WindIn));
    }

    /// A sheet was fed: `outgoing` rolls out as the next rolls in. Typed, it
    /// was filed, and flies into the folder.
    pub(super) fn start_feed(&mut self, outgoing: Option<(Page, u16)>, now: f64) {
        let filed = outgoing.as_ref().is_some_and(|(page, _)| !page.is_blank());
        self.feed.flight = filed.then(|| Flight::new(now, self.feed.motion.wind_out()));
        self.feed.feeding = Some(Feeding {
            started: now,
            outgoing,
            motion: self.feed.motion.clone(),
        });
    }

    /// Ends a finished feed, then winds a reopened sheet back.
    pub(super) fn move_sheets(&mut self, now: f64) {
        if !self.feed.is_feeding(now) {
            self.feed.feeding = None;
        }
        self.wind_back(now);
    }

    /// Once a reopened sheet is in, turns the platen knob down to where
    /// typing stopped, a click per line.
    fn wind_back(&mut self, now: f64) {
        if self.feed.feeding.is_some() {
            return;
        }
        let Some(wind) = &mut self.feed.wind_back else {
            return;
        };
        let machine = &mut self.project.machine;
        let to = wind.to_half_line;
        let (started, from) = *wind
            .started
            .get_or_insert((now, machine.carriage().half_line));
        let target = wound_to(from, to, now - started);
        // Not `apply`: its roll sound is the full-volume one, and winding
        // back to the saved line changes nothing to save.
        while machine.carriage().half_line < target {
            if machine
                .apply(Command::PlatenNotch)
                .contains(&Event::PageEnd)
            {
                break;
            }
            if (machine.carriage().half_line - from).is_multiple_of(2) {
                self.effects.push(Effect::Sound(Sound::WindBackClick));
            }
        }
        if target >= to {
            self.feed.wind_back = None;
        }
    }
}

/// A re-fed sheet's way out of line, from random `bits`: up to a third of a
/// cell either way, across and down.
pub(super) fn refeed_shift(bits: u64) -> Shift {
    let within = |bits: u64| {
        // Safe cast: below 67, so -33..=33.
        (bits % 67) as i8 - 33
    };
    Shift {
        across: within(bits),
        down: within(bits >> 32),
    }
}

/// Where a hand on the knob has wound the sheet `elapsed` seconds in: slow
/// to start, fast midway, easing onto the line.
fn wound_to(from: u16, to: u16, elapsed: f64) -> u16 {
    let distance = to.saturating_sub(from);
    let seconds = (f64::from(distance) / 2.0 * WIND_BACK_LINE_SECONDS)
        .clamp(WIND_BACK_MIN_SECONDS, WIND_BACK_MAX_SECONDS);
    let eased = smoothstep((elapsed / seconds) as f32);
    // Safe cast: eased is 0..=1, so at most `distance`.
    from + (f32::from(distance) * eased).round() as u16
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::desk::testing::{press, type_text};
    use crate::input::Action;
    use crate::render::splitmix64;

    #[test]
    fn a_refed_sheet_is_never_more_than_a_third_of_a_cell_off() {
        for seed in 0..500 {
            let shift = refeed_shift(splitmix64(seed));
            assert!(shift.across.abs() <= 33 && shift.down.abs() <= 33);
        }
    }

    #[test]
    fn a_hand_on_the_knob_starts_slow_and_eases_onto_the_line() {
        let (from, to) = (12, 120);
        let at = |seconds| wound_to(from, to, seconds);
        assert_eq!(at(0.0), from);
        assert_eq!(at(WIND_BACK_MAX_SECONDS), to);
        assert_eq!(at(99.0), to);
        let steps: Vec<u16> = (0..=30).map(|i| at(f64::from(i) * 0.1)).collect();
        assert!(steps.windows(2).all(|w| w[0] <= w[1]));
        let (start, middle) = (steps[3] - steps[0], steps[16] - steps[13]);
        assert!(start < middle, "slow start: {start} vs {middle}");
    }

    #[test]
    fn a_short_way_winds_at_a_steady_hand_pace() {
        // Four lines: 0.4 s, not the full three.
        assert_eq!(wound_to(12, 20, 0.4), 20);
        assert!(wound_to(12, 20, 0.2) < 20);
        assert_eq!(wound_to(12, 12, 0.0), 12);
    }

    #[test]
    fn a_new_project_winds_its_first_sheet_in_before_keys_type() {
        let mut desk = super::super::testing::fresh();
        desk.start_frame(0.0);
        assert_eq!(desk.take_effects(), [Effect::Sound(Sound::WindIn)]);
        assert!(desk.is_busy(0.5));
        desk.tick(30.0);
        assert!(!desk.is_busy(30.0));
        desk.start_frame(30.0);
        assert!(desk.take_effects().is_empty(), "only once");
    }

    #[test]
    fn a_typed_sheet_flies_into_the_folder_and_a_blank_one_does_not() {
        let mut desk = super::super::testing::desk();
        press(&mut desk, &[Action::Machine(Command::FeedSheet)], 10.0);
        assert!(desk.feed.flight.is_none(), "blank: nothing filed");

        type_text(&mut desk, "done", 20.0);
        press(&mut desk, &[Action::Machine(Command::FeedSheet)], 30.0);
        assert!(desk.feed.flight.is_some());
        assert!(desk.is_animating(31.0));
        desk.tick(40.0);
        assert!(!desk.is_animating(40.0), "landed and still");
    }
}
