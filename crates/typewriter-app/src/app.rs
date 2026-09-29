use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use eframe::egui::{self, FontData, FontDefinitions, FontFamily, Painter, Pos2, Rect, pos2};
use typewriter_core::page::Page;
use typewriter_core::session::Totals;
use typewriter_core::{
    Command, Constraints, Direction, EraseMode, Event, Goal, Session, Typewriter,
};

use crate::audio::{self, Audio};
use crate::filing::{self, Filing, Keeping, Picked, WriteStatus};
use crate::input::{Action, Input};
use crate::machines::Machines;
use crate::render::background::Background;
use crate::render::calm::{self, Dimming};
use crate::render::feed::{self, FeedMotion};
use crate::render::folder::{FolderAction, ProjectLabel};
use crate::render::platen::{self, PlatenView};
use crate::render::{COURIER_PRIME, FONT_FAMILY, Metrics, folder, note, paper, ruler};
use crate::settings::{SettingsFile, ZOOM_DEFAULT, ZOOM_MAX, ZOOM_MIN, ZOOM_STEP};
use crate::{render, settings, storage};

/// Screen points per inch at 100% zoom.
const POINTS_PER_INCH: f32 = 96.0;
/// Scrolled distance that counts as one zoom step: about one wheel notch.
const SCROLL_POINTS_PER_STEP: f32 = 40.0;

/// Correction fluid smudges what is typed on it until it has dried.
const FLUID_DRY_SECONDS: f64 = 3.0;

/// A reopened sheet is wound back to where typing stopped a notch at a
/// time, at the pace of the ratchet clicks that wind a finished sheet out.
const WIND_BACK_NOTCH_SECONDS: f64 = 0.06;

/// Room beyond the window's edge for a moving sheet's shadow.
const SHADOW_ROOM: f32 = 30.0;

/// What fills the window.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum View {
    Typing,
    Folder,
    /// A finished sheet taken out of the folder, by index.
    Sheet(usize),
    Settings,
}

/// A reopened sheet being wound back down to the line typing stopped at.
struct WindBack {
    to_half_line: u16,
    /// When the next notch turns. Set once the sheet has wound in.
    next_notch: Option<f64>,
}

impl WindBack {
    fn to(to_half_line: u16) -> Self {
        Self {
            to_half_line,
            next_notch: None,
        }
    }
}

/// Where the machine goes once the project in it has been dealt with.
#[derive(Debug, Clone)]
enum Leaving {
    Quit,
    New,
    Open(PathBuf),
}

/// A finished sheet being scrunched up: its outline on screen when it went.
struct Scrunching {
    outline: [Pos2; 4],
    started: f64,
    seed: u64,
}

/// A new sheet being wound in, and the finished one rolling out.
struct Feeding {
    started: f64,
    /// The finished sheet and the half-line it was at, if there was one.
    outgoing: Option<(Page, u16)>,
    motion: FeedMotion,
}

pub struct TypewriterApp {
    machine: Typewriter,
    /// Where the project in the machine is saved.
    filing: Filing,
    /// Words and typing time since the project was put in, and the goal.
    session: Session,
    /// The new name being typed on the folder's tab.
    renaming: Option<String>,
    /// The note being pencilled on the open sheet.
    annotating: Option<String>,
    /// The chosen sheet's new number, being typed.
    renumbering: Option<String>,
    /// The finished sheet waiting for a yes before it is scrunched up.
    confirm_scrunch: Option<usize>,
    scrunching: Option<Scrunching>,
    /// Waiting for an answer about the draft or unsaved changes before
    /// leaving the project.
    leaving: Option<Leaving>,
    /// Leaving once the Save As dialog has saved the draft.
    leaving_after_save_as: Option<Leaving>,
    /// The window may close: whatever needed asking was answered.
    quitting: bool,
    /// Where the chosen sheet is drawn in the folder, as of the last frame.
    pulled: Option<[Pos2; 4]>,
    metrics: Metrics,
    platen: PlatenView,
    input: Input,
    background: Background,
    /// `None` without a working output device: the machine stays silent.
    audio: Option<Audio>,
    /// Winding a sheet in takes as long as its sound, and the machine takes
    /// no input until it is done.
    feed_motion: FeedMotion,
    feeding: Option<Feeding>,
    /// A project, new or reopened, starts by winding its sheet in.
    first_sheet_pending: bool,
    /// Then a reopened one winds down to where typing stopped.
    wind_back: Option<WindBack>,
    /// Cells of the sheet in the machine with correction fluid still drying,
    /// and when it was dabbed on.
    wet: HashMap<(u16, u16), f64>,
    view: View,
    /// Distraction-free: the chrome fades away and lines dim around the
    /// typing line.
    calm: bool,
    /// The sheet chosen in the folder, by index.
    selected: usize,
    zoom_percent: u16,
    scroll_zoom: f32,
    settings: settings::Settings,
    settings_file: SettingsFile,
    /// The machines a project can be typed on.
    machines: Machines,
}

