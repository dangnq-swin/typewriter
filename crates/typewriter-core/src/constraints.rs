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
            erase: EraseMode::Paper,
            lock_at_right_margin: true,
            free_movement: false,
        }
    }
}

/// How mistakes are fixed. Corrections cover what was struck, which stays
/// in the cell's stack.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum EraseMode {
    Off,
    /// A correction slip held in front of the ribbon: Erase puts it in and
    /// takes it out, and characters struck meanwhile are covered in chalk.
    Paper,
    /// A typewriter eraser rubs out the character before the carriage.
    Eraser,
    /// Correction fluid is dabbed on the character before the carriage. It
    /// smudges what is struck on it until it dries.
    Fluid,
}

impl EraseMode {
    /// The next method, for a control that cycles through them. Off is
    /// not one of them.
    pub fn next(self) -> Self {
        match self {
            Self::Paper => Self::Eraser,
            Self::Eraser => Self::Fluid,
            Self::Fluid | Self::Off => Self::Paper,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn correction_methods_cycle_without_off() {
        assert_eq!(EraseMode::Paper.next(), EraseMode::Eraser);
        assert_eq!(EraseMode::Eraser.next(), EraseMode::Fluid);
        assert_eq!(EraseMode::Fluid.next(), EraseMode::Paper);
        assert_eq!(EraseMode::Off.next(), EraseMode::Paper);
    }
}
