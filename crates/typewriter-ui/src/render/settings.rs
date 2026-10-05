//! The settings card on the dimmed desk, and the gear icon that opens it.

use std::f32::consts::TAU;

use eframe::egui::{
    self, Color32, CornerRadius, Margin, Pos2, Rect, RichText, Shape, Stroke, Ui, UiBuilder, Vec2,
    vec2,
};

use super::{CLICK, DIM, DeskIcon, HIGHLIGHT, SHEET, SHEET_EDGE, background};
use crate::machines::{self, Machines};
use crate::settings::{self, Settings};
use crate::storage;

const TEXT: Color32 = Color32::from_rgb(0x2A, 0x26, 0x22);
const QUIET: Color32 = Color32::from_rgb(0x6E, 0x66, 0x5A);
const PROBLEM: Color32 = Color32::from_rgb(0xA0, 0x2C, 0x1C);
const ICON_FILL: Color32 = Color32::from_rgb(0xDC, 0xD8, 0xCE);
const ICON_EDGE: Color32 = Color32::from_rgb(0x7A, 0x72, 0x66);
const CARD_WIDTH: f32 = 520.0;

#[derive(Debug, Default)]
pub struct SettingsResponse {
    pub close: bool,
    pub choose_texture: bool,
}

/// Draws the card. Edits `settings` in place: changes apply at once.
/// `background`: `Some` with why the own texture isn't showing, if it
/// isn't; `None` in normal mode, which has no Background row.
pub fn show_settings(
    ui: &mut Ui,
    view: Rect,
    settings: &mut Settings,
    machines: &Machines,
    background: Option<Option<&str>>,
) -> SettingsResponse {
    ui.painter_at(view)
        .rect_filled(view, CornerRadius::ZERO, DIM);
    let width = CARD_WIDTH.min(view.width() - 32.0);
    let card = Rect::from_center_size(
        view.center() - vec2(0.0, 20.0),
        vec2(width, (view.height() - 140.0).max(200.0)),
    );
    let mut response = SettingsResponse::default();
    ui.scope_builder(UiBuilder::new().max_rect(card), |ui| {
        ui.style_mut().visuals = egui::Visuals::light();
        ui.style_mut().visuals.override_text_color = Some(TEXT);
        egui::Frame::new()
            .fill(SHEET)
            .stroke(Stroke::new(1.0, SHEET_EDGE))
            .corner_radius(CornerRadius::same(4))
            .inner_margin(Margin::same(20))
            .show(ui, |ui| {
                ui.set_min_size(card.size() - vec2(40.0, 40.0));
                ui.horizontal(|ui| {
                    ui.heading("Settings");
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button("Close (Esc)").clicked() {
                            response.close = true;
                        }
                    });
                });
                ui.add_space(6.0);
                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        sound(ui, &mut settings.sound);
                        look(ui, &mut settings.look);
                        if let Some(problem) = background {
                            response.choose_texture =
                                background_row(ui, &mut settings.look, problem);
                        }
                        goals(ui, &mut settings.goals);
                        machine(ui, &mut settings.machine, machines);
                        section(ui, "Projects");
                        ui.checkbox(&mut settings.saving.autosave, "Autosave");
                        ui.add_space(14.0);
                        ui.separator();
                        if ui.button("Reset to defaults").clicked() {
                            *settings = Settings::default();
                        }
                    });
            });
    });
    response
}

fn section(ui: &mut Ui, title: &str) {
    ui.add_space(14.0);
    ui.label(RichText::new(title).strong().size(15.0));
    ui.separator();
}

fn note(ui: &mut Ui, text: impl Into<String>) {
    ui.label(RichText::new(text).color(QUIET).size(12.0));
}