impl TypewriterApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> anyhow::Result<Self> {
        install_fonts(&cc.egui_ctx);
        // No Ctrl shortcuts on a typewriter, egui's interface zoom included.
        cc.egui_ctx.options_mut(|o| o.zoom_with_keyboard = false);
        let (settings, settings_file, settings_trouble) = SettingsFile::load();
        let machines = Machines::load()?;
        let (mut machine, filing, trouble) = first_project(&machines, &settings.machine.profile)?;
        let zoom_percent = settings.look.zoom_percent;
        let metrics = Metrics::new(machine.profile(), points_per_inch(zoom_percent));
        let audio = Audio::new(machine.profile().sounds.clone(), settings.sound.clone())
            .inspect_err(|err| eprintln!("sound unavailable, typing silently: {err:#}"))
            .ok();
        let wind_back = filing.is_saved_somewhere().then(|| machine.reinsert());
        let mut filing = filing;
        let crashed = storage::mark_running();
        let recovered = (crashed && filing.is_saved_somewhere()).then(|| {
            "Recovered your work from when Typewriter last closed unexpectedly.".to_owned()
        });
        if let Some(notice) = trouble.or(settings_trouble).or(recovered) {
            filing.notify(notice, 0.0);
        }
        filing.remember();
        let mut session = Session::start(machine.document(), unix_now());
        session.set_goal(settings.goals.goal);
        Ok(Self {
            machine,
            filing,
            session,
            renaming: None,
            annotating: None,
            renumbering: None,
            confirm_scrunch: None,
            scrunching: None,
            leaving: None,
            leaving_after_save_as: None,
            quitting: false,
            pulled: None,
            metrics,
            platen: PlatenView::new(settings.look.carriage_travel),
            input: Input::default(),
            background: Background::load(&cc.egui_ctx),
            audio,
            feed_motion: audio::sheet_feed_motion(),
            feeding: None,
            first_sheet_pending: true,
            wind_back: wind_back.map(WindBack::to),
            wet: HashMap::new(),
            view: View::Typing,
            calm: false,
            selected: 0,
            zoom_percent,
            scroll_zoom: 0.0,
            settings,
            settings_file,
            machines,
        })
    }

    fn handle_input(&mut self, ctx: &egui::Context) {
        let now = ctx.input(|i| i.time);
        let (events, shift_down, scrolled) =
            ctx.input(|i| (i.events.clone(), i.modifiers.shift, i.smooth_scroll_delta.y));
        // Keys still go through the input state (e.g. Shift+Tab counting),
        // they just do nothing while a sheet is being wound in.
        // A name being typed on the folder's tab, or a note being written,
        // is not for the machine.
        if self.renaming.is_some()
            || self.annotating.is_some()
            || self.renumbering.is_some()
            || self.confirm_scrunch.is_some()
            || self.leaving.is_some()
        {
            return;
        }
        let actions = self.input.actions(&events, shift_down);
        // Esc closes an open menu (egui does that) before it closes a view.
        let menu_open = egui::Popup::is_any_open(ctx);
        if self.view == View::Settings {
            // Keys are for the card's fields. Esc closes it unless a field
            // is being edited (Esc leaves the field first).
            let editing = ctx.memory(|m| m.focused().is_some());
            for action in actions {
                match action {
                    Action::Fullscreen => toggle_fullscreen(ctx),
                    Action::Escape if !editing && !menu_open => self.escape(),
                    Action::Save => self.filing.save_now(&self.machine, ctx, now),
                    _ => {}
                }
            }
            return;
        }
        let feeding = self.is_busy(now);
        for action in actions {
            match action {
                // The window is not part of the machine.
                Action::Fullscreen => toggle_fullscreen(ctx),
                Action::Save => self.filing.save_now(&self.machine, ctx, now),
                _ if feeding => {}
                Action::Machine(command) => {
                    if !self.browse_command(command) {
                        self.apply(command, now);
                        self.typed(now);
                    }
                }
                Action::PageUp => self.page_up(),
                Action::PageDown => self.page_down(),
                Action::Escape if menu_open => {}
                Action::Escape => self.escape(),
                Action::NextCorrection => self.next_correction(now),
                Action::Delete if self.view == View::Folder => {
                    self.folder_action(FolderAction::Scrunch, ctx, now);
                }
                Action::Delete => {
                    self.apply(Command::Erase, now);
                    self.typed(now);
                }
                Action::ShiftArrow(direction) => match self.view {
                    View::Folder => self.move_chosen(direction, now),
                    _ => {
                        let command = Command::Move(direction);
                        if !self.browse_command(command) {
                            self.apply(command, now);
                            self.typed(now);
                        }
                    }
                },
            }
        }
        if feeding {
            return;
        }
        // The paper never scrolls freely, so the wheel zooms: up is closer.
        self.scroll_zoom += scrolled;
        if self.scroll_zoom.abs() >= SCROLL_POINTS_PER_STEP {
            self.zoom(if self.scroll_zoom > 0.0 { 1 } else { -1 });
            self.scroll_zoom = 0.0;
        }
    }

    /// Typing always goes to the machine, so it also brings the view back.
    fn apply(&mut self, command: Command, now: f64) {
        self.view = View::Typing;
        // The finished sheet is still seen rolling out after the machine
        // has filed it.
        let outgoing = (command == Command::FeedSheet).then(|| {
            (
                self.machine.page().clone(),
                self.machine.carriage().half_line,
            )
        });
        let mut page_end = false;
        self.filing.changed(now);
        for event in self.machine.apply(command) {
            match event {
                // Many keyboards have no Insert key, so a return on the last
                // line feeds the next sheet.
                Event::PageEnd => page_end = true,
                Event::Blocked(_) => self.platen.jolt(now),
                Event::Erase(EraseMode::Fluid) => {
                    let c = self.machine.carriage();
                    self.wet.insert((c.half_line, c.column), now);
                }
                Event::SheetFed => {
                    self.wet.clear();
                    let autosave = self.settings.saving.autosave;
                    self.filing.keep(&self.machine, now, autosave);
                    self.feeding = Some(Feeding {
                        started: now,
                        outgoing: outgoing.clone(),
                        motion: self.feed_motion.clone(),
                    });
                }
                _ => {}
            }
            if let Some(audio) = &mut self.audio {
                audio.play(event);
            }
        }
        if page_end {
            self.apply(Command::FeedSheet, now);
        }
    }

    /// Counts a key towards the session, ringing the bell once its goal is
    /// reached, and keeps the session's stats with the project.
    fn typed(&mut self, now: f64) {
        if self.session.typed(self.machine.document(), now)
            && let Some(audio) = &mut self.audio
        {
            audio.play(Event::Bell);
        }
        if let Some(stats) = self.session.stats() {
            self.machine.record_session(stats);
        }
    }

    fn next_goal(&mut self) {
        let goal = Goal::next(self.session.goal(), &self.settings.goals.cycle());
        self.session.set_goal(goal);
        self.settings.goals.goal = goal;
    }

    /// A custom goal being aimed for changes as it is edited.
    fn follow_custom_goal(&mut self, before: &settings::Goals) {
        let goals = &mut self.settings.goals;
        let (old, new) = (before.custom(), goals.custom());
        let is_preset = old.is_some_and(|old| Goal::cycle(None).contains(&old));
        if old != new && goals.goal == old && !is_preset {
            goals.goal = new;
        }
    }

    /// Puts changed settings into effect.
    fn apply_settings(&mut self) {
        self.platen.carriage_travel = self.settings.look.carriage_travel;
        if let Some(audio) = &mut self.audio {
            audio.set_settings(self.settings.sound.clone());
        }
        self.set_zoom(self.settings.look.zoom_percent);
        if self.session.goal() != self.settings.goals.goal {
            self.session.set_goal(self.settings.goals.goal);
        }
    }

    fn open_settings(&mut self) {
        // Profiles added since are listed too.
        match Machines::load() {
            Ok(machines) => self.machines = machines,
            Err(err) => eprintln!("could not reload the machines: {err:#}"),
        }
        self.view = View::Settings;
    }

    fn next_correction(&mut self, now: f64) {
        let next = self.machine.constraints.erase.next();
        self.apply(Command::SetEraseMode(next), now);
    }

    /// Tells the machine which dabs of fluid have dried by now.
    fn dry_fluid(&mut self, now: f64) {
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

    /// Puts another project in the machine, winding its sheet in. The one
    /// there is saved first.
    fn put_in(&mut self, mut machine: Typewriter, filing: Filing, reopened: bool, now: f64) {
        // Anything that needed asking about was answered before this.
        let autosave = self.settings.saving.autosave;
        self.filing.keep(&self.machine, now, autosave);
        self.wind_back = reopened.then(|| WindBack::to(machine.reinsert()));
        // A new session, still aiming for the same goal.
        let goal = self.session.goal();
        self.session = Session::start(machine.document(), unix_now());
        self.session.set_goal(goal);
        self.machine = machine;
        self.filing = filing;
        self.filing.remember();
        self.wet.clear();
        self.feeding = None;
        self.first_sheet_pending = true;
        self.view = View::Typing;
        self.selected = 0;
        self.renaming = None;
        self.annotating = None;
        self.renumbering = None;
        self.confirm_scrunch = None;
        self.scrunching = None;
        // Another machine may type at another pitch, on other paper.
        self.metrics = Metrics::new(self.machine.profile(), self.points_per_inch());
        self.platen.snap();
        if let Some(audio) = &mut self.audio {
            audio.set_machine(self.machine.profile().sounds.clone());
        }
    }

    fn folder_action(&mut self, action: FolderAction, ctx: &egui::Context, now: f64) {
        match action {
            FolderAction::Save => self.filing.save_now(&self.machine, ctx, now),
            FolderAction::SaveAs => self.filing.ask_save_as(ctx),
            FolderAction::Rename => self.renaming = Some(self.filing.name()),
            FolderAction::RenameTo(name) => {
                self.renaming = None;
                self.filing.rename(&self.machine, &name, now);
            }
            FolderAction::CancelRename => self.renaming = None,
            FolderAction::New => self.leave(Leaving::New, ctx, now),
            FolderAction::Open => self.filing.ask_open(ctx),
            FolderAction::Renumber => self.renumbering = Some(String::new()),
            FolderAction::RenumberTo(number) => {
                self.renumbering = None;
                self.renumber(&number, now);
            }
            FolderAction::CancelRenumber => self.renumbering = None,
            FolderAction::Scrunch => {
                if self.selected < self.machine.document().finished().len() {
                    self.confirm_scrunch = Some(self.selected);
                }
            }
            FolderAction::Export(format) => {
                let ink_realism = self.settings.look.ink_realism;
                self.filing.export(&self.machine, format, ink_realism, now);
            }
        }
    }

    /// Gives the chosen sheet the typed number, the others shifting along.
    fn renumber(&mut self, number: &str, now: f64) {
        let count = self.machine.document().finished().len();
        // A number that is not a sheet's changes nothing.
        if let Ok(n) = number.parse::<usize>()
            && (1..=count).contains(&n)
            && self.machine.renumber(self.selected, n - 1)
        {
            self.selected = n - 1;
            self.filing.changed(now);
        }
    }

    /// Shift+arrows move the chosen sheet one place: up or left is older.
    fn move_chosen(&mut self, direction: Direction, now: f64) {
        let count = self.machine.document().finished().len();
        let to = match direction {
            Direction::Up | Direction::Left => self.selected.checked_sub(1),
            Direction::Down | Direction::Right => Some(self.selected + 1).filter(|&to| to < count),
        };
        if let Some(to) = to
            && self.machine.renumber(self.selected, to)
        {
            self.selected = to;
            self.filing.changed(now);
        }
    }

    /// Scrunches up the finished sheet `index`, for good.
    fn scrunch(&mut self, index: usize, ctx: &egui::Context, now: f64) {
        if self.machine.scrunch(index).is_none() {
            return;
        }
        if let Some(outline) = self.pulled {
            self.scrunching = Some(Scrunching {
                outline,
                started: now,
                seed: index as u64 ^ now.to_bits(),
            });
        }
        if let Some(audio) = &self.audio {
            audio.play_crumple();
        }
        // The sheet that takes its place slides out, not starts out.
        ctx.animate_value_with_time(egui::Id::new(("folder-pull", index)), 0.0, 0.0);
        let count = self.machine.document().finished().len();
        self.selected = self.selected.min(count.saturating_sub(1));
        self.session.recount(self.machine.document());
        if let Some(stats) = self.session.stats() {
            self.machine.record_session(stats);
        }
        self.filing.changed(now);
    }

    /// "Scrunch up sheet N?", over everything, until answered.
    fn confirm_scrunch(&mut self, ctx: &egui::Context, now: f64) {
        let Some(index) = self.confirm_scrunch else {
            return;
        };
        let modal = egui::Modal::new(egui::Id::new("confirm-scrunch")).show(ctx, |ui| {
            ui.set_width(300.0);
            ui.heading(format!("Scrunch up sheet {}?", index + 1));
            ui.label("It can't be smoothed out again.");
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                let scrunch = ui.button("Scrunch up").clicked();
                let keep = ui.button("Keep it").clicked();
                (scrunch, keep)
            })
            .inner
        });
        let (scrunch, keep) = modal.inner;
        if scrunch {
            self.confirm_scrunch = None;
            self.scrunch(index, ctx, now);
        } else if keep || modal.should_close() {
            self.confirm_scrunch = None;
        }
    }

    fn take_picked(&mut self, ctx: &egui::Context, now: f64) {
        match self.filing.picked() {
            Some(Picked::SaveAs(path)) => {
                let saved = self.filing.save_as(&self.machine, path, now);
                if let Some(leaving) = self.leaving_after_save_as.take()
                    && saved
                {
                    self.go(leaving, ctx, now);
                }
            }
            Some(Picked::Open(path)) => self.leave(Leaving::Open(path), ctx, now),
            Some(Picked::Cancelled) => self.leaving_after_save_as = None,
            None => {}
        }
    }

    /// Something typed would be put away unsaved: a draft with work in it,
    /// or changes while autosave is off.
    fn must_ask(&self) -> bool {
        self.filing.is_draft_with_work(&self.machine)
            || (!self.settings.saving.autosave
                && self.filing.is_saved()
                && self.filing.has_unsaved_changes())
    }

    /// Leaves the project, asking first if work would be put away unsaved.
    fn leave(&mut self, leaving: Leaving, ctx: &egui::Context, now: f64) {
        if self.must_ask() {
            self.leaving = Some(leaving);
        } else {
            self.go(leaving, ctx, now);
        }
    }

    fn go(&mut self, leaving: Leaving, ctx: &egui::Context, now: f64) {
        match leaving {
            Leaving::Quit => {
                self.quitting = true;
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
            Leaving::New => {
                let profile = self.machines.for_new(&self.settings.machine.profile);
                let constraints = self.machine.constraints.clone();
                match Typewriter::new(profile, constraints) {
                    Ok(machine) => self.put_in(machine, Filing::draft(), false, now),
                    Err(err) => self.filing.notify(format!("No new project: {err}"), now),
                }
            }
            Leaving::Open(path) => match filing::open(&self.machines, &path) {
                Ok(machine) => self.put_in(machine, Filing::at(path), true, now),
                Err(err) => self
                    .filing
                    .notify(format!("Could not open that project: {err:#}"), now),
            },
        }
    }

    /// Asks what to do with a draft, or with unsaved changes, before
    /// leaving the project.
    fn leaving_dialog(&mut self, ctx: &egui::Context, now: f64) {
        let Some(leaving) = self.leaving.clone() else {
            return;
        };
        #[derive(Clone, Copy)]
        enum Answer {
            SaveAs,
            Save,
            Keep,
            Discard,
            DontSave,
            Cancel,
        }
        let draft = !self.filing.is_saved();
        let name = self.filing.name();
        let modal = egui::Modal::new(egui::Id::new("leaving")).show(ctx, |ui| {
            ui.set_width(340.0);
            if draft {
                ui.heading("Keep this draft?");
                ui.label("It has not been saved under a name.");
            } else {
                ui.heading(format!("Save changes to \u{201c}{name}\u{201d}?"));
            }
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                let buttons: &[(&str, Answer)] = if draft {
                    &[
                        ("Save As\u{2026}", Answer::SaveAs),
                        ("Keep as draft", Answer::Keep),
                        ("Discard", Answer::Discard),
                        ("Cancel", Answer::Cancel),
                    ]
                } else {
                    &[
                        ("Save", Answer::Save),
                        ("Don't save", Answer::DontSave),
                        ("Cancel", Answer::Cancel),
                    ]
                };
                let mut chosen = None;
                for &(label, answer) in buttons {
                    if ui.button(label).clicked() {
                        chosen = Some(answer);
                    }
                }
                chosen
            })
            .inner
        });
        let answer = modal
            .inner
            .or_else(|| modal.should_close().then_some(Answer::Cancel));
        let Some(answer) = answer else {
            return;
        };
        self.leaving = None;
        match answer {
            Answer::SaveAs => {
                self.leaving_after_save_as = Some(leaving);
                self.filing.ask_save_as(ctx);
            }
            Answer::Save => {
                self.filing.changed(now);
                self.filing.save(&self.machine, now);
                let failed = matches!(
                    self.filing.keeping(true, now),
                    Keeping::Autosave(WriteStatus::Failed(_))
                );
                if !failed {
                    self.go(leaving, ctx, now);
                }
            }
            Answer::Keep | Answer::DontSave => self.go(leaving, ctx, now),
            Answer::Discard => {
                self.filing.discard_draft();
                self.go(leaving, ctx, now);
            }
            Answer::Cancel => {}
        }
    }

    /// 1 for fluid just dabbed on, falling to 0 as it dries.
    fn wetness(&self, now: f64, half_line: u16, column: u16) -> f32 {
        self.wet.get(&(half_line, column)).map_or(0.0, |&dabbed| {
            (1.0 - (now - dabbed) / FLUID_DRY_SECONDS).clamp(0.0, 1.0) as f32
        })
    }

    /// The machine takes no input while a sheet goes in or winds back.
    fn is_busy(&self, now: f64) -> bool {
        self.is_feeding(now) || self.wind_back.is_some()
    }

    /// Turns the platen a notch at a time once a reopened sheet is in, with
    /// a ratchet click each, until it reaches the line typing stopped at.
    fn wind_back(&mut self, now: f64) {
        if self.feeding.is_some() {
            return;
        }
        let Some(wind) = &mut self.wind_back else {
            return;
        };
        let to = wind.to_half_line;
        let mut next = *wind.next_notch.get_or_insert(now);
        while now >= next && self.machine.carriage().half_line < to {
            self.apply(Command::PlatenNotch, now);
            next += WIND_BACK_NOTCH_SECONDS;
        }
        if self.machine.carriage().half_line >= to {
            self.wind_back = None;
        } else if let Some(wind) = &mut self.wind_back {
            wind.next_notch = Some(next);
        }
    }

    fn is_feeding(&self, now: f64) -> bool {
        self.feeding
            .as_ref()
            .is_some_and(|f| now - f.started < f.motion.duration())
    }

    /// There is no finished sheet to wind out yet, so the first sheet only
    /// winds in.
    fn load_first_sheet(&mut self, now: f64) {
        self.feeding = Some(Feeding {
            started: now,
            outgoing: None,
            motion: self.feed_motion.clone().after_wind_out(0.0),
        });
        if let Some(audio) = &mut self.audio {
            audio.play_wind_in();
        }
    }

    /// Zooms `steps` steps in (positive) or out (negative).
    fn zoom(&mut self, steps: i32) {
        self.set_zoom(zoomed(self.zoom_percent, steps));
    }

    fn set_zoom(&mut self, percent: u16) {
        self.settings.look.zoom_percent = percent;
        if percent != self.zoom_percent {
            self.zoom_percent = percent;
            self.metrics = Metrics::new(self.machine.profile(), self.points_per_inch());
            self.platen.snap();
        }
    }

    fn points_per_inch(&self) -> f32 {
        points_per_inch(self.zoom_percent)
    }

    /// Calm mode's dimming around `half_line`, `amount` of the way on.
    fn dimming(&self, half_line: u16, amount: f32) -> Dimming {
        let look = &self.settings.look;
        Dimming::calm(
            half_line,
            amount,
            look.calm_falloff_lines,
            look.calm_minimum_percent,
        )
    }

    /// Opens the folder with the newest sheet chosen.
    fn open_folder(&mut self) {
        self.view = View::Folder;
        self.selected = self.machine.document().finished().len().saturating_sub(1);
    }

    /// Moves through the finished sheets, `-1` towards older ones.
    fn browse(&mut self, step: isize) {
        let count = self.machine.document().finished().len();
        match self.view {
            View::Typing | View::Settings => {}
            View::Folder => self.selected = stepped(self.selected, step, count),
            View::Sheet(i) => self.view = View::Sheet(stepped(i, step, count)),
        }
    }

    /// In the folder and the open sheet, arrows and Enter browse instead of
    /// reaching the machine. Returns whether the command was used up.
    fn browse_command(&mut self, command: Command) -> bool {
        if self.view == View::Typing {
            return false;
        }
        match command {
            Command::Move(Direction::Up | Direction::Left) => self.browse(-1),
            Command::Move(Direction::Down | Direction::Right) => self.browse(1),
            // A held Enter that opened a sheet must not reach the machine.
            Command::LineFeed => {}
            Command::Return if self.view == View::Folder => {
                if !self.machine.document().finished().is_empty() {
                    self.view = View::Sheet(self.selected);
                }
            }
            _ => return false,
        }
        true
    }

    fn page_up(&mut self) {
        match self.view {
            View::Typing => self.open_folder(),
            View::Folder | View::Sheet(_) => self.browse(-1),
            // Keys do not reach here from the card.
            View::Settings => {}
        }
    }

    fn page_down(&mut self) {
        self.browse(1);
    }

    fn escape(&mut self) {
        self.view = match self.view {
            View::Typing => {
                self.calm = !self.calm;
                View::Typing
            }
            View::Folder | View::Settings => View::Typing,
            View::Sheet(i) => {
                self.selected = i;
                View::Folder
            }
        };
    }

    fn show_typing(&mut self, ui: &mut egui::Ui, now: f64) {
        let view = ui.max_rect();
        let carriage = self.machine.carriage();
        let cell = self
            .metrics
            .cell_offset(carriage.half_line, carriage.column);
        let layout = self.platen.layout(view, &self.metrics, cell, now);
        let painter = ui.painter_at(view);
        let calm = ui.ctx().animate_bool_with_time(
            egui::Id::new("calm-mode"),
            self.calm,
            calm::FADE_SECONDS,
        );
        let chrome = 1.0 - calm;
        let mut paper_origin = layout.paper_origin;
        let mut pointer_opacity = 1.0;
        if let Some(feeding) = &self.feeding {
            let t = now - feeding.started;
            let motion = &feeding.motion;
            if let (Some(rolled), Some((old_page, old_half_line))) =
                (motion.roll_out(t), &feeding.outgoing)
            {
                let old_y = layout.strike_point.y - self.metrics.cell_offset(*old_half_line, 0).y;
                let exit = old_y + self.metrics.paper_size.y - view.top() + SHADOW_ROOM;
                let old_origin = pos2(paper_origin.x, old_y - rolled * exit);
                self.paint_lifted(&painter, view, old_origin, 0.0, 1.0);
                let dimming = self.dimming(*old_half_line, calm);
                self.paint_page(&painter, old_page, old_origin, dimming, &paper::dry);
            }
            // Rises from below the window until its top margin reaches the
            // typing line.
            let placed = layout.strike_point.y - cell.y;
            let below = view.bottom() + SHADOW_ROOM;
            paper_origin.y = below + (placed - below) * motion.progress(t);
            self.paint_lifted(&painter, view, paper_origin, motion.curl(t), motion.lift(t));
            pointer_opacity = motion.pointer_opacity(t);
        }
        let dimming = self.dimming(carriage.half_line, calm);
        let wetness = |half_line, column| self.wetness(now, half_line, column);
        self.paint_page(
            &painter,
            self.machine.page(),
            paper_origin,
            dimming,
            &wetness,
        );
        if self.machine.slip_in() {
            platen::paint_slip(&painter, &self.metrics, layout.strike_point);
        }
        platen::paint_strike_marker(
            &painter,
            &self.metrics,
            layout.strike_point,
            pointer_opacity,
        );

        let feeding = self.is_busy(now);
        let finished = self.machine.document().finished().len();
        if folder::desk_icon(ui, view, finished, chrome) && !feeding {
            self.open_folder();
        }
        if calm::calm_icon(ui, view) && !feeding {
            self.calm = !self.calm;
        }
        if render::settings::gear_icon(ui, view, chrome) && !feeding {
            self.open_settings();
        }
        if chrome <= 0.0 {
            return;
        }
        let mut painter = painter;
        painter.multiply_opacity(chrome);
        let carriage = self.machine.carriage();
        let ruler_top = ruler::top(&self.metrics, layout.strike_point);
        ruler::paint_scale(
            &painter,
            &self.metrics,
            carriage,
            self.machine.page().columns(),
            layout.paper_origin.x,
            ruler_top,
        );
        let spacing_plate = ruler::paint_spacing_indicator(
            &painter,
            carriage.line_spacing,
            layout.paper_origin.x,
            ruler_top + ruler::HEIGHT,
        );
        let zoom_plate = ruler::paint_zoom_plate(&painter, self.zoom_percent, spacing_plate);
        let correction_plate = ruler::paint_correction_plate(
            &painter,
            self.machine.constraints.erase,
            self.machine.slip_in(),
            zoom_plate,
        );
        let goal_plate =
            ruler::paint_goal_plate(&painter, self.session.progress(), correction_plate);
        let keeping = self.filing.keeping(self.settings.saving.autosave, now);
        let autosave_plate = ruler::paint_autosave_plate(
            &painter,
            &keeping,
            layout.paper_origin.x + self.metrics.paper_size.x,
            spacing_plate,
            goal_plate.right(),
        );
        let next_spacing = carriage.line_spacing.next();
        // Plates only react once fully shown, not while calm mode fades them.
        if chrome < 1.0 {
            return;
        }
        let spacing = plate_button(
            ui,
            spacing_plate,
            "spacing-plate",
            "Line spacing (F1 / F2 / F3). Click for the next notch.",
        );
        if spacing.clicked() && !feeding {
            self.apply(Command::SetLineSpacing(next_spacing), now);
        }
        let zoom = plate_button(
            ui,
            zoom_plate,
            "zoom-plate",
            "Zoom (mouse wheel). Double-click for 100 %.",
        );
        if zoom.double_clicked() && !feeding {
            self.set_zoom(ZOOM_DEFAULT);
        }
        let correction = plate_button(
            ui,
            correction_plate,
            "correction-plate",
            "Click (or F4) for the next way: correction paper, eraser or fluid",
        );
        if correction.clicked() && !feeding {
            self.next_correction(now);
        }
        let goal = plate_button(
            ui,
            goal_plate,
            "goal-plate",
            "Click for the next: 250, 500 or 1000 words, 15, 25 or 50 minutes of typing, or off",
        );
        if goal.clicked() {
            self.next_goal();
        }
        let tip = match &keeping {
            Keeping::Autosave(WriteStatus::Failed(err)) => {
                format!("Could not save: {err}. Click to try again.")
            }
            Keeping::Autosave(_) => {
                format!("Saved to {}. Click to save now.", self.filing.location())
            }
            Keeping::Draft => "Kept in the drafts folder. Click to save it as a file.".to_owned(),
            Keeping::Off { unsaved: true } => "Unsaved changes. Click to save.".to_owned(),
            Keeping::Off { unsaved: false } => "Saved. Click to save now.".to_owned(),
        };
        let autosave = plate_button(ui, autosave_plate, "autosave-plate", &tip);
        if autosave.clicked() {
            let ctx = ui.ctx().clone();
            self.filing.save_now(&self.machine, &ctx, now);
        }
    }
}

