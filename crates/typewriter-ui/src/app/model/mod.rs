//! The model: the app's state and decisions, without a window. It takes
//! [`Intent`]s, changes, and asks the window for [`Effect`]s; the window
//! draws it and does what it asks. Tested without a window, sound or disk.

mod feeding;
mod folder;
mod leaving;
mod project;
mod settings;
#[cfg(test)]
pub mod testing;
mod typing;

pub use feeding::Feed;
pub use leaving::{Answer, Leaving};
pub use project::Project;

use std::collections::HashSet;

use typewriter_core::{Command, Typewriter};

use super::intent::{Effect, Intent};
use crate::filing::Filing;
use crate::machines::Machines;
use crate::picker::Dialog;
use crate::render::feed::FeedMotion;
use crate::render::notice::Notice;
use crate::render::{COURIER_PRIME, font_characters, holder, pad};
use crate::settings::{Settings, ZOOM_DEFAULT, ZOOM_MIN};
use crate::stage::Return;

/// What fills the window.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum View {
    Typing,
    Folder,
    /// A finished sheet, open, by index.
    Sheet(usize),
    Settings,
}

/// What lies open over the view: gone when another project comes in.
#[derive(Debug, Default)]
pub struct Overlays {
    /// Text fields in progress; each keeps keys from the machine.
    pub renaming: Option<String>,
    pub annotating: Option<String>,
    pub renumbering: Option<String>,
    /// The notebook, open: where the pencil is.
    pub notebook: Option<pad::Writing>,
    /// A finished sheet on the copy holder. Not saved.
    pub holder: Option<holder::Holder>,
    /// The sheet awaiting a yes to scrunch.
    pub confirm_scrunch: Option<usize>,
    /// The writing log is up close.
    pub log_open: bool,
}

impl Overlays {
    /// A text field or question has the keys.
    fn has_keys(&self) -> bool {
        self.renaming.is_some()
            || self.annotating.is_some()
            || self.renumbering.is_some()
            || self.notebook.is_some()
            || self.confirm_scrunch.is_some()
    }
}

pub struct Model {
    pub project: Project,
    pub overlays: Overlays,
    pub view: View,
    pub calm: bool,
    /// The chosen sheet in the folder.
    pub selected: usize,
    /// Months the writing log is turned back from today's.
    log_back: u32,
    pub zoom_percent: u16,
    /// The mode's furthest out.
    pub zoom_min: u16,
    /// Scroll not yet turned into zoom steps.
    scroll_zoom: f32,
    /// Knob turn not yet a whole notch: wheel or drag points, up positive.
    knob_turn: f32,
    /// The last strike from the keyboard, for type jams.
    last_strike: f64,
    /// When the platen knob last turned, for its guides.
    knob_turned: f64,
    pub last_return: Return,
    pub feed: Feed,
    /// The leave dialog is open for this.
    pub leaving: Option<Leaving>,
    /// Leave once Save As has saved the draft.
    leaving_after_save_as: Option<Leaving>,
    /// What Courier Prime can print: the machine's keys.
    typeface: HashSet<char>,
    pub settings: Settings,
    pub machines: Machines,
    pub notice: Notice,
    effects: Vec<Effect>,
}

impl Model {
    /// The first project in: a file already there was reopened, and its
    /// sheet winds back to where typing stopped.
    pub fn new(
        mut machine: Typewriter,
        filing: Filing,
        settings: Settings,
        machines: Machines,
        feed_motion: FeedMotion,
    ) -> Self {
        let wind_back = filing.is_saved_somewhere().then(|| machine.reinsert());
        Self {
            project: Project::new(machine, filing, settings.goals.goal),
            overlays: Overlays::default(),
            view: View::Typing,
            calm: false,
            selected: 0,
            log_back: 0,
            zoom_percent: settings.look.zoom_percent,
            zoom_min: ZOOM_MIN,
            scroll_zoom: 0.0,
            knob_turn: 0.0,
            last_strike: f64::NEG_INFINITY,
            knob_turned: f64::NEG_INFINITY,
            last_return: Return::NONE,
            feed: Feed::new(feed_motion, wind_back),
            leaving: None,
            leaving_after_save_as: None,
            typeface: font_characters(COURIER_PRIME),
            settings,
            machines,
            notice: Notice::default(),
            effects: Vec::new(),
        }
    }

