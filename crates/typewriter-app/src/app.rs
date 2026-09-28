use std::sync::Arc;

use eframe::egui::{self, FontData, FontDefinitions, FontFamily};
use typewriter_core::{Constraints, Event, Profile, Typewriter};

use crate::input::Input;
use crate::render::background::Background;
use crate::render::platen::{self, PlatenView};
use crate::render::{FONT_FAMILY, Metrics, paper, ruler};

const COURIER_PRIME: &[u8] =
    include_bytes!("../../../assets/fonts/courier-prime/CourierPrime-Regular.ttf");
// Built-in until user profiles are loaded from disk (M8).
const SM9_PROFILE: &str = include_str!("../../../profiles/olympia-sm9.toml");
const POINTS_PER_INCH: f32 = 96.0;

pub struct TypewriterApp {
    machine: Typewriter,
    metrics: Metrics,
    platen: PlatenView,
    input: Input,
    background: Background,
}

impl TypewriterApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> anyhow::Result<Self> {
        install_fonts(&cc.egui_ctx);
        let profile = Profile::from_toml_str(SM9_PROFILE)?;
        let metrics = Metrics::new(&profile, POINTS_PER_INCH);
        let machine = Typewriter::new(profile, Constraints::default())?;
        Ok(Self {
            machine,
            metrics,
            platen: PlatenView::new(true),
            input: Input::default(),
            background: Background::load(&cc.egui_ctx),
        })
    }

    fn handle_input(&mut self, ctx: &egui::Context) {
        let now = ctx.input(|i| i.time);
        let (events, shift_down) = ctx.input(|i| (i.events.clone(), i.modifiers.shift));
        for command in self.input.commands(&events, shift_down) {
            for event in self.machine.apply(command) {
                if matches!(event, Event::Blocked(_)) {
                    self.platen.jolt(now);
                }
            }
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
                let carriage = self.machine.carriage();
                let cell = self
                    .metrics
                    .cell_offset(carriage.half_line, carriage.column);
                let layout = self.platen.layout(view, &self.metrics, cell, now);
                let painter = ui.painter_at(view);
                self.background.paint(&painter, view);
                paper::paint_margin_frame(
                    &painter,
                    &self.metrics,
                    carriage,
                    self.machine.profile().margins.top_lines,
                    self.machine.page(),
                    layout.paper_origin,
                );
                paper::paint_sheet(
                    &painter,
                    &self.metrics,
                    self.machine.page(),
                    layout.paper_origin,
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
            });

        if self.platen.is_animating(now) {
            ctx.request_repaint();
        }
    }
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
