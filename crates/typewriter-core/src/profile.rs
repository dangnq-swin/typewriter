//! Machine profiles: pitch, line pitch, paper and default stops.

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::accents;

const MM_PER_INCH: f64 = 25.4;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Profile {
    pub name: String,
    /// Characters per inch (Pica = 10, Elite = 12).
    pub pitch_cpi: u16,
    pub lines_per_inch: u16,
    /// Columns before the right margin at which the bell rings.
    pub bell_columns_before_margin: u16,
    #[serde(default)]
    pub tab_stops: Vec<u16>,
    /// Accents that strike without moving the carriage (`accents::ACCENTS`).
    /// None: accented letters can't be typed.
    #[serde(default)]
    pub dead_keys: Vec<char>,
    pub paper: Paper,
    pub margins: Margins,
    #[serde(default)]
    pub sounds: Sounds,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Paper {
    pub width_mm: f64,
    pub height_mm: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Margins {
    pub left_column: u16,
    /// First column the carriage locks at; typing stops before it.
    pub right_column: u16,
    /// Blank lines above the first typed line of a fresh sheet.
    pub top_lines: u16,
}

/// Which mechanisms make a sound (e.g. the SM9's return is silent).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Sounds {
    #[serde(default = "audible")]
    pub carriage_return: bool,
    /// The platen ratchet when the paper is rolled by hand.
    #[serde(default = "audible")]
    pub line_feed: bool,
}

impl Default for Sounds {
    fn default() -> Self {
        Self {
            carriage_return: audible(),
            line_feed: audible(),
        }
    }
}

fn audible() -> bool {
    true
}

#[derive(Debug, Error)]
pub enum ProfileError {
    #[error("invalid profile TOML: {0}")]
    Parse(#[from] toml::de::Error),
    #[error("pitch and line pitch must be greater than zero")]
    ZeroPitch,
    #[error("paper dimensions must be positive")]
    InvalidPaper,
    #[error("margins must satisfy left < right <= {columns} columns (got {left}..{right})")]
    InvalidMargins { left: u16, right: u16, columns: u16 },
    #[error("top margin of {top_lines} lines does not fit on the page")]
    InvalidTopMargin { top_lines: u16 },
    #[error("tab stop {0} is outside the page")]
    InvalidTabStop(u16),
    #[error("dead key {0} is not one of ` \u{b4} ^ \u{a8} ~")]
    InvalidDeadKey(char),
}

impl Profile {
    pub fn from_toml_str(s: &str) -> Result<Self, ProfileError> {
        let profile: Self = toml::from_str(s)?;
        profile.validate()?;
        Ok(profile)
    }

    pub fn validate(&self) -> Result<(), ProfileError> {
        if self.pitch_cpi == 0 || self.lines_per_inch == 0 {
            return Err(ProfileError::ZeroPitch);
        }
        let paper_ok = |v: f64| v.is_finite() && v > 0.0;
        if !paper_ok(self.paper.width_mm) || !paper_ok(self.paper.height_mm) {
            return Err(ProfileError::InvalidPaper);
        }
        let columns = self.columns();
        let Margins {
            left_column: left,
            right_column: right,
            top_lines,
        } = self.margins;
        if left >= right || right > columns {
            return Err(ProfileError::InvalidMargins {
                left,
                right,
                columns,
            });
        }
        if top_lines.saturating_mul(2) >= self.half_lines() {
            return Err(ProfileError::InvalidTopMargin { top_lines });
        }
        if let Some(&stop) = self.tab_stops.iter().find(|&&s| s >= columns) {
            return Err(ProfileError::InvalidTabStop(stop));
        }
        if let Some(&key) = self.dead_keys.iter().find(|&&k| !accents::is_accent(k)) {
            return Err(ProfileError::InvalidDeadKey(key));
        }
        Ok(())
    }

    /// Whole character cells that fit across the paper.
    pub fn columns(&self) -> u16 {
        cells_across(self.paper.width_mm, self.pitch_cpi)
    }

    /// Vertical positions in half-line steps, the ratchet's notch (allows 1.5
    /// spacing).
    pub fn half_lines(&self) -> u16 {
        cells_across(self.paper.height_mm, self.lines_per_inch.saturating_mul(2))
    }
}

fn cells_across(length_mm: f64, per_inch: u16) -> u16 {
    let cells = (length_mm / MM_PER_INCH * f64::from(per_inch)).floor();
    // `as` saturates; paper is validated positive.
    cells as u16
}

#[cfg(test)]
mod tests {
    use super::*;

    const SM9: &str = include_str!("../../../profiles/olympia-sm9.toml");

    #[test]
    fn sm9_profile_parses_and_fits_a4_at_pica() {
        let p = Profile::from_toml_str(SM9).unwrap();
        assert_eq!(p.name, "Olympia SM9");
        assert_eq!(p.columns(), 82);
        assert_eq!(p.half_lines(), 140);
        assert!(!p.sounds.carriage_return);
        assert!(p.sounds.line_feed);
    }

    #[test]
    fn sounds_default_to_audible() {
        let s: String = SM9
            .lines()
            .take_while(|l| !l.starts_with("[sounds]"))
            .collect::<Vec<_>>()
            .join("\n");
        let p = Profile::from_toml_str(&s).unwrap();
        assert!(p.sounds.carriage_return);
        assert!(p.sounds.line_feed);
    }

    #[test]
    fn unknown_fields_are_rejected() {
        let s = format!("{SM9}\nfoo = 1\n");
        assert!(matches!(
            Profile::from_toml_str(&s),
            Err(ProfileError::Parse(_))
        ));
    }

    #[test]
    fn margins_outside_page_are_rejected() {
        let mut p = Profile::from_toml_str(SM9).unwrap();
        p.margins.right_column = 83;
        assert!(matches!(
            p.validate(),
            Err(ProfileError::InvalidMargins { .. })
        ));
        p.margins.right_column = 10;
        assert!(matches!(
            p.validate(),
            Err(ProfileError::InvalidMargins { .. })
        ));
    }

    #[test]
    fn zero_pitch_is_rejected() {
        let mut p = Profile::from_toml_str(SM9).unwrap();
        p.pitch_cpi = 0;
        assert!(matches!(p.validate(), Err(ProfileError::ZeroPitch)));
    }

    #[test]
    fn tab_stop_outside_page_is_rejected() {
        let mut p = Profile::from_toml_str(SM9).unwrap();
        p.tab_stops = vec![82];
        assert!(matches!(
            p.validate(),
            Err(ProfileError::InvalidTabStop(82))
        ));
    }
}