    pub fn update(&mut self, intent: Intent, now: f64) {
        match intent {
            Intent::Input {
                keys,
                wheel,
                over_knob,
            } => self.input(keys, wheel, over_knob, now),
            Intent::DragKnob { points, per_notch } => self.turn_knob(points, per_notch, now),
            Intent::ReleaseKnob => self.knob_turn = 0.0,
            // Locked while a sheet moves, as their keys are.
            Intent::OpenFolder
            | Intent::ToggleCalm
            | Intent::OpenSettings
            | Intent::NextSpacing
            | Intent::ResetZoom
            | Intent::NextCorrection
                if self.is_busy(now) => {}
            Intent::OpenFolder => self.open_folder(),
            Intent::OpenNotebook => self.open_notebook(),
            Intent::CloseNotebook => self.overlays.notebook = None,
            Intent::NotebookWritten => self.project.filing.changed(now),
            Intent::ToggleCalm => self.calm = !self.calm,
            Intent::OpenSettings => self.open_settings(),
            Intent::CloseSettings => self.view = View::Typing,
            Intent::SettingsEdited(before) => self.settings_edited(&before, now),
            Intent::ChooseTexture => self.effects.push(Effect::Ask(Dialog::Texture)),
            Intent::NextSpacing => self.next_spacing(now),
            Intent::ResetZoom => self.set_zoom(ZOOM_DEFAULT),
            Intent::NextCorrection => self.next_correction(now),
            Intent::NextGoal => self.next_goal(),
            Intent::Save => self.save_now(now),
            Intent::ReleaseMargins => self.apply(Command::MarginRelease, now),
            Intent::MoveMargin { side, column } => {
                self.apply(Command::MoveMargin { side, column }, now);
            }
            Intent::TakeHolderDown => self.overlays.holder = None,
            Intent::Folder(action) => self.folder_action(action, now),
            Intent::OpenSheet(index) => self.view = View::Sheet(index),
            Intent::OpenLog => self.open_log(),
            Intent::CloseLog => self.overlays.log_open = false,
            Intent::TurnLog(step) => self.turn_log(step),
            Intent::StartNote => self.start_note(),
            Intent::NoteWritten { sheet, note } => self.note_written(sheet, &note, now),
            Intent::ScrunchUp(index) => {
                self.overlays.confirm_scrunch = None;
                self.scrunch(index, now);
            }
            Intent::KeepSheet => self.overlays.confirm_scrunch = None,
            Intent::Leave(answer) => self.answer(answer, now),
            Intent::Picked(picked) => self.picked(picked, now),
            Intent::OpenFile(path) => self.open_file(path, now),
            Intent::CloseWindow => self.close_window(),
            Intent::WindowFullscreen(on) => self.settings.look.fullscreen = on,
        }
    }

    /// After the frame's input: fluid dries, the project autosaves, sheets
    /// move on.
    pub fn tick(&mut self, now: f64) {
        self.project.dry_fluid(now);
        let autosave = self.settings.saving.autosave;
        let autosaved = self
            .project
            .filing
            .autosave(&self.project.machine, now, autosave);
        self.tell(autosaved.err(), now);
        self.move_sheets(now);
        // A sheet no longer there: back to the folder.
        let count = self.project.machine.document().finished().len();
        if matches!(self.view, View::Sheet(i) if i >= count) {
            self.view = View::Folder;
        }
    }

    /// What the model asked of the window since last taken, in order.
    pub fn take_effects(&mut self) -> Vec<Effect> {
        std::mem::take(&mut self.effects)
    }

    /// A text field or question has the keys: the machine gets none.
    pub fn keys_to_fields(&self) -> bool {
        self.overlays.has_keys() || self.leaving.is_some()
    }

    /// Something moves or fades: draw again soon.
    pub fn is_animating(&self, now: f64) -> bool {
        self.guides_opacity(now) > 0.0
            || self.feed.is_moving()
            || self.feed.is_filing(now)
            || self.project.is_drying()
            || self.project.filing.is_animating(now)
            || self.notice.is_animating(now)
    }

    /// Shows `told` in the notice, if anything.
    fn tell(&mut self, told: Option<String>, now: f64) {
        if let Some(text) = told {
            self.notice.show(text, now);
        }
    }

    /// Writes if autosave would.
    fn keep(&mut self, now: f64) {
        let autosave = self.settings.saving.autosave;
        let kept = self
            .project
            .filing
            .keep(&self.project.machine, now, autosave);
        self.tell(kept.err(), now);
    }

    /// The Save command: writes now, or Save As for a draft.
    fn save_now(&mut self, now: f64) {
        if !self.project.filing.is_saved() {
            self.effects.push(Effect::Ask(Dialog::SaveAs));
            return;
        }
        let told = self.project.filing.save_now(&self.project.machine, now);
        self.notice.show(told, now);
    }
}

#[cfg(test)]
mod tests {
    use super::super::intent::Sound;
    use super::testing::{model, model_with, type_text};
    use super::*;
    use crate::picker::Dialog;
    use crate::storage;
    use std::fs;
    use typewriter_core::{Goal, LineSpacing};

