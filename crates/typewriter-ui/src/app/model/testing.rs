//! Desks for the tests: the settings' machine on a draft kept nowhere, the
//! built-in machines, an even feed. Nothing is written to disk.

use typewriter_core::{Command, Constraints, Typewriter};

use super::Model;
use crate::app::intent::Intent;
use crate::filing::Filing;
use crate::input::Action;
use crate::machines::Machines;
use crate::render::feed::FeedMotion;
use crate::settings::Settings;

/// Past any feed in the tests.
const SETTLED: f64 = 5.0;

/// Its first sheet not yet wound in.
pub fn fresh_with(settings: Settings) -> Model {
    let machines = Machines::built_in().unwrap();
    let profile = machines.for_new(&settings.machine.profile);
    let constraints = settings
        .machine
        .rules
        .constraints(Constraints::default().erase);
    let machine = Typewriter::new(profile, constraints).unwrap();
    Model::new(
        machine,
        Filing::nowhere(),
        settings,
        machines,
        FeedMotion::even(1.0),
    )
}

pub fn fresh() -> Model {
    fresh_with(Settings::default())
}

/// Ready to type from time [`SETTLED`], its first sheet in.
pub fn model_with(settings: Settings) -> Model {
    let mut model = fresh_with(settings);
    model.start_frame(0.0);
    model.tick(SETTLED);
    model.take_effects();
    model
}

pub fn model() -> Model {
    model_with(Settings::default())
}

/// `keys` in one frame at `now`.
pub fn press(model: &mut Model, keys: &[Action], now: f64) {
    let keys = keys.to_vec();
    model.update(
        Intent::Input {
            keys,
            wheel: 0.0,
            over_knob: false,
        },
        now,
    );
}

/// `text`, a key a frame, a tenth of a second apart from `now`.
pub fn type_text(model: &mut Model, text: &str, now: f64) {
    for (i, c) in text.chars().enumerate() {
        let at = now + 0.1 * i as f64;
        press(model, &[Action::Machine(Command::Type(c))], at);
    }
}
