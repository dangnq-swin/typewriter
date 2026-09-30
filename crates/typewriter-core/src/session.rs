//! Focus goals and session stats: words written and time typed, from opening
//! a project until it is put away; and the writing log, words per day.
//!
//! No clock here: the app passes key times and days.

use std::collections::BTreeMap;

use jiff::civil::Date;
use serde::{Deserialize, Serialize};

use crate::document::Document;
use crate::page::Page;

/// Longer gaps between keys don't count as typing.
pub const IDLE_SECONDS: f64 = 60.0;

/// What a session aims for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Goal {
    Words(u32),
    Minutes(u32),
    /// Reached by whichever comes first.
    WordsOrMinutes {
        words: u32,
        minutes: u32,
    },
}

impl Goal {
    /// The user's own goal from its two targets (`None` = off).
    pub fn custom(words: Option<u32>, minutes: Option<u32>) -> Option<Goal> {
        match (words, minutes) {
            (Some(words), Some(minutes)) => Some(Goal::WordsOrMinutes { words, minutes }),
            (Some(words), None) => Some(Goal::Words(words)),
            (None, Some(minutes)) => Some(Goal::Minutes(minutes)),
            (None, None) => None,
        }
    }

    /// The Goal plate's cycle after off: word presets, time presets, then the
    /// custom goal unless it is a preset.
    pub fn cycle(custom: Option<Goal>) -> Vec<Goal> {
        let mut goals: Vec<Goal> = [250, 500, 1000]
            .map(Goal::Words)
            .into_iter()
            .chain([15, 25, 50].map(Goal::Minutes))
            .collect();
        if let Some(custom) = custom.filter(|c| !goals.contains(c)) {
            goals.push(custom);
        }
        goals
    }

    /// The goal after `goal`, with off at both ends. A goal no longer in
    /// `cycle` moves on to the first.
    pub fn next(goal: Option<Goal>, cycle: &[Goal]) -> Option<Goal> {
        match goal {
            None => cycle.first().copied(),
            Some(goal) => match cycle.iter().position(|&g| g == goal) {
                Some(i) => cycle.get(i + 1).copied(),
                None => cycle.first().copied(),
            },
        }
    }
}

/// Net words per day: one entry for each day written on.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct WritingLog(BTreeMap<Date, i64>);

impl WritingLog {
    /// Adds `words` (negative: corrections) to `day`. A day back at zero
    /// is dropped.
    pub fn add(&mut self, day: Date, words: i64) {
        let total = self.0.entry(day).or_default();
        *total += words;
        if *total == 0 {
            self.0.remove(&day);
        }
    }

    /// Days written on, oldest first.
    pub fn days(&self) -> &BTreeMap<Date, i64> {
        &self.0
    }

    /// Words over every day.
    pub fn words(&self) -> i64 {
        self.0.values().sum()
    }
}

/// How far the session is towards its goal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Progress {
    pub goal: Goal,
    /// Net words, floored at zero.
    pub words: u32,
    pub minutes: u32,
    pub reached: bool,
}

/// The session in progress.
#[derive(Debug, Clone)]
pub struct Session {
    words_at_start: usize,
    /// (sheets counted, their words). Filed sheets don't change: count once.
    filed: (usize, usize),
    words: usize,
    seconds: f64,
    last_key: Option<f64>,
    goal: Option<Goal>,
    reached: bool,
}

impl Session {
    /// Starts counting from what is on the sheets now.
    pub fn start(document: &Document) -> Self {
        let mut session = Self {
            words_at_start: 0,
            filed: (0, 0),
            words: 0,
            seconds: 0.0,
            last_key: None,
            goal: None,
            reached: false,
        };
        session.count(document);
        session.words_at_start = session.words;
        session
    }

    /// A key was struck at `now` (any steady clock, seconds). True if this key
    /// reached the goal.
    pub fn typed(&mut self, document: &Document, now: f64) -> bool {
        if let Some(last) = self.last_key {
            let gap = now - last;
            if (0.0..=IDLE_SECONDS).contains(&gap) {
                self.seconds += gap;
            }
        }
        self.last_key = Some(now);
        self.count(document);
        let was_reached = self.reached;
        self.reached = self.is_met();
        self.reached && !was_reached
    }