fn sound(ui: &mut Ui, sound: &mut settings::Sound) {
    section(ui, "Sound");
    ui.horizontal(|ui| {
        ui.label("Volume");
        ui.add_enabled(
            !sound.mute,
            egui::Slider::new(&mut sound.volume, 0..=settings::VOLUME_MAX).suffix(" %"),
        );
        ui.checkbox(&mut sound.mute, "Mute");
    });
    ui.add_enabled_ui(!sound.mute, |ui| {
        ui.checkbox(&mut sound.keys, "Keys: strikes, space bar, backspace, tab");
        ui.checkbox(
            &mut sound.bell,
            "Margin bell, and the bell for a reached goal",
        );
        ui.checkbox(
            &mut sound.platen,
            "Platen clicks and carriage return (where the machine makes them)",
        );
        ui.checkbox(&mut sound.sheet_feed, "Sheet feed and scrunching up");
        ui.checkbox(&mut sound.corrections, "Corrections: eraser and fluid");
        ui.checkbox(&mut sound.blocked, "Blocked input");
    });
}

fn look(ui: &mut Ui, look: &mut settings::Look) {
    section(ui, "Look & calm");
    ui.checkbox(
        &mut look.ink_realism,
        "Ink realism: each strike a little uneven",
    );
    ui.checkbox(&mut look.fullscreen, "Fullscreen (F11)");
    ui.checkbox(
        &mut look.platen_guides,
        "Guides while the platen knob turns",
    )
    .on_hover_text("Faint lines at the ascender, baseline and descender");
    ui.checkbox(
        &mut look.carriage_travel,
        "The paper slides sideways with the carriage",
    );
    ui.label("Calm mode dimming");
    ui.horizontal(|ui| {
        ui.label("  faintest after");
        ui.add(egui::Slider::new(
            &mut look.calm_falloff_lines,
            settings::CALM_FALLOFF_LINES,
        ));
        ui.label("lines");
    });
    ui.horizontal(|ui| {
        ui.label("  faintest ink");
        ui.add(
            egui::Slider::new(
                &mut look.calm_minimum_percent,
                settings::CALM_MINIMUM_PERCENT,
            )
            .suffix(" %"),
        );
    });
}

/// The background choice. True if the texture's file dialog was asked for.
fn background_row(ui: &mut Ui, look: &mut settings::Look, problem: Option<&str>) -> bool {
    let mut choose = false;
    ui.horizontal(|ui| {
        ui.label("Background");
        ui.radio_value(&mut look.background, settings::Background::Paper, "Paper");
        ui.radio_value(&mut look.background, settings::Background::Colour, "Plain");
        ui.radio_value(
            &mut look.background,
            settings::Background::Texture,
            "Own image",
        );
    });
    match look.background {
        settings::Background::Paper => {}
        settings::Background::Colour => {
            ui.horizontal(|ui| {
                ui.add_space(16.0);
                for tone in settings::PaperTone::ALL {
                    if swatch(ui, tone, look.background_colour == tone) {
                        look.background_colour = tone;
                    }
                }
            });
        }
        settings::Background::Texture => {
            ui.horizontal(|ui| {
                ui.add_space(16.0);
                choose = ui.button("Choose…").clicked();
                match &look.background_texture {
                    Some(path) => note(ui, storage::home_relative(path)),
                    None => note(ui, "None chosen"),
                }
            });
            if let Some(problem) = problem {
                ui.colored_label(PROBLEM, problem);
            }
        }
    }
    choose
}

/// A square of paper `tone`, ringed if `chosen`. True if clicked.
fn swatch(ui: &mut Ui, tone: settings::PaperTone, chosen: bool) -> bool {
    let (rect, response) = ui.allocate_exact_size(vec2(26.0, 20.0), CLICK);
    let edge = if chosen || response.hovered() {
        Stroke::new(2.0, HIGHLIGHT)
    } else {
        Stroke::new(1.0, SHEET_EDGE)
    };
    ui.painter().rect(
        rect,
        CornerRadius::same(2),
        background::tone(tone),
        edge,
        egui::StrokeKind::Inside,
    );
    response.on_hover_text(tone.name()).clicked()
}

