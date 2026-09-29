//! The user's settings, kept in `$XDG_CONFIG_HOME/typewriter/config.toml`.
//!
//! Every field has a default, so a missing file, section or key is fine, and
//! keys from a newer version are ignored.

use std::fs;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use typewriter_core::Goal;

use crate::render::calm;
use crate::storage;

/// Written once the settings have stopped changing for this long, so
/// dragging a slider does not write on every frame.
const WRITE_AFTER_SECONDS: f64 = 0.5;

pub const ZOOM_MIN: u16 = 50;
pub const ZOOM_MAX: u16 = 200;
pub const ZOOM_STEP: u16 = 10;
pub const ZOOM_DEFAULT: u16 = 100;
pub const VOLUME_MAX: u8 = 100;
pub const CALM_FALLOFF_LINES: std::ops::RangeInclusive<u8> = 1..=10;
pub const CALM_MINIMUM_PERCENT: std::ops::RangeInclusive<u8> = 0..=60;
pub const CUSTOM_WORDS_MAX: u32 = 100_000;
pub const CUSTOM_MINUTES_MAX: u32 = 600;

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub sound: Sound,
    pub look: Look,
    pub goals: Goals,
    pub machine: Machine,
    pub saving: Saving,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Saving {
    /// A saved project is written after each pause. Drafts are always kept.
    pub autosave: bool,
}