impl TypewriterApp {
    fn paint_page(
        &self,
        painter: &Painter,
        page: &Page,
        origin: Pos2,
        dimming: Dimming,
        wetness: paper::Wetness<'_>,
    ) {
        paper::paint_margin_frame(
            painter,
            &self.metrics,
            self.machine.carriage(),
            self.machine.profile().margins.top_lines,
            origin,
        );
        paper::paint_sheet(
            painter,
            &self.metrics,
            page,
            origin,
            self.settings.look.ink_realism,
            dimming,
            wetness,
        );
    }

    fn paint_lifted(&self, painter: &Painter, view: Rect, origin: Pos2, curl: f32, lift: f32) {
        feed::paint_lifted_sheet(
            painter,
            &self.background,
            view,
            Rect::from_min_size(origin, self.metrics.paper_size),
            self.metrics.points_per_inch,
            curl,
            lift,
        );
    }
}

impl eframe::App for TypewriterApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        let now = ctx.input(|i| i.time);
        if self.first_sheet_pending {
            self.first_sheet_pending = false;
            self.load_first_sheet(now);
        }
        self.handle_input(&ctx);
        self.dry_fluid(now);
        self.take_picked(&ctx, now);
        self.filing
            .autosave(&self.machine, now, self.settings.saving.autosave);
        // Closing the window puts the project away too: ask first if needed.
        if ctx.input(|i| i.viewport().close_requested()) && !self.quitting && self.must_ask() {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            if self.leaving.is_none() && self.leaving_after_save_as.is_none() {
                self.leaving = Some(Leaving::Quit);
            }
        }
        if !self.is_feeding(now) {
            self.feeding = None;
        }
        self.wind_back(now);
        let mut folder_action = None;
        let mut note_written = None;
        let points_per_inch = self.points_per_inch();

        egui::CentralPanel::default()
            .frame(egui::Frame::NONE)
            .show(ui, |ui| {
                let view = ui.max_rect();
                self.background.paint(&ui.painter_at(view), view);
                let sheets = self.machine.document().finished();
                match self.view {
                    View::Typing => self.show_typing(ui, now),
                    View::Folder => {
                        let name = self.filing.name();
                        let location = self.filing.location();
                        let stats = sessions_line(Totals::of(self.machine.sessions()));
                        let project = ProjectLabel {
                            name: &name,
                            location: &location,
                            saved: self.filing.is_saved(),
                            stats: &stats,
                        };
                        let response = folder::show_folder(
                            ui,
                            view,
                            sheets,
                            &self.metrics,
                            &self.machine.profile().margins,
                            self.selected,
                            &project,
                            self.renaming.as_mut(),
                            self.renumbering.as_mut(),
                        );
                        self.pulled = response.pulled;
                        if let Some(i) = response.opened {
                            self.view = View::Sheet(i);
                        }
                        folder_action = response.action;
                    }
                    View::Sheet(i) => match sheets.get(i) {
                        Some(page) => {
                            let response = folder::show_sheet(
                                ui,
                                view,
                                self.machine.profile(),
                                self.machine.carriage(),
                                page,
                                i,
                                sheets.len(),
                                points_per_inch,
                                self.settings.look.ink_realism,
                                self.annotating.as_mut(),
                            );
                            if response.start_note {
                                self.annotating = Some(page.note().to_owned());
                            }
                            if let Some(note) = response.note_written {
                                note_written = Some((i, note));
                            }
                        }
                        None => self.view = View::Folder,
                    },
                    View::Settings => {
                        let before = self.settings.clone();
                        let response = render::settings::show_settings(
                            ui,
                            view,
                            &mut self.settings,
                            &self.machines,
                        );
                        if render::settings::gear_icon(ui, view, 1.0) || response.close {
                            self.view = View::Typing;
                        }
                        if self.settings != before {
                            self.follow_custom_goal(&before.goals);
                            self.apply_settings();
                        }
                    }
                }
                if let Some(scrunching) = &self.scrunching {
                    let t = now - scrunching.started;
                    if t < render::scrunch::SECONDS {
                        let painter = ui.ctx().layer_painter(egui::LayerId::new(
                            egui::Order::Foreground,
                            egui::Id::new("scrunch"),
                        ));
                        render::scrunch::paint(&painter, scrunching.outline, t, scrunching.seed);
                    }
                }
                self.filing.paint_notice(ui.ctx(), view, now);
            });
        if let Some(action) = folder_action {
            self.folder_action(action, &ctx, now);
        }
        self.confirm_scrunch(&ctx, now);
        self.leaving_dialog(&ctx, now);
        if self
            .scrunching
            .as_ref()
            .is_some_and(|s| now - s.started >= render::scrunch::SECONDS)
        {
            self.scrunching = None;
        }
        if let Some((sheet, note)) = note_written {
            self.annotating = None;
            if self.machine.annotate(sheet, &note) {
                self.filing.changed(now);
            }
        }
        if let Err(err) = self.settings_file.keep(&self.settings, now, false) {
            self.filing.notify(err, now);
        }

        if self.platen.is_animating(now)
            || self.feeding.is_some()
            || self.wind_back.is_some()
            || !self.wet.is_empty()
            || self.filing.is_animating(now)
            || self.settings_file.is_pending()
            || self.scrunching.is_some()
        {
            ctx.request_repaint();
        }
    }

    fn on_exit(&mut self) {
        // Saved whatever the pause, so nothing typed is lost, unless the
        // user chose not to (autosave off, or a discarded draft).
        let autosave = self.settings.saving.autosave;
        if autosave || !self.filing.is_saved() {
            self.filing.changed(0.0);
        }
        self.filing.keep(&self.machine, 0.0, autosave);
        storage::clear_running();
        if let Err(err) = self.settings_file.keep(&self.settings, 0.0, true) {
            eprintln!("{err}");
        }
    }
}

