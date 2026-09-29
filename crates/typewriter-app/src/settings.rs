//! User settings in `$XDG_CONFIG_HOME/typewriter/config.toml`
//! (`%APPDATA%\typewriter\config.toml` on Windows).
//!
//! Give every field a default: missing keys must load, unknown ones are
//! ignored.

use std::fs;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use typewriter_core::{Constraints, EraseMode, Goal};

use crate::storage;

/// Write once settings rest this long: a dragged slider mustn't write every
/// frame.
const WRITE_AFTER_SECONDS: f64 = 0.5;

pub const ZOOM_MIN: u16 = 50;
pub const ZOOM_MAX: u16 = 200;
pub const ZOOM_STEP: u16 = 10;
pub const ZOOM_DEFAULT: u16 = 100;
pub const VOLUME_MAX: u8 = 100;
pub const CALM_FALLOFF_LINES: std::ops::RangeInclusive<u8> = 1..=10;
pub const CALM_MINIMUM_PERCENT: std::ops::RangeInclusive<u8> = 0..=60;
/// Strong by default: only a line or two around the typing line stay legible.
pub const CALM_FALLOFF_LINES_DEFAULT: u8 = 4;
pub const CALM_MINIMUM_PERCENT_DEFAULT: u8 = 10;
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
    /// Write saved projects after each pause. Drafts are always kept.
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
    /// Percent.
    pub volume: u8,
    pub mute: bool,
    /// Strikes, space bar, backspace, tab.
    pub keys: bool,
    /// Margin bell and goal bell.
    pub bell: bool,
    /// Ratchet clicks and carriage return.
    pub platen: bool,
    /// Sheet feed and scrunching up.
    pub sheet_feed: bool,
    /// Eraser and fluid.
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
    /// Shared with F11.
    pub fullscreen: bool,
    /// Calm mode: lines from the typing line to the faintest ink.
    pub calm_falloff_lines: u8,
    /// Calm mode: the faintest ink, percent.
    pub calm_minimum_percent: u8,
    /// As the mouse wheel left it.
    pub zoom_percent: u16,
    /// Type-line guides while the platen knob turns.
    pub platen_guides: bool,
}

impl Default for Look {
    fn default() -> Self {
        Self {
            ink_realism: true,
            carriage_travel: true,
            fullscreen: true,
            calm_falloff_lines: CALM_FALLOFF_LINES_DEFAULT,
            calm_minimum_percent: CALM_MINIMUM_PERCENT_DEFAULT,
            zoom_percent: ZOOM_DEFAULT,
            platen_guides: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Goals {
    /// The custom goal, last in the plate's cycle. Both on: whichever first.
    pub custom_words_on: bool,
    pub custom_words: u32,
    pub custom_minutes_on: bool,
    pub custom_minutes: u32,
    /// The plate's goal, carried to the next session.
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
    /// New projects' profile name.
    pub profile: String,
    pub rules: Rules,
}

impl Default for Machine {
    fn default() -> Self {
        Self {
            profile: crate::machines::DEFAULT.to_owned(),
            rules: Rules::default(),
        }
    }
}

/// New projects start with these; a change applies to the project in the
/// machine too. A reopened project keeps its own.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Rules {
    pub backspace: bool,
    pub lock_at_right_margin: bool,
    pub free_movement: bool,
    /// Digital delete joins the correction cycle.
    pub delete_in_cycle: bool,
    pub type_jams: bool,
}

impl Default for Rules {
    fn default() -> Self {
        let machine = Constraints::default();
        Self {
            backspace: machine.backspace,
            lock_at_right_margin: machine.lock_at_right_margin,
            free_movement: machine.free_movement,
            delete_in_cycle: false,
            type_jams: machine.type_jams,
        }
    }
}

impl Rules {
    /// A new project's rules, correcting with `erase`.
    pub fn constraints(&self, erase: EraseMode) -> Constraints {
        Constraints {
            backspace: self.backspace,
            erase,
            lock_at_right_margin: self.lock_at_right_margin,
            free_movement: self.free_movement,
            type_jams: self.type_jams,
        }
    }

    /// Puts the rules changed since `before` into `machine`, leaving the
    /// rest as the project has them. False if none changed.
    pub fn apply_changes(&self, before: &Self, machine: &mut Constraints) -> bool {
        if self == before {
            return false;
        }
        if self.backspace != before.backspace {
            machine.backspace = self.backspace;
        }
        if self.lock_at_right_margin != before.lock_at_right_margin {
            machine.lock_at_right_margin = self.lock_at_right_margin;
        }
        if self.free_movement != before.free_movement {
            machine.free_movement = self.free_movement;
        }
        if self.type_jams != before.type_jams {
            machine.type_jams = self.type_jams;
        }
        // Out of the cycle: no longer the method either.
        if !self.delete_in_cycle && machine.erase == EraseMode::Delete {
            machine.erase = EraseMode::Paper;
        }
        true
    }
}

impl Settings {
    /// Clamps hand-edited values to what the controls allow.
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
    /// What this run believes is in the file.
    written: Settings,
    /// When the settings started differing from `written`.
    changed_at: Option<f64>,
    /// Unreadable file: move it aside before the first write, never overwrite.
    unreadable: bool,
}

impl SettingsFile {
    /// Loads the settings (defaults on failure), plus a notice if one failed.
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

    /// Writes `settings` once settled. `force` writes now (on quit).
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
        // On failure, retry only after the next change.
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
    fn a_changed_rule_reaches_the_project_and_the_others_stay_its_own() {
        let before = Rules::default();
        let mut project = Constraints {
            lock_at_right_margin: false,
            erase: EraseMode::Delete,
            ..Constraints::default()
        };
        let after = Rules {
            free_movement: true,
            ..before.clone()
        };
        assert!(after.apply_changes(&before, &mut project));
        assert!(project.free_movement);
        assert!(
            !project.lock_at_right_margin,
            "not changed: the project's own"
        );
        assert_eq!(
            project.erase,
            EraseMode::Paper,
            "delete is out of the cycle"
        );
        assert!(!after.apply_changes(&after, &mut project));
    }

    #[test]
    fn rules_sit_in_their_own_table() {
        let mut settings = Settings::default();
        settings.machine.rules.delete_in_cycle = true;
        let text = toml::to_string(&settings).unwrap();
        assert!(text.contains("[machine.rules]\n"), "{text}");
        let loaded = Settings::from_toml("[machine.rules]\nbackspace = false\n").unwrap();
        assert!(!loaded.machine.rules.backspace);
        assert!(loaded.machine.rules.lock_at_right_margin);
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
