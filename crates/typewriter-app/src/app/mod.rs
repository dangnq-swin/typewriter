//! The window around the [`Desk`]: reads input into [`Intent`]s, draws the
//! desk ([`view`]), and does what it asks ([`Effect`]s): sound, dialogs,
//! the viewport, page geometry.

mod desk;
mod fonts;
mod intent;
mod view;

use std::path::{Path, PathBuf};

use eframe::egui::{self, Pos2, Rect};
use typewriter_core::{Constraints, Typewriter};

use crate::audio::{self, Audio};
use crate::filing::{self, Filing};
use crate::input::{Action, Input};
use crate::instance::{self, Listening};
use crate::machines::Machines;
use crate::picker::{Dialog, Picker};
use crate::printing::Printing;
use crate::render::background::Background;
use crate::render::platen::PlatenView;
use crate::render::{Metrics, scrunch};
use crate::render::{folder, pdf};
use crate::settings::{self, SettingsFile};
use crate::storage;
use desk::{Desk, View};
use intent::{Effect, Intent, Sound};

/// A sheet being scrunched: its on-screen outline when it went.
struct Scrunching {
    outline: [Pos2; 4],
    started: f64,
    seed: u64,
}

pub struct TypewriterApp {
    desk: Desk,
    input: Input,
    picker: Picker,
    /// `None` without an output device: silent.
    audio: Option<Audio>,
    background: Background,
    metrics: Metrics,
    platen: PlatenView,
    /// The platen knobs' grips as last drawn, while they can be turned. The
    /// wheel turns them instead of zooming.
    knobs: Vec<Rect>,
    /// The chosen sheet's outline in the folder, last frame.
    pulled: Option<[Pos2; 4]>,
    scrunching: Option<Scrunching>,
    /// When fullscreen was last switched. Until the window catches up, its
    /// state must not overrule the setting.
    fullscreen_sent: Option<f64>,
    /// Everything was answered: let the window close.
    quitting: bool,
    settings_file: SettingsFile,
    running: storage::Running,
    /// Projects later launches hand over. `None` if another app took them.
    listening: Option<Listening>,
    printing: Printing,
}

impl TypewriterApp {
    /// `settings`: from [`SettingsFile::load`], before the window opened.
    pub fn new(
        cc: &eframe::CreationContext<'_>,
        settings: (settings::Settings, SettingsFile, Option<String>),
    ) -> anyhow::Result<Self> {
        fonts::install(&cc.egui_ctx);
        // No Ctrl shortcuts, egui's zoom keys included.
        cc.egui_ctx.options_mut(|o| o.zoom_with_keyboard = false);
        let (settings, settings_file, settings_trouble) = settings;
        let machines = Machines::load()?;
        let (machine, filing, trouble) = first_project(&machines, &settings.machine)?;
        let audio = Audio::new(machine.profile().sounds.clone(), settings.sound.clone())
            .inspect_err(|err| eprintln!("sound unavailable, typing silently: {err:#}"))
            .ok();
        let (running, crashed) = storage::Running::mark();
        let recovered = (crashed && filing.is_saved_somewhere()).then(|| {
            "Recovered your work from when Typewriter last closed unexpectedly.".to_owned()
        });
        filing.remember();
        let platen = PlatenView::new(settings.look.carriage_travel);
        let feed_motion = audio::sheet_feed_motion();
        let mut desk = Desk::new(machine, filing, settings, machines, feed_motion);
        let background = Background::load(&cc.egui_ctx, &mut desk.settings.look);
        let background_trouble = background.problem().map(str::to_owned);
        if let Some(trouble) = trouble
            .or(settings_trouble)
            .or(recovered)
            .or(background_trouble)
        {
            desk.notice.show(trouble, 0.0);
        }
        let metrics = Metrics::new(desk.project.machine.profile(), desk.points_per_inch());
        Ok(Self {
            desk,
            input: Input::default(),
            picker: Picker::default(),
            audio,
            background,
            metrics,
            platen,
            knobs: Vec::new(),
            pulled: None,
            scrunching: None,
            // Grace for the desktop to show the window as built.
            fullscreen_sent: Some(0.0),
            quitting: false,
            settings_file,
            running,
            listening: instance::listen(&cc.egui_ctx),
            printing: Printing::default(),
        })
    }

    /// Hands `intent` to the desk and does what it asks, at once.
    fn send(&mut self, intent: Intent, ctx: &egui::Context, now: f64) {
        self.desk.update(intent, now);
        self.run_effects(ctx, now);
    }