/// The project to start with: the one named on the command line, else the
/// one open last time, else a new one on the `new_machine`. Also says why a
/// project could not be opened.
fn first_project(
    machines: &Machines,
    new_machine: &str,
) -> anyhow::Result<(Typewriter, Filing, Option<String>)> {
    let asked = std::env::args_os().nth(1).map(PathBuf::from);
    let mut trouble = None;
    if let Some(path) = asked.or_else(storage::last_project) {
        match filing::open(machines, &path) {
            Ok(machine) => return Ok((machine, Filing::at(path), None)),
            Err(err) => trouble = Some(format!("Could not open {}: {err:#}", path.display())),
        }
    }
    let machine = Typewriter::new(machines.for_new(new_machine), Constraints::default())?;
    Ok((machine, Filing::draft(), trouble))
}

fn points_per_inch(zoom_percent: u16) -> f32 {
    POINTS_PER_INCH * f32::from(zoom_percent) / 100.0
}

fn unix_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

/// "4 sessions · 2 h 10 min · 1,840 words", or nothing before the first.
fn sessions_line(totals: Totals) -> String {
    if totals.sessions == 0 {
        return String::new();
    }
    let minutes = totals.seconds / 60;
    let time = match (minutes / 60, minutes % 60) {
        (0, m) => format!("{m} min"),
        (h, 0) => format!("{h} h"),
        (h, m) => format!("{h} h {m} min"),
    };
    let plural = |n: i64, word: &str| {
        let s = if n == 1 { "" } else { "s" };
        format!("{} {word}{s}", thousands(n))
    };
    format!(
        "{}  \u{b7}  {time}  \u{b7}  {}",
        plural(totals.sessions as i64, "session"),
        plural(totals.words, "word")
    )
}

