//! How strictly the machine behaves like a real typewriter.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Constraints {
    /// Backspace moves the carriage back one column without erasing, so the
    /// next strike lands on top of the previous one.
    pub backspace: bool,
    pub erase: EraseMode,
    /// The carriage locks at the right margin until the margin release is used.
    pub lock_at_right_margin: bool,
    /// Move the carriage and roll the platen freely (carriage release lever,
    /// platen knob) instead of only via typing, backspace, tab and return.
    pub free_movement: bool,
}

impl Default for Constraints {
    fn default() -> Self {
        Self {
            backspace: true,
            erase: EraseMode::Digital,
            lock_at_right_margin: true,
            free_movement: false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum EraseMode {
    Off,
    /// Removes the character as if it had never been typed.
    Digital,
    /// Covers the character with correction fluid. It stays in the glyph stack.
    WhiteOut,
    /// Covers the character with lift-off correction tape.
    CorrectionTape,
}