    /// The frame's keys and wheel, unless a text field or question has the
    /// keys.
    fn read_input(&mut self, ctx: &egui::Context) -> Option<Intent> {
        if self.desk.keys_to_fields() {
            return None;
        }
        let (events, shift_down, wheel) =
            ctx.input(|i| (i.events.clone(), i.modifiers.shift, i.smooth_scroll_delta.y));
        // Run the gesture state even while busy (Shift+Tab counting).
        let mut keys = self.input.actions(&events, shift_down);
        // egui takes Esc to close a menu, and on the settings card to leave
        // a focused field. The log up close takes it first.
        let menu_open = egui::Popup::is_any_open(ctx);
        let editing = self.desk.view == View::Settings && ctx.memory(|m| m.focused().is_some());
        if !self.desk.overlays.log_open && (menu_open || editing) {
            keys.retain(|&action| action != Action::Escape);
        }
        let pointer = ctx.input(|i| i.pointer.hover_pos());
        let over_knob = pointer.is_some_and(|p| self.knobs.iter().any(|knob| knob.contains(p)));
        Some(Intent::Input {
            keys,
            wheel,
            over_knob,
        })
    }

    /// Does what the desk asked.
    fn run_effects(&mut self, ctx: &egui::Context, now: f64) {
        for effect in self.desk.take_effects() {
            match effect {
                Effect::Sound(sound) => self.play(sound),
                Effect::Jolt => self.platen.jolt(now),
                Effect::Scrunched(index) => {
                    if let Some(outline) = self.pulled {
                        self.scrunching = Some(Scrunching {
                            outline,
                            started: now,
                            seed: index as u64 ^ now.to_bits(),
                        });
                    }
                    folder::put_back(ctx, index);
                }
                Effect::Ask(Dialog::Texture) => {
                    let texture = self.desk.settings.look.background_texture.as_deref();
                    let folder = texture.and_then(Path::parent).map(Path::to_path_buf);
                    self.picker.ask(Dialog::Texture, folder, String::new(), ctx);
                }
                Effect::Ask(dialog) => {
                    let filing = &self.desk.project.filing;
                    let (folder, file_name) = (filing.dialog_folder(), filing.file_name());
                    self.picker.ask(dialog, folder, file_name, ctx);
                }
                Effect::Fullscreen(on) => self.set_fullscreen(ctx, on, now),
                Effect::CancelClose => ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose),
                Effect::Close => {
                    self.quitting = true;
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
                Effect::Zoomed => self.relayout(),
                Effect::MachineChanged => {
                    self.scrunching = None;
                    self.relayout();
                    if let Some(audio) = &mut self.audio {
                        audio.set_machine(self.desk.project.machine.profile().sounds.clone());
                    }
                }
                Effect::SettingsChanged => {
                    self.background.follow(ctx, &mut self.desk.settings.look);
                    let settings = &self.desk.settings;
                    self.platen.carriage_travel = settings.look.carriage_travel;
                    if let Some(audio) = &mut self.audio {
                        audio.set_settings(settings.sound.clone());
                    }
                    self.set_fullscreen(ctx, self.desk.settings.look.fullscreen, now);
                }
                Effect::Print(sheet) => self.print(ctx, sheet, now),
                Effect::ReloadMachines => match Machines::load() {
                    Ok(machines) => self.desk.machines = machines,
                    Err(err) => eprintln!("could not reload the machines: {err:#}"),
                },
            }
        }
    }

    fn play(&mut self, sound: Sound) {
        let Some(audio) = &mut self.audio else {
            return;
        };
        match sound {
            Sound::Machine(event) => audio.play(event),
            Sound::Crumple => audio.play_crumple(),
            Sound::WindIn => audio.play_wind_in(),
            Sound::WindBackClick => audio.play_wind_back_click(),
        }
    }

    /// Opens finished sheet `sheet`, or every sheet, in the PDF viewer.
    fn print(&mut self, ctx: &egui::Context, sheet: Option<usize>, now: f64) {
        let project = &self.desk.project;
        let (document, name) = (project.machine.document(), project.filing.name());
        let (title, sheets) = match sheet {
            None => (name, pdf::project_sheets(document)),
            Some(index) => (
                format!("{name}, sheet {}", index + 1),
                document
                    .finished()
                    .get(index)
                    .map(|page| vec![(index, page)])
                    .unwrap_or_default(),
            ),
        };
        let profile = project.machine.profile();
        let ink_realism = self.desk.settings.look.ink_realism;
        let told = self
            .printing
            .print(ctx, profile, &sheets, &title, ink_realism);
        self.desk.notice.show(told, now);
    }

    /// Page geometry for the machine and zoom, jumped to without gliding.
    fn relayout(&mut self) {
        let profile = self.desk.project.machine.profile();
        self.metrics = Metrics::new(profile, self.desk.points_per_inch());
        self.platen.snap();
    }