/// 1840 as "1,840".
fn thousands(n: i64) -> String {
    let digits = n.unsigned_abs().to_string();
    let mut out = String::new();
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    if n < 0 { format!("-{out}") } else { out }
}

fn toggle_fullscreen(ctx: &egui::Context) {
    let fullscreen = ctx.input(|i| i.viewport().fullscreen.unwrap_or(false));
    ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(!fullscreen));
}

/// A plate below the scale that reacts to the pointer.
fn plate_button(ui: &mut egui::Ui, rect: Rect, id: &str, tip: &str) -> egui::Response {
    let response = ui
        .interact(rect, egui::Id::new(id), egui::Sense::click())
        .on_hover_text(tip);
    if response.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    response
}

fn zoomed(percent: u16, steps: i32) -> u16 {
    let target = i32::from(percent) + steps * i32::from(ZOOM_STEP);
    // Clamped to ZOOM_MIN..=ZOOM_MAX, so it fits in u16.
    target.clamp(i32::from(ZOOM_MIN), i32::from(ZOOM_MAX)) as u16
}

/// Index `step` sheets away from `index`, kept within `count` sheets.
fn stepped(index: usize, step: isize, count: usize) -> usize {
    index
        .saturating_add_signed(step)
        .min(count.saturating_sub(1))
}

