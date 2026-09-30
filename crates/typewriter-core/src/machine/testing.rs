//! Machines for the tests.

use super::{Command, Event, Typewriter};
use crate::constraints::Constraints;
use crate::profile::Profile;

const SM9: &str = include_str!("../../../../profiles/olympia-sm9.toml");

pub(super) fn sm9() -> Typewriter {
    let profile = Profile::from_toml_str(SM9).unwrap();
    Typewriter::new(profile, Constraints::default()).unwrap()
}

/// The SM9 with dead ´ and ^, as on some national keyboards.
pub(super) fn with_dead_keys() -> Typewriter {
    let mut profile = Profile::from_toml_str(SM9).unwrap();
    profile.dead_keys = vec!['\u{b4}', '^'];
    Typewriter::new(profile, Constraints::default()).unwrap()
}

pub(super) fn type_str(tw: &mut Typewriter, s: &str) -> Vec<Event> {
    s.chars().flat_map(|c| tw.apply(Command::Type(c))).collect()
}
