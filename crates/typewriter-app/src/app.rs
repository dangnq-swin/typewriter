use std::sync::Arc;

use eframe::egui::{self, FontData, FontDefinitions, FontFamily};
use typewriter_core::{Command, Constraints, Direction, Event, Profile, Typewriter};

use crate::input::{Action, Input, Zoom};
use crate::render::background::Background;
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
/// Accumulated Ctrl+scroll zoom factor that counts as one step.
const SCROLL_ZOOM_STEP: f32 = 1.1;

/// What fills the window.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum View {
    Typing,
    Folder,
    /// A finished sheet taken out of the folder, by index.
    Sheet(usize),
}

pub struct TypewriterApp {
    machine: Typewriter,
    metrics: Metrics,
    platen: PlatenView,
    input: Input,
    background: Background,
    view: View,
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
        // Ctrl+Plus/Minus zoom the sheet, not egui's whole interface.
        cc.egui_ctx.options_mut(|o| o.zoom_with_keyboard = false);
        let profile = Profile::from_toml_str(SM9_PROFILE)?;
        let metrics = Metrics::new(&profile, POINTS_PER_INCH);
        let machine = Typewriter::new(profile, Constraints::default())?;
        Ok(Self {
            machine,
            metrics,
            platen: PlatenView::new(true),
            input: Input::default(),
            background: Background::load(&cc.egui_ctx),
            view: View::Typing,
            selected: 0,
            zoom_percent: 100,
            scroll_zoom: 1.0,
            ink_realism: true,
        })
    }

    fn handle_input(&mut self, ctx: &egui::Context) {
        let now = ctx.input(|i| i.time);
        let (events, shift_down, zoom_delta) =
            ctx.input(|i| (i.events.clone(), i.modifiers.shift, i.zoom_delta()));
        for action in self.input.actions(&events, shift_down) {
            match action {
                Action::Machine(command) => {
                    if !self.browse_command(command) {
                        self.apply(command, now);
                    }
                }
                Action::Zoom(zoom) => self.zoom(zoom),
                Action::PageUp => self.page_up(),
                Action::PageDown => self.page_down(),
                Action::Escape => self.escape(),
            }
        }
        self.scroll_zoom *= zoom_delta;
        if self.scroll_zoom >= SCROLL_ZOOM_STEP {
            self.zoom(Zoom::In);
            self.scroll_zoom = 1.0;
        } else if self.scroll_zoom <= 1.0 / SCROLL_ZOOM_STEP {
            self.zoom(Zoom::Out);
            self.scroll_zoom = 1.0;
        }
    }

    /// Typing always goes to the machine, so it also brings the view back.
    fn apply(&mut self, command: Command, now: f64) {
        self.view = View::Typing;
        for event in self.machine.apply(command) {
            if matches!(event, Event::Blocked(_)) {
                self.platen.jolt(now);
            }
        }
    }

    fn zoom(&mut self, zoom: Zoom) {
        let percent = zoomed(self.zoom_percent, zoom);
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
            View::Typing | View::Folder => View::Typing,
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
        paper::paint_margin_frame(
            &painter,
            &self.metrics,
            carriage,
            self.machine.profile().margins.top_lines,
            layout.paper_origin,
        );
        paper::paint_sheet(
            &painter,
            &self.metrics,
            self.machine.page(),
            layout.paper_origin,
            self.ink_realism,
        );
        let ruler_top = ruler::top(&self.metrics, layout.strike_point);
        ruler::paint_scale(
            &painter,
            &self.metrics,
            carriage,
            self.machine.page().columns(),
            layout.paper_origin.x,
            ruler_top,
        );
        ruler::paint_spacing_indicator(
            &painter,
            carriage.line_spacing,
            layout.paper_origin.x,
            ruler_top + ruler::HEIGHT,
        );
        platen::paint_strike_marker(&painter, &self.metrics, layout.strike_point);

        let finished = self.machine.document().finished().len();
        if folder::desk_icon(ui, view, finished) {
            self.open_folder();
        }
    }
}

impl eframe::App for TypewriterApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        self.handle_input(&ctx);
        let now = ctx.input(|i| i.time);

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

        if self.platen.is_animating(now) {
            ctx.request_repaint();
        }
    }
}

fn zoomed(percent: u16, zoom: Zoom) -> u16 {
    match zoom {
        Zoom::In => percent.saturating_add(ZOOM_STEP).min(ZOOM_MAX),
        Zoom::Out => percent.saturating_sub(ZOOM_STEP).max(ZOOM_MIN),
        Zoom::Reset => 100,
    }
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
        assert_eq!(zoomed(100, Zoom::In), 110);
        assert_eq!(zoomed(100, Zoom::Out), 90);
        assert_eq!(zoomed(200, Zoom::In), 200);
        assert_eq!(zoomed(50, Zoom::Out), 50);
        assert_eq!(zoomed(170, Zoom::Reset), 100);
    }

    #[test]
    fn browsing_stays_within_the_folder() {
        assert_eq!(stepped(2, -1, 5), 1);
        assert_eq!(stepped(0, -1, 5), 0);
        assert_eq!(stepped(4, 1, 5), 4);
        assert_eq!(stepped(0, 1, 0), 0);
    }
}
