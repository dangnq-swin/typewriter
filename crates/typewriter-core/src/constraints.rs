//! How strictly the machine behaves like a real typewriter.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Constraints {
    /// Backspace steps back without erasing; the next strike overtypes.
    pub backspace: bool,
    pub erase: EraseMode,
    /// Lock at the right margin until the margin release is used.
    pub lock_at_right_margin: bool,
    /// Move the carriage and platen freely (release lever, platen knob), not
    /// only by typing, backspace, tab and return.
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

/// How mistakes are fixed. Corrections cover the strike; it stays in the stack.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum EraseMode {
    Off,
    /// Correction slip before the ribbon: Erase puts it in or out; strikes
    /// meanwhile leave chalk.
    Paper,
    /// Rub out the character before the carriage.
    Eraser,
    /// Dab fluid on the character before the carriage. Smudges strikes until dry.
    Fluid,
}

impl EraseMode {
    /// The next method in the cycle. Off is never in it.
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