fn install_fonts(ctx: &egui::Context) {
    let mut fonts = FontDefinitions::default();
    fonts.font_data.insert(
        FONT_FAMILY.into(),
        Arc::new(FontData::from_static(COURIER_PRIME)),
    );
    // Default fonts stay behind Courier Prime as a fallback for glyphs it lacks.
    let mut family = vec![FONT_FAMILY.to_owned()];
    family.extend(
        fonts
            .families
            .get(&FontFamily::Monospace)
            .cloned()
            .unwrap_or_default(),
    );
    fonts
        .families
        .insert(FontFamily::Name(FONT_FAMILY.into()), family);
    fonts.font_data.insert(
        note::PENCIL_FAMILY.into(),
        Arc::new(FontData::from_static(note::CAVEAT)),
    );
    let mut pencil = vec![note::PENCIL_FAMILY.to_owned()];
    pencil.extend(
        fonts
            .families
            .get(&FontFamily::Proportional)
            .cloned()
            .unwrap_or_default(),
    );
    fonts
        .families
        .insert(FontFamily::Name(note::PENCIL_FAMILY.into()), pencil);
    ctx.set_fonts(fonts);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zoom_steps_within_limits() {
        assert_eq!(zoomed(100, 1), 110);
        assert_eq!(zoomed(100, -1), 90);
        assert_eq!(zoomed(200, 1), 200);
        assert_eq!(zoomed(50, -1), 50);
    }

    #[test]
    fn sessions_add_up_in_words() {
        assert_eq!(sessions_line(Totals::default()), "");
        let totals = Totals {
            sessions: 4,
            seconds: 7_830,
            words: 1_840,
        };
        assert_eq!(
            sessions_line(totals),
            "4 sessions  \u{b7}  2 h 10 min  \u{b7}  1,840 words"
        );
        let one = Totals {
            sessions: 1,
            seconds: 59,
            words: -1_234_567,
        };
        assert_eq!(
            sessions_line(one),
            "1 session  \u{b7}  0 min  \u{b7}  -1,234,567 words"
        );
    }

    #[test]
    fn browsing_stays_within_the_folder() {
        assert_eq!(stepped(2, -1, 5), 1);
        assert_eq!(stepped(0, -1, 5), 0);
        assert_eq!(stepped(4, 1, 5), 4);
        assert_eq!(stepped(0, 1, 0), 0);
    }
}
