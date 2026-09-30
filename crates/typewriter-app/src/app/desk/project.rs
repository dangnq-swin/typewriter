//! The project in the machine: the typewriter, its file, this session's
//! words, and fluid drying on the sheet.

use std::collections::HashMap;

use typewriter_core::{Command, Goal, Session, Typewriter};

use crate::filing::Filing;
use crate::render::calendar;

/// Fluid smudges strikes until dry.
const FLUID_DRY_SECONDS: f64 = 3.0;

pub struct Project {
    pub machine: Typewriter,
    pub filing: Filing,
    pub session: Session,
    /// The session's words already in the writing log.
    logged_words: i64,
    /// Drying fluid on the current sheet: cell → when dabbed.
    wet: HashMap<(u16, u16), f64>,
}

impl Project {
    /// A session starts with it, aiming for `goal`.
    pub fn new(machine: Typewriter, filing: Filing, goal: Option<Goal>) -> Self {
        let mut session = Session::start(machine.document());
        session.set_goal(goal);
        Self {
            machine,
            filing,
            session,
            logged_words: 0,
            wet: HashMap::new(),
        }
    }

    /// Brings today's words in the log up to date with the session.
    pub fn record_words(&mut self) {
        let words = self.session.net_words();
        if words != self.logged_words {
            self.machine
                .log_words(calendar::today(), words - self.logged_words);
            self.logged_words = words;
        }
    }

    /// Another sheet is in: nothing on it is wet. A re-fed sheet leaves or
    /// rejoins the folder mid-way: the filed count alone can't tell.
    pub fn sheet_changed(&mut self) {
        self.wet.clear();
        self.session.recount(self.machine.document());
    }

    /// Fluid dabbed at the carriage, at `now`.
    pub fn dab(&mut self, now: f64) {
        let c = self.machine.carriage();
        self.wet.insert((c.half_line, c.column), now);
    }

    /// Tells the machine which fluid dabs have dried.
    pub fn dry_fluid(&mut self, now: f64) {
        let dried: Vec<(u16, u16)> = self
            .wet
            .iter()
            .filter(|&(_, &dabbed)| now - dabbed >= FLUID_DRY_SECONDS)
            .map(|(&cell, _)| cell)
            .collect();
        for (half_line, column) in dried {
            self.wet.remove(&(half_line, column));
            self.machine
                .apply(Command::FluidDried { half_line, column });
            self.filing.changed(now);
        }
    }

    pub fn is_drying(&self) -> bool {
        !self.wet.is_empty()
    }

    /// 1 just dabbed, falling to 0 as it dries.
    pub fn wetness(&self, now: f64, half_line: u16, column: u16) -> f32 {
        self.wet.get(&(half_line, column)).map_or(0.0, |&dabbed| {
            (1.0 - (now - dabbed) / FLUID_DRY_SECONDS).clamp(0.0, 1.0) as f32
        })
    }
}