fn goals(ui: &mut Ui, goals: &mut settings::Goals) {
    section(ui, "Goals");
    egui::Grid::new("custom-goals")
        .num_columns(2)
        .spacing([12.0, 6.0])
        .show(ui, |ui| {
            ui.checkbox(&mut goals.custom_words_on, "Custom words");
            ui.add_enabled(
                goals.custom_words_on,
                egui::DragValue::new(&mut goals.custom_words)
                    .range(1..=settings::CUSTOM_WORDS_MAX)
                    .speed(10),
            );
            ui.end_row();
            ui.checkbox(&mut goals.custom_minutes_on, "Custom minutes");
            ui.add_enabled(
                goals.custom_minutes_on,
                egui::DragValue::new(&mut goals.custom_minutes)
                    .range(1..=settings::CUSTOM_MINUTES_MAX)
                    .speed(1),
            );
            ui.end_row();
        });
}

fn machine(ui: &mut Ui, machine: &mut settings::Machine, machines: &Machines) {
    section(ui, "Machine");
    ui.horizontal(|ui| {
        ui.label("New projects are typed on");
        egui::ComboBox::from_id_salt("machine")
            .selected_text(&machine.profile)
            .show_ui(ui, |ui| {
                for profile in machines.all() {
                    ui.selectable_value(&mut machine.profile, profile.name.clone(), &profile.name);
                }
            });
    });
    match machines.find(&machine.profile) {
        Some(profile) => note(ui, machines::describe(&profile)),
        None => {
            ui.colored_label(
                PROBLEM,
                format!("Not found, so new projects use the {}.", machines::DEFAULT),
            );
        }
    }
    for problem in &machines.problems {
        ui.colored_label(PROBLEM, problem);
    }
    let rules = &mut machine.rules;
    ui.label("Rules")
        .on_hover_text("For new projects, and the one in the machine");
    ui.checkbox(&mut rules.backspace, "Backspace");
    ui.checkbox(&mut rules.lock_at_right_margin, "Lock at the right margin")
        .on_hover_text("Home releases it until the next return");
    ui.checkbox(&mut rules.free_movement, "Free movement (Left / Right)")
        .on_hover_text("Left / Right move the carriage without typing");
    ui.checkbox(&mut rules.type_jams, "Type jams")
        .on_hover_text("Two keys struck almost together tangle; Backspace frees them");
    ui.checkbox(
        &mut rules.delete_in_cycle,
        "Delete in the correction cycle (F4)",
    )
    .on_hover_text("Digital style: the letter goes without a trace");
}

/// A small gear right of the calm mode icon. Fades with the chrome.
pub fn gear_icon(ui: &mut Ui, view: Rect, opacity: f32) -> bool {
    let centre = view.left_bottom() + vec2(160.0, -31.0);
    let icon = DeskIcon {
        hit: Rect::from_center_size(centre, vec2(30.0, 30.0)),
        id: "settings-icon",
        tip: "Settings",
    };
    icon.show(ui, view, opacity, |painter, hovered| {
        let edge = if hovered { HIGHLIGHT } else { ICON_EDGE };
        let outline = gear(centre, 12.0, 9.0, 8);
        // Not convex: fill as a disc plus a quad per tooth.
        painter.circle_filled(centre, 9.0, ICON_FILL);
        let n = outline.len();
        for i in (0..n).step_by(4) {
            // Root before, two tips, root after.
            let quad = vec![
                outline[(i + n - 1) % n],
                outline[i],
                outline[i + 1],
                outline[i + 2],
            ];
            painter.add(Shape::convex_polygon(quad, ICON_FILL, Stroke::NONE));
        }
        painter.add(Shape::closed_line(outline, Stroke::new(1.0, edge)));
        painter.circle(centre, 4.0, Color32::TRANSPARENT, Stroke::new(1.2, edge));
    })
}

/// A gear outline: `teeth` flat-topped teeth between two radii.
fn gear(centre: Pos2, outer: f32, inner: f32, teeth: u16) -> Vec<Pos2> {
    let steps = teeth * 4;
    (0..steps)
        .map(|i| {
            let angle = TAU * f32::from(i) / f32::from(steps);
            // Two points on the tooth, two in the gap.
            let radius = if i % 4 < 2 { outer } else { inner };
            centre + Vec2::angled(angle) * radius
        })
        .collect()
}