impl Default for Saving {
    fn default() -> Self {
        Self { autosave: true }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Sound {
    /// 0 to 100 %.
    pub volume: u8,
    pub mute: bool,
    /// Key strikes, space bar, backspace and tab.
    pub keys: bool,
    /// The margin bell, and the bell for a reached goal.
    pub bell: bool,
    /// Platen ratchet clicks and the carriage return, where the machine
    /// makes them.
    pub platen: bool,
    /// Sheets fed, and finished ones scrunched up.
    pub sheet_feed: bool,
    /// Eraser and correction fluid.
    pub corrections: bool,
    pub blocked: bool,
}

impl Default for Sound {
    fn default() -> Self {
        Self {
            volume: VOLUME_MAX,
            mute: false,
            keys: true,
            bell: true,
            platen: true,
            sheet_feed: true,
            corrections: true,
            blocked: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Look {
    pub ink_realism: bool,
    /// The paper slides sideways with the carriage.
    pub carriage_travel: bool,
    /// The window fills the screen. F11 switches it too.
    pub fullscreen: bool,
    /// Lines away from the typing line at which calm mode's dimming is
    /// strongest.
    pub calm_falloff_lines: u8,
    /// The faintest a dimmed line gets, in percent of full ink.
    pub calm_minimum_percent: u8,
    /// Remembered as the mouse wheel leaves it.
    pub zoom_percent: u16,
}

impl Default for Look {
    fn default() -> Self {
        Self {
            ink_realism: true,
            carriage_travel: true,
            fullscreen: true,
            calm_falloff_lines: calm::DEFAULT_FALLOFF_LINES,
            calm_minimum_percent: calm::DEFAULT_MINIMUM_PERCENT,
            zoom_percent: ZOOM_DEFAULT,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Goals {
    /// The user's own goal, ending the Goal plate's cycle: a word target, a
    /// time target, or both (reached by whichever comes first).
    pub custom_words_on: bool,
    pub custom_words: u32,
    pub custom_minutes_on: bool,
    pub custom_minutes: u32,
    /// The goal chosen on the Goal plate, kept for the next session.
    pub goal: Option<Goal>,
}

impl Default for Goals {
    fn default() -> Self {
        Self {
            custom_words_on: false,
            custom_words: 750,
            custom_minutes_on: false,
            custom_minutes: 30,
            goal: None,
        }
    }
}

impl Goals {
    pub fn custom(&self) -> Option<Goal> {
        Goal::custom(
            self.custom_words_on.then_some(self.custom_words),
            self.custom_minutes_on.then_some(self.custom_minutes),
        )
    }

    pub fn cycle(&self) -> Vec<Goal> {
        Goal::cycle(self.custom())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Machine {
    /// The machine new projects are typed on, by profile name.
    pub profile: String,
}

impl Default for Machine {
    fn default() -> Self {
        Self {
            profile: crate::machines::DEFAULT.to_owned(),
        }
    }
}

impl Settings {
    /// Brings hand-edited values back within what the controls allow.
    fn within_limits(mut self) -> Self {
        let sound = &mut self.sound;
        sound.volume = sound.volume.min(VOLUME_MAX);
        let look = &mut self.look;
        look.calm_falloff_lines = look
            .calm_falloff_lines
            .clamp(*CALM_FALLOFF_LINES.start(), *CALM_FALLOFF_LINES.end());
        look.calm_minimum_percent = look
            .calm_minimum_percent
            .clamp(*CALM_MINIMUM_PERCENT.start(), *CALM_MINIMUM_PERCENT.end());
        let zoom = look.zoom_percent.clamp(ZOOM_MIN, ZOOM_MAX);
        look.zoom_percent = zoom - (zoom - ZOOM_MIN) % ZOOM_STEP;
        let goals = &mut self.goals;
        goals.custom_words = goals.custom_words.clamp(1, CUSTOM_WORDS_MAX);
        goals.custom_minutes = goals.custom_minutes.clamp(1, CUSTOM_MINUTES_MAX);
        self
    }

    fn from_toml(text: &str) -> Result<Self, toml::de::Error> {
        toml::from_str::<Self>(text).map(Self::within_limits)
    }
}

/// Keeps `config.toml` in step with the settings.
pub struct SettingsFile {
    path: Option<PathBuf>,
    /// What is in the file, as far as this run knows.
    written: Settings,
    /// When the settings last differed from `written`.
    changed_at: Option<f64>,
    /// The file could not be read. It is kept aside, not overwritten.
    unreadable: bool,
}

impl SettingsFile {
    /// Loads the settings, falling back to the defaults. Also says what went
    /// wrong, if anything.
    pub fn load() -> (Settings, Self, Option<String>) {
        let path = storage::config_path();
        let mut file = Self {
            path: path.clone(),
            written: Settings::default(),
            changed_at: None,
            unreadable: false,
        };
        let Some(path) = path else {
            return (Settings::default(), file, None);
        };
        let text = match fs::read_to_string(&path) {
            Ok(text) => text,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
                return (Settings::default(), file, None);
            }
            Err(err) => {
                file.unreadable = true;
                let trouble = format!("Settings unreadable, using the defaults: {err}");
                return (Settings::default(), file, Some(trouble));
            }
        };
        match Settings::from_toml(&text) {
            Ok(settings) => {
                file.written = settings.clone();
                (settings, file, None)
            }
            Err(err) => {
                file.unreadable = true;
                let trouble = format!(
                    "{} has a mistake, using the defaults: {}",
                    storage::home_relative(&path),
                    err.message()
                );
                (Settings::default(), file, Some(trouble))
            }
        }
    }

    /// Writes `settings` once they have settled. `force` writes any change
    /// now, e.g. on quit.
    pub fn keep(&mut self, settings: &Settings, now: f64, force: bool) -> Result<(), String> {
        if *settings == self.written {
            self.changed_at = None;
            return Ok(());
        }
        let since = *self.changed_at.get_or_insert(now);
        if !force && now - since < WRITE_AFTER_SECONDS {
            return Ok(());
        }
        self.changed_at = None;
        // Not tried again until the next change.
        self.written = settings.clone();
        let Some(path) = &self.path else {
            return Ok(());
        };
        if self.unreadable {
            self.unreadable = false;
            let aside = path.with_extension("toml.unreadable");
            fs::rename(path, &aside).map_err(|err| format!("Could not save settings: {err}"))?;
        }
        let text =
            toml::to_string(settings).map_err(|err| format!("Could not save settings: {err}"))?;
        storage::write_atomic(path, format!("# Typewriter settings\n\n{text}"))
            .map_err(|err| format!("Could not save settings: {err}"))
    }

    pub fn is_pending(&self) -> bool {
        self.changed_at.is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_round_trip_through_toml() {
        let mut settings = Settings::default();
        settings.sound.volume = 40;
        settings.sound.bell = false;
        settings.goals.goal = Some(Goal::Minutes(25));
        settings.goals.custom_words_on = true;
        settings.goals.custom_words = 1200;
        settings.look.zoom_percent = 130;
        let text = toml::to_string(&settings).unwrap();
        assert_eq!(Settings::from_toml(&text).unwrap(), settings);
        assert!(text.contains("[goals.goal]\nminutes = 25"), "{text}");
    }

    #[test]
    fn missing_keys_take_their_defaults_and_unknown_ones_are_ignored() {
        let settings = Settings::from_toml("[sound]\nmute = true\n[later]\nx = 1\n").unwrap();
        assert!(settings.sound.mute);
        assert_eq!(settings.sound.volume, VOLUME_MAX);
        assert_eq!(settings.look, Look::default());
    }

    #[test]
    fn hand_edited_values_are_brought_within_limits() {
        let text =
            "[look]\nzoom_percent = 333\ncalm_falloff_lines = 0\ncalm_minimum_percent = 90\n";
        let settings = Settings::from_toml(text).unwrap();
        assert_eq!(settings.look.calm_minimum_percent, 60);
        assert_eq!(settings.look.zoom_percent, 200);
        assert_eq!(settings.look.calm_falloff_lines, 1);
        let odd = Settings::from_toml("[look]\nzoom_percent = 137\n").unwrap();
        assert_eq!(odd.look.zoom_percent, 130);
    }
}