    /// Recounts from scratch. Call after removing sheets.
    pub fn recount(&mut self, document: &Document) {
        self.filed = (0, 0);
        self.count(document);
    }

    pub fn goal(&self) -> Option<Goal> {
        self.goal
    }

    /// A goal already met counts as reached at once, without a bell.
    pub fn set_goal(&mut self, goal: Option<Goal>) {
        self.goal = goal;
        self.reached = self.is_met();
    }

    pub fn progress(&self) -> Option<Progress> {
        Some(Progress {
            goal: self.goal?,
            words: u32::try_from(self.net_words().max(0)).unwrap_or(u32::MAX),
            minutes: self.whole_seconds() / 60,
            reached: self.reached,
        })
    }

    fn is_met(&self) -> bool {
        let words = |target| self.net_words() >= i64::from(target);
        let minutes = |target: u32| self.whole_seconds() >= target.saturating_mul(60);
        match self.goal {
            None => false,
            Some(Goal::Words(target)) => words(target),
            Some(Goal::Minutes(target)) => minutes(target),
            Some(Goal::WordsOrMinutes {
                words: w,
                minutes: m,
            }) => words(w) || minutes(m),
        }
    }

    /// Words at the end less at the start: corrections subtract.
    pub fn net_words(&self) -> i64 {
        // Word counts never near i64::MAX.
        self.words as i64 - self.words_at_start as i64
    }

    fn whole_seconds(&self) -> u32 {
        // Clamped first, so the cast can't wrap.
        self.seconds.clamp(0.0, f64::from(u32::MAX)) as u32
    }

    fn count(&mut self, document: &Document) {
        let finished = document.finished();
        let (counted, words) = &mut self.filed;
        if *counted > finished.len() {
            (*counted, *words) = (0, 0);
        }
        for page in &finished[*counted..] {
            *words += words_on(page);
        }
        *counted = finished.len();
        self.words = *words + words_on(document.current());
    }
}

