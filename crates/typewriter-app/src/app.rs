use std::collections::HashMap;
use std::sync::Arc;

use eframe::egui::{self, FontData, FontDefinitions, FontFamily, Painter, Pos2, Rect, pos2};
use typewriter_core::page::Page;
use typewriter_core::{Command, Constraints, Direction, EraseMode, Event, Profile, Typewriter};

use crate::audio::{self, Audio};
use crate::input::{Action, Input};
use crate::render::background::Background;
use crate::render::calm::{self, Dimming};
use crate::render::feed::{self, FeedMotion};
use crate::render::platen::{self, PlatenView};
use crate::render::{FONT_FAMILY, Metrics, folder, paper, ruler};

const COURIER_PRIME: &[u8] =
    include_bytes!("../../../assets/fonts/courier-prime/CourierPrime-Regular.ttf");
// Built-in until user profiles are loaded from disk (M8).
const SM9_PROFILE: &str = include_str!("../../../profiles/olympia-sm9.toml");
/// Screen points per inch at 100% zoom.
const POINTS_PER_INCH: f32 = 96.0;
const ZOOM_MIN: u16 = 50;
const ZOOM_MAX: u16 = 200;
const ZOOM_STEP: u16 = 10;
const ZOOM_DEFAULT: u16 = 100;
/// Scrolled distance that counts as one zoom step: about one wheel notch.
const SCROLL_POINTS_PER_STEP: f32 = 40.0;

/// Correction fluid smudges what is typed on it until it has dried.
const FLUID_DRY_SECONDS: f64 = 3.0;

/// Room beyond the window's edge for a moving sheet's shadow.
const SHADOW_ROOM: f32 = 30.0;

/// What fills the window.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum View {
    Typing,
    Folder,
    /// A finished sheet taken out of the folder, by index.
    Sheet(usize),
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
    /// A new document starts by winding its first sheet in.
    first_sheet_pending: bool,
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
    // Toggle comes with settings (M8).
    ink_realism: bool,
}

impl TypewriterApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> anyhow::Result<Self> {
        install_fonts(&cc.egui_ctx);
        // No Ctrl shortcuts on a typewriter, egui's interface zoom included.
        cc.egui_ctx.options_mut(|o| o.zoom_with_keyboard = false);
        let profile = Profile::from_toml_str(SM9_PROFILE)?;
        let metrics = Metrics::new(&profile, POINTS_PER_INCH);
        let audio = Audio::new(&profile)
            .inspect_err(|err| eprintln!("sound unavailable, typing silently: {err:#}"))
            .ok();
        let machine = Typewriter::new(profile, Constraints::default())?;
        Ok(Self {
            machine,
            metrics,
            platen: PlatenView::new(true),
            input: Input::default(),
            background: Background::load(&cc.egui_ctx),
            audio,
            feed_motion: audio::sheet_feed_motion(),
            feeding: None,
            first_sheet_pending: true,
            wet: HashMap::new(),
            view: View::Typing,
            calm: false,
            selected: 0,
            zoom_percent: ZOOM_DEFAULT,
            scroll_zoom: 0.0,
            ink_realism: true,
        })
    }

    fn handle_input(&mut self, ctx: &egui::Context) {
        let now = ctx.input(|i| i.time);
        let (events, shift_down, scrolled) =
            ctx.input(|i| (i.events.clone(), i.modifiers.shift, i.smooth_scroll_delta.y));
        // Keys still go through the input state (e.g. Shift+Tab counting),
        // they just do nothing while a sheet is being wound in.
        let actions = self.input.actions(&events, shift_down);
        let feeding = self.is_feeding(now);
        for action in actions {
            match action {
                // The window is not part of the machine.
                Action::Fullscreen => toggle_fullscreen(ctx),
                _ if feeding => {}
                Action::Machine(command) => {
                    if !self.browse_command(command) {
                        self.apply(command, now);
                    }
                }
                Action::PageUp => self.page_up(),
                Action::PageDown => self.page_down(),
                Action::Escape => self.escape(),
                Action::NextCorrection => self.next_correction(now),
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
        }
    }

    /// 1 for fluid just dabbed on, falling to 0 as it dries.
    fn wetness(&self, now: f64, half_line: u16, column: u16) -> f32 {
        self.wet.get(&(half_line, column)).map_or(0.0, |&dabbed| {
            (1.0 - (now - dabbed) / FLUID_DRY_SECONDS).clamp(0.0, 1.0) as f32
        })
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
        if percent != self.zoom_percent {
            self.zoom_percent = percent;
            self.metrics = Metrics::new(self.machine.profile(), self.points_per_inch());
            self.platen.snap();
        }
    }

    fn points_per_inch(&self) -> f32 {
        POINTS_PER_INCH * f32::from(self.zoom_percent) / 100.0
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
            View::Typing => {}
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
            View::Folder => View::Typing,
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
                let dimming = Dimming {
                    half_line: *old_half_line,
                    amount: calm,
                };
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
        let dimming = Dimming {
            half_line: carriage.half_line,
            amount: calm,
        };
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

        let feeding = self.is_feeding(now);
        let finished = self.machine.document().finished().len();
        if folder::desk_icon(ui, view, finished, chrome) && !feeding {
            self.open_folder();
        }
        if calm::calm_icon(ui, view) && !feeding {
            self.calm = !self.calm;
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
            "Fixing mistakes (F4). Click for the next way: correction paper, eraser or fluid.",
        );
        if correction.clicked() && !feeding {
            self.next_correction(now);
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
            self.ink_realism,
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
        if !self.is_feeding(now) {
            self.feeding = None;
        }

        egui::CentralPanel::default()
            .frame(egui::Frame::NONE)
            .show(ui, |ui| {
                let view = ui.max_rect();
                self.background.paint(&ui.painter_at(view), view);
                let sheets = self.machine.document().finished();
                match self.view {
                    View::Typing => self.show_typing(ui, now),
                    View::Folder => {
                        if let Some(i) =
                            folder::show_folder(ui, view, sheets, &self.metrics, &mut self.selected)
                        {
                            self.view = View::Sheet(i);
                        }
                    }
                    View::Sheet(i) => match sheets.get(i) {
                        Some(page) => folder::show_sheet(
                            &ui.painter_at(view),
                            view,
                            self.machine.profile(),
                            self.machine.carriage(),
                            page,
                            i + 1,
                            sheets.len(),
                            self.points_per_inch(),
                            self.ink_realism,
                        ),
                        None => self.view = View::Folder,
                    },
                }
            });

        if self.platen.is_animating(now) || self.feeding.is_some() || !self.wet.is_empty() {
            ctx.request_repaint();
        }
    }
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
    fn browsing_stays_within_the_folder() {
        assert_eq!(stepped(2, -1, 5), 1);
        assert_eq!(stepped(0, -1, 5), 0);
        assert_eq!(stepped(4, 1, 5), 4);
        assert_eq!(stepped(0, 1, 0), 0);
    }
}