    fn set_fullscreen(&mut self, ctx: &egui::Context, fullscreen: bool, now: f64) {
        if ctx.input(|i| i.viewport().fullscreen) != Some(fullscreen) {
            ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(fullscreen));
            self.fullscreen_sent = Some(now);
        }
    }

    /// The setting follows the window (the desktop can switch it too),
    /// except within 1 s of the app switching it.
    fn follow_fullscreen(&mut self, ctx: &egui::Context, now: f64) {
        let Some(actual) = ctx.input(|i| i.viewport().fullscreen) else {
            return;
        };
        match self.fullscreen_sent {
            Some(sent) if actual != self.desk.settings.look.fullscreen && now - sent < 1.0 => {}
            _ => {
                self.fullscreen_sent = None;
                self.send(Intent::WindowFullscreen(actual), ctx, now);
            }
        }
    }
}

#[cfg(test)]
impl TypewriterApp {
    /// The window's parts around a test desk: no sound, nothing on disk.
    fn for_tests(ctx: &egui::Context) -> Self {
        let mut desk = desk::testing::desk();
        let background = Background::load(ctx, &mut desk.settings.look);
        let metrics = Metrics::new(desk.project.machine.profile(), desk.points_per_inch());
        Self {
            desk,
            input: Input::default(),
            picker: Picker::default(),
            audio: None,
            background,
            metrics,
            platen: PlatenView::new(true),
            knobs: Vec::new(),
            pulled: None,
            scrunching: None,
            fullscreen_sent: None,
            quitting: false,
            settings_file: SettingsFile::nowhere(),
            running: storage::Running::nowhere(),
            listening: None,
            printing: Printing::default(),
        }
    }
}

impl eframe::App for TypewriterApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        let now = ctx.input(|i| i.time);
        self.desk.start_frame(now);
        self.run_effects(&ctx, now);
        if let Some(input) = self.read_input(&ctx) {
            self.send(input, &ctx, now);
        }
        self.follow_fullscreen(&ctx, now);
        if let Some(picked) = self.picker.picked() {
            self.send(Intent::Picked(picked), &ctx, now);
        }
        if let Some(failed) = self.printing.failure() {
            self.desk.notice.show(failed, now);
        }
        while let Some(project) = self.listening.as_ref().and_then(Listening::take) {
            // Wayland may refuse the focus; the desktop then flags the window.
            ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
            self.send(Intent::OpenFile(project), &ctx, now);
        }
        if ctx.input(|i| i.viewport().close_requested()) && !self.quitting {
            self.send(Intent::CloseWindow, &ctx, now);
        }
        self.desk.tick(now);
        self.run_effects(&ctx, now);

        for intent in self.show(ui, now) {
            self.send(intent, &ctx, now);
        }
        if self
            .scrunching
            .as_ref()
            .is_some_and(|s| now - s.started >= scrunch::SECONDS)
        {
            self.scrunching = None;
        }
        if let Err(err) = self.settings_file.keep(&self.desk.settings, now, false) {
            self.desk.notice.show(err, now);
        }

        if self.desk.is_animating(now)
            || self.platen.is_animating(now)
            || self.picker.is_open()
            || self.settings_file.is_pending()
            || self.scrunching.is_some()
        {
            ctx.request_repaint();
        }
    }

    fn on_exit(&mut self) {
        if let Err(err) = self.desk.put_away() {
            eprintln!("{err}");
        }
        self.running.clear();
        if let Some(listening) = &self.listening {
            listening.stop();
        }
        if let Err(err) = self.settings_file.keep(&self.desk.settings, 0.0, true) {
            eprintln!("{err}");
        }
    }
}

/// The starting project: the command line's, else last run's, else a new one
/// on `new_machine`'s profile and rules. Plus why an open failed, if one did.
fn first_project(
    machines: &Machines,
    new_machine: &settings::Machine,
) -> anyhow::Result<(Typewriter, Filing, Option<String>)> {
    let asked = std::env::args_os().nth(1).map(PathBuf::from);
    let mut trouble = None;
    if let Some(path) = asked.or_else(storage::last_project) {
        match filing::open(machines, &path) {
            Ok(machine) => return Ok((machine, Filing::at(path), None)),
            Err(err) => trouble = Some(format!("Could not open {}: {err:#}", path.display())),
        }
    }
    let profile = machines.for_new(&new_machine.profile);
    let constraints = new_machine.rules.constraints(Constraints::default().erase);
    let machine = Typewriter::new(profile, constraints)?;
    Ok((machine, Filing::draft(), trouble))
}