/// Words on a sheet: runs of visible characters along a line holding a
/// letter or digit (a lone dash is not a word).
pub fn words_on(page: &Page) -> usize {
    let mut words = 0;
    // (position of the run's last character, run has a letter or digit)
    let mut run: Option<((u16, u16), bool)> = None;
    for ((half_line, column), cell) in page.cells() {
        let Some(c) = cell.reads_as().filter(|c| !c.is_whitespace()) else {
            continue;
        };
        let alphanumeric = c.is_alphanumeric();
        run = Some(match run {
            Some(((h, col), word)) if h == half_line && col + 1 == column => {
                ((half_line, column), word || alphanumeric)
            }
            previous => {
                words += usize::from(previous.is_some_and(|(_, word)| word));
                ((half_line, column), alphanumeric)
            }
        });
    }
    words + usize::from(run.is_some_and(|(_, word)| word))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::page::Correction;

    fn typed(page: &mut Page, half_line: u16, text: &str) {
        for (i, c) in text.chars().enumerate() {
            if c != ' ' {
                page.strike(half_line, i as u16, c);
            }
        }
    }

    #[test]
    fn a_recount_follows_a_sheet_back_to_its_place() {
        let mut document = Document::new(Page::new(40, 10));
        typed(document.current_mut(), 0, "one two");
        document.feed(Page::new(40, 10));
        typed(document.current_mut(), 0, "three");
        document.feed(Page::new(40, 10));
        let mut session = Session::start(&document);
        assert!(document.roll_in(0, crate::page::Shift::default()));
        typed(document.current_mut(), 2, "four");
        document.feed(Page::new(40, 10));
        // The same number of sheets filed, one of them changed and moved.
        session.recount(&document);
        assert_eq!(session.net_words(), 1);
    }

    #[test]
    fn words_are_runs_of_type_with_a_letter_in_them() {
        let mut page = Page::new(40, 10);
        typed(&mut page, 0, "It was -- a dark,");
        typed(&mut page, 2, "stormy night!");
        assert_eq!(words_on(&page), 6);
    }

    #[test]
    fn corrected_words_are_not_counted() {
        let mut page = Page::new(40, 10);
        typed(&mut page, 0, "no yes");
        for column in 0..2 {
            page.cover(0, column, Correction::Eraser);
        }
        assert_eq!(words_on(&page), 1);
    }

    #[test]
    fn a_session_counts_net_words_and_typing_time() {
        let mut document = Document::new(Page::new(40, 10));
        typed(document.current_mut(), 0, "already here");
        let mut session = Session::start(&document);
        assert_eq!((session.net_words(), session.whole_seconds()), (0, 0));

        typed(document.current_mut(), 2, "one two three");
        session.typed(&document, 10.0);
        session.typed(&document, 40.0);
        // A long pause is left out.
        session.typed(&document, 200.0);
        session.typed(&document, 205.0);
        document.feed(Page::new(40, 10));
        typed(document.current_mut(), 0, "four");
        session.typed(&document, 206.0);
        assert_eq!((session.net_words(), session.whole_seconds()), (4, 36));
    }

    #[test]
    fn the_goal_is_reached_once() {
        let mut document = Document::new(Page::new(40, 10));
        let mut session = Session::start(&document);
        session.set_goal(Some(Goal::Words(2)));
        typed(document.current_mut(), 0, "one");
        assert!(!session.typed(&document, 1.0));
        typed(document.current_mut(), 0, "one two");
        assert!(session.typed(&document, 2.0));
        assert!(!session.typed(&document, 3.0));
        let progress = session.progress().unwrap();
        assert_eq!((progress.words, progress.reached), (2, true));

        session.set_goal(Some(Goal::Minutes(1)));
        assert!(!session.progress().unwrap().reached);
        assert!(!session.typed(&document, 50.0));
        assert!(session.typed(&document, 70.0));
    }

    #[test]
    fn a_goal_already_met_is_reached_quietly() {
        let mut document = Document::new(Page::new(40, 10));
        let mut session = Session::start(&document);
        typed(document.current_mut(), 0, "one two three");
        session.typed(&document, 1.0);
        session.set_goal(Some(Goal::Words(2)));
        assert!(session.progress().unwrap().reached);
        assert!(!session.typed(&document, 2.0));
    }

    #[test]
    fn the_goal_plate_cycles_through_the_presets() {
        let cycle = Goal::cycle(None);
        let mut goal = None;
        let mut seen = Vec::new();
        loop {
            goal = Goal::next(goal, &cycle);
            match goal {
                Some(g) => seen.push(g),
                None => break,
            }
        }
        assert_eq!(seen.len(), 6);
        assert_eq!(seen[1], Goal::Words(500));
        assert_eq!(seen[4], Goal::Minutes(25));
    }

    #[test]
    fn the_custom_goal_ends_the_cycle_unless_it_is_a_preset() {
        let words = Goal::custom(Some(2000), None);
        assert_eq!(Goal::cycle(words)[6], Goal::Words(2000));
        assert_eq!(Goal::cycle(Goal::custom(None, Some(25))).len(), 6);
        let both = Goal::custom(Some(2000), Some(25));
        assert_eq!(
            Goal::cycle(both).last(),
            Some(&Goal::WordsOrMinutes {
                words: 2000,
                minutes: 25
            })
        );
        // A custom goal since turned off moves on to the first.
        assert_eq!(
            Goal::next(words, &Goal::cycle(None)),
            Some(Goal::Words(250))
        );
    }

    #[test]
    fn words_or_minutes_is_reached_by_whichever_comes_first() {
        let mut document = Document::new(Page::new(40, 10));
        let mut session = Session::start(&document);
        session.set_goal(Goal::custom(Some(3), Some(1)));
        typed(document.current_mut(), 0, "one two");
        assert!(!session.typed(&document, 0.0));
        assert!(!session.typed(&document, 50.0));
        assert!(
            session.typed(&document, 61.0),
            "a minute before three words"
        );
        let progress = session.progress().unwrap();
        assert_eq!((progress.words, progress.minutes), (2, 1));
    }

    #[test]
    fn a_day_back_at_zero_leaves_the_log() {
        let mut log = WritingLog::default();
        let day = jiff::civil::date(2026, 9, 30);
        log.add(day, 12);
        log.add(day, -12);
        assert!(log.days().is_empty());
    }
}