    /// A model whose project is saved in its own folder of the test desktop.
    fn saved(tag: &str) -> (Model, std::path::PathBuf) {
        let path = storage::test_desktop().join(format!("model-{tag}/novel.typr"));
        let mut model = model();
        model.project.filing = Filing::at(path.clone());
        (model, path)
    }

    #[test]
    fn save_asks_where_a_draft_should_go() {
        let mut model = model();
        model.update(Intent::Save, 10.0);
        assert_eq!(model.take_effects(), [Effect::Ask(Dialog::SaveAs)]);
    }

    #[test]
    fn save_writes_a_saved_project_and_says_so() {
        let (mut model, path) = saved("save");
        type_text(&mut model, "typed", 10.0);
        model.take_effects();
        model.update(Intent::Save, 12.0);
        assert!(path.exists(), "written at once, pause or not");
        assert!(model.notice.is_animating(12.0), "the plate says \"Saved\"");
    }

    #[test]
    fn a_failing_autosave_reaches_the_notice() {
        let dir = storage::test_desktop().join("model-broken");
        fs::create_dir_all(&dir).unwrap();
        let blocker = dir.join("is-a-file");
        fs::write(&blocker, "x").unwrap();
        let mut model = model();
        model.project.filing = Filing::at(blocker.join("novel.typr"));
        model.take_effects();
        type_text(&mut model, "work", 10.0);
        model.take_effects();
        model.tick(20.0);
        assert!(model.notice.is_animating(20.0), "why the save failed");
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn a_release_ends_a_half_turned_drag() {
        let mut model = model();
        let half_line = model.project.machine.carriage().half_line;
        let drag = |model: &mut Model, points| {
            model.update(
                Intent::DragKnob {
                    points,
                    per_notch: 10.0,
                },
                10.0,
            );
        };
        drag(&mut model, 5.0);
        assert_eq!(model.project.machine.carriage().half_line, half_line);
        model.update(Intent::ReleaseKnob, 10.0);
        drag(&mut model, 5.0);
        assert_eq!(
            model.project.machine.carriage().half_line,
            half_line,
            "the released half-charge is gone"
        );
        drag(&mut model, 20.0);
        assert_eq!(
            model.project.machine.carriage().half_line,
            half_line + 2,
            "forwards rolls the sheet on"
        );
        drag(&mut model, -15.0);
        assert_eq!(
            model.project.machine.carriage().half_line,
            half_line + 1,
            "backwards rolls it up, one whole notch kept"
        );
    }

    #[test]
    fn the_settings_card_closes_back_to_the_machine() {
        let mut model = model();
        model.update(Intent::OpenSettings, 10.0);
        assert_eq!(model.view, View::Settings);
        model.take_effects();
        model.update(Intent::CloseSettings, 10.0);
        assert_eq!(model.view, View::Typing);
    }

    #[test]
    fn the_spacing_lever_has_a_handle_too() {
        let mut model = model();
        assert_eq!(
            model.project.machine.carriage().line_spacing,
            LineSpacing::Single
        );
        model.update(Intent::NextSpacing, 10.0);
        assert_eq!(
            model.project.machine.carriage().line_spacing,
            LineSpacing::OneAndHalf
        );
    }

    #[test]
    fn writing_in_the_notebook_dirtyies_the_project() {
        let mut model = model();
        assert!(!model.project.filing.has_unsaved_changes());
        model.update(Intent::NotebookWritten, 10.0);
        assert!(model.project.filing.has_unsaved_changes());
    }

    #[test]
    fn the_desktop_says_when_it_goes_fullscreen() {
        let mut model = model();
        model.update(Intent::WindowFullscreen(true), 10.0);
        assert!(model.settings.look.fullscreen);
    }

    #[test]
    fn the_log_opens_in_place_of_the_notebook_and_turns_nothin() {
        let mut model = model();
        model.update(Intent::OpenNotebook, 10.0);
        assert!(model.overlays.notebook.is_some());
        model.update(Intent::OpenLog, 10.0);
        assert!(model.overlays.notebook.is_none(), "the book closes");
        assert!(model.overlays.log_open);
        // An empty log has no months to turn between.
        model.update(Intent::TurnLog(-1), 10.0);
        model.update(Intent::TurnLog(1), 10.0);
        assert_eq!(model.log_back, 0);
        model.update(Intent::CloseLog, 10.0);
        assert!(!model.overlays.log_open);
    }

    #[test]
    fn the_goal_bell_rings_once_on_being_reached() {
        let mut settings = Settings::default();
        settings.goals.goal = Some(Goal::Words(1));
        let mut model = model_with(settings);
        type_text(&mut model, "ab cd", 10.0);
        let effects = model.take_effects();
        assert!(
            effects.contains(&Effect::Sound(Sound::Machine(typewriter_core::Event::Bell))),
            "crossing the goal rings"
        );
    }
}
