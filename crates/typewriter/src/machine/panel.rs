//! The front panel falling to the keyboard, and its controls where the
//! maker's badge would be: spacing, zoom, correction, goal and save.

use eframe::egui::{Align2, Color32, Painter, Rect, Shape, Stroke};
use typewriter_core::{EraseMode, LineSpacing};

use super::canvas::Canvas;
use super::cover::{COVER_FRONT, COVER_HALF};
use super::eye::{Eye, paint_flat_text};
use super::geometry::{add, dot};
use super::light::{matte, toward_light};
use super::{CHROME, ENGRAVED, IVORY, SHIFT_CAP, SHIFT_FRONT};
use typewriter_ui::draw::{Controls, HIGHLIGHT, Metrics, ruler};
use typewriter_ui::settings::{ZOOM_NOTCHES, zoom_notch};

/// The front panel, falling from the cover's fold to the keyboard's opening.
pub(super) const PANEL_BOTTOM: (f32, f32) = (119.0, -60.0);
pub(super) const PANEL_HALF_BOTTOM: f32 = 164.0;
/// The controls where the maker's badge would be: across the panel at `y`,
/// their names and readings below.
pub(super) const CONTROLS_Y: f32 = 91.0;
/// Knob to its name and reading beside it; their lines, above and below
/// the knob's centre.
const LABEL_GAP: f32 = 13.0;
/// The save label further out still, past the lamp.
const SAVE_LABEL_PAST: f32 = 8.0;
const LABEL_LINES: (f32, f32) = (-2.0, 2.0);
pub(super) const KNOB_RADIUS: f32 = 8.0;
const KNOB_HEIGHT: f32 = 6.0;
const BUTTON_HEIGHT: f32 = 3.0;
/// Past the knob, where its index marks are engraved: inner and outer.
pub(super) const INDEX_MARKS: (f32, f32) = (1.25, 1.5);
const LAMP_RADIUS: f32 = 2.0;
/// The lamp's middle out from the save button's, its chrome rim round it:
/// clear of the button and its shadow, short of the label.
const LAMP_OUT: f32 = 14.0;
const LAMP_RIM: f32 = 1.4 * LAMP_RADIUS;
const _: () = assert!(LAMP_OUT + LAMP_RIM < LABEL_GAP + SAVE_LABEL_PAST);
const KNOB_TOP: Color32 = Color32::from_rgb(0xEC, 0xE7, 0xD6);
const KNOB_SIDE: Color32 = Color32::from_rgb(0xCC, 0xC5, 0xAF);
const KNOB_RIB: Color32 = Color32::from_rgb(0xAE, 0xA6, 0x8E);
const READING: Color32 = Color32::from_rgb(0x2A, 0x28, 0x25);
const LAMP_OFF: Color32 = Color32::from_rgb(0x4A, 0x4A, 0x46);

/// The front panel, falling from the cover to the keyboard's opening.
pub(super) fn paint_face(canvas: &Canvas, eye: &Eye) {
    let (y0, z0) = COVER_FRONT;
    let (y1, z1) = PANEL_BOTTOM;
    let (top, bottom) = (COVER_HALF.1, PANEL_HALF_BOTTOM);
    let outline = [
        [-top, y0, z0],
        [top, y0, z0],
        [bottom, y1, z1],
        [-bottom, y1, z1],
    ];
    // One slope, one normal: the lamp washes it the way the gradient faked.
    eye.fill(canvas, &outline, |_| matte(IVORY, panel_normal()));
}

/// A point on the front panel's slope.
pub(super) fn on_panel(x: f32, y: f32) -> [f32; 3] {
    let (y0, z0) = COVER_FRONT;
    let (y1, z1) = PANEL_BOTTOM;
    [x, y, z0 + (y - y0) / (y1 - y0) * (z1 - z0)]
}

/// Down the panel's slope, toward the writer, unit length.
fn panel_down() -> [f32; 3] {
    let (y0, z0) = COVER_FRONT;
    let (y1, z1) = PANEL_BOTTOM;
    let (dy, dz) = (y1 - y0, z1 - z0);
    let length = (dy * dy + dz * dz).sqrt();
    [0.0, dy / length, dz / length]
}

/// Out of the panel, square to it, unit length.
fn panel_normal() -> [f32; 3] {
    let [_, dy, dz] = panel_down();
    [0.0, -dz, dy]
}

/// On the panel's plane from `centre`, `radius` millimetres toward `turn`
/// degrees clockwise from the top, the top being up the slope.
pub(super) fn panel_offset(centre: [f32; 3], radius: f32, turn: f32) -> [f32; 3] {
    let down = panel_down();
    let (sin, cos) = turn.to_radians().sin_cos();
    [0, 1, 2].map(|k| {
        let right = if k == 0 { 1.0 } else { 0.0 };
        centre[k] + radius * (sin * right - cos * down[k])
    })
}

/// Where the light casts `point`, `height` millimetres off the panel, onto it.
fn cast_on_panel(point: [f32; 3], height: f32) -> [f32; 3] {
    let light = toward_light();
    let along = height / dot(light, panel_normal());
    [0, 1, 2].map(|k| point[k] - light[k] * along)
}

/// A circle on the panel's plane.
fn circle(centre: [f32; 3], radius: f32) -> Vec<[f32; 3]> {
    (0..32u8)
        .map(|i| panel_offset(centre, radius, 360.0 * f32::from(i) / 32.0))
        .collect()
}

/// The panel's controls, left to right.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Control {
    Spacing,
    Zoom,
    Correct,
    Goal,
    Save,
}

impl Control {
    pub const ALL: [Self; 5] = [
        Self::Spacing,
        Self::Zoom,
        Self::Correct,
        Self::Goal,
        Self::Save,
    ];

    /// Across the panel, in millimetres from its centre: where the maker's
    /// badge would be on the left, where its emblem would be on the right.
    fn x(self) -> f32 {
        match self {
            Self::Spacing => -142.0,
            Self::Zoom => -104.0,
            Self::Correct => -58.0,
            Self::Goal => 30.0,
            Self::Save => 114.0,
        }
    }

    /// Room for the name and reading beside it, in millimetres.
    fn label_width(self) -> f32 {
        match self {
            Self::Spacing => 13.0,
            Self::Zoom => 20.0,
            Self::Correct => 30.0,
            Self::Goal => 56.0,
            Self::Save => 18.0,
        }
    }

    /// Where its name and reading start: past the knob, or the button and
    /// its lamp.
    fn label_x(self) -> f32 {
        let past = if self == Self::Save {
            SAVE_LABEL_PAST
        } else {
            0.0
        };
        self.x() + LABEL_GAP + past
    }

    fn name(self) -> &'static str {
        match self {
            Self::Spacing => "SPACING",
            Self::Zoom => "ZOOM",
            Self::Correct => "CORRECT",
            Self::Goal => "GOAL",
            Self::Save => "SAVE",
        }
    }
}

/// The front panel's controls for the typing line at `typing_y`.
pub struct Panel {
    eye: Eye,
}

impl Panel {
    pub fn new(view: Rect, metrics: &Metrics, typing_y: f32) -> Self {
        Self {
            eye: Eye::new(view, metrics, typing_y),
        }
    }

    /// Where `control` takes a click: the knob, its marks and its labels.
    pub fn rect(&self, control: Control) -> Rect {
        let (left, reach) = (
            control.x() - KNOB_RADIUS * INDEX_MARKS.1,
            KNOB_RADIUS * INDEX_MARKS.1,
        );
        let right = control.label_x() + control.label_width();
        let [left, right] = [left, right].map(|x| on_panel(x, CONTROLS_Y));
        let corners = [
            panel_offset(left, reach, 0.0),
            panel_offset(right, reach, 0.0),
            panel_offset(right, reach, 180.0),
            panel_offset(left, reach, 180.0),
        ];
        Rect::from_points(&self.eye.polygon(&corners))
    }

    /// Draws each control, `hovered` ringed.
    pub fn paint(&self, painter: &Painter, state: &Controls, hovered: Option<Control>) {
        let canvas = &Canvas::flat(painter);
        for control in Control::ALL {
            let lit = hovered == Some(control);
            let (turn, marks, reading, colour) = match control {
                Control::Spacing => {
                    let notch = match state.spacing {
                        LineSpacing::Single => 0,
                        LineSpacing::OneAndHalf => 1,
                        LineSpacing::Double => 2,
                    };
                    let marks = notches(3, 100.0);
                    let reading = ["1", "1½", "2"][notch].to_owned();
                    (marks[notch], marks, reading, READING)
                }
                Control::Zoom => {
                    // A notch apart, evenly: most of the turn is above 70 %.
                    let last = (ZOOM_NOTCHES.len() - 1) as f32;
                    let share = zoom_notch(state.zoom_percent) as f32 / last;
                    let turn = -135.0 + 270.0 * share;
                    let reading = format!("{} %", state.zoom_percent);
                    (turn, notches(4, 270.0), reading, READING)
                }
                Control::Correct => {
                    let mut ways = vec![EraseMode::Paper, EraseMode::Eraser, EraseMode::Fluid];
                    if state.delete_in_cycle || state.erase == EraseMode::Delete {
                        ways.push(EraseMode::Delete);
                    }
                    let marks = notches(ways.len(), 40.0 * (ways.len() - 1) as f32);
                    let way = ways.iter().position(|&w| w == state.erase).unwrap_or(0);
                    let reading = ruler::correction_method(state.erase, state.slip_in).to_owned();
                    (marks[way], marks, reading, READING)
                }
                Control::Goal => {
                    // Off first, then the plate's cycle.
                    let positions = state.goals.len() + 1;
                    let span = (30.0 * (positions - 1) as f32).min(270.0);
                    let marks = notches(positions, span);
                    let at = state
                        .goal
                        .and_then(|goal| state.goals.iter().position(|&g| g == goal))
                        .map_or(0, |i| i + 1);
                    let reached = state.progress.is_some_and(|p| p.reached);
                    let colour = if reached { ruler::SAVED } else { READING };
                    (
                        marks[at],
                        marks,
                        ruler::goal_reading(state.progress),
                        colour,
                    )
                }
                Control::Save => {
                    let (reading, lamp) = ruler::autosave_state(state.keeping);
                    self.paint_button(canvas, control, lit, lamp);
                    self.paint_labels(canvas, control, reading, READING);
                    continue;
                }
            };
            self.paint_knob(canvas, control, lit, turn, &marks);
            self.paint_labels(canvas, control, &reading, colour);
        }
    }

    /// A ribbed knob standing off the panel, its pointer at `turn` degrees
    /// clockwise from the top, index marks engraved round it at `marks`.
    fn paint_knob(&self, canvas: &Canvas, control: Control, lit: bool, turn: f32, marks: &[f32]) {
        let eye = &self.eye;
        let centre = on_panel(control.x(), CONTROLS_Y);
        for &mark in marks {
            let [inner, outer] =
                [INDEX_MARKS.0, INDEX_MARKS.1].map(|r| panel_offset(centre, KNOB_RADIUS * r, mark));
            eye.line(canvas, &[inner, outer], 0.64, ENGRAVED);
        }
        let top = self.paint_cylinder(canvas, centre, KNOB_HEIGHT, lit, [KNOB_TOP, KNOB_SIDE]);
        let [from, to] = [0.25, 0.85].map(|r| panel_offset(top, KNOB_RADIUS * r, turn));
        eye.line(canvas, &[from, to], 1.27, READING);
    }

    /// The save button, flush, and its lamp beside it: lit in `lamp`'s
    /// colour, dark if `None`.
    fn paint_button(&self, canvas: &Canvas, control: Control, lit: bool, lamp: Option<Color32>) {
        let eye = &self.eye;
        let centre = on_panel(control.x(), CONTROLS_Y);
        self.paint_cylinder(canvas, centre, BUTTON_HEIGHT, lit, [SHIFT_CAP, SHIFT_FRONT]);
        let bulb = on_panel(control.x() + LAMP_OUT, CONTROLS_Y);
        let rim = circle(bulb, LAMP_RIM);
        eye.fill(canvas, &rim, |_| CHROME);
        let glass = circle(bulb, LAMP_RADIUS);
        eye.fill(canvas, &glass, |_| lamp.unwrap_or(LAMP_OFF));
        let glint = circle(
            panel_offset(bulb, LAMP_RADIUS * 0.4, -40.0),
            LAMP_RADIUS * 0.3,
        );
        eye.fill(canvas, &glint, |_| Color32::from_white_alpha(120));
    }

    /// A short cylinder on the panel at `base`, `height` tall: its shadow,
    /// ribbed side and top. Returns its top's centre.
    fn paint_cylinder(
        &self,
        canvas: &Canvas,
        base: [f32; 3],
        height: f32,
        lit: bool,
        [top_colour, side_colour]: [Color32; 2],
    ) -> [f32; 3] {
        let eye = &self.eye;
        let normal = panel_normal();
        let top = [0, 1, 2].map(|k| base[k] + normal[k] * height);
        let shadow = circle(cast_on_panel(top, height), KNOB_RADIUS);
        eye.fill(canvas, &shadow, |_| Color32::from_black_alpha(60));
        // The side: the base's near half, the top's far half.
        let steps = 24u8;
        let at = |centre: [f32; 3], i: u8| {
            let turn = 90.0 + 360.0 * f32::from(i) / f32::from(steps);
            panel_offset(centre, KNOB_RADIUS, turn)
        };
        let half = steps / 2;
        let side: Vec<[f32; 3]> = (0..=half)
            .map(|i| at(base, i))
            .chain((half..=steps).map(|i| at(top, i)))
            .collect();
        eye.fill(canvas, &side, |_| side_colour);
        for i in (1..half).step_by(2) {
            eye.line(canvas, &[at(base, i), at(top, i)], 0.5, KNOB_RIB);
        }
        let face = circle(top, KNOB_RADIUS);
        eye.fill(canvas, &face, |_| top_colour);
        let rim = if lit { HIGHLIGHT } else { side_colour };
        canvas.painter().add(Shape::closed_line(
            eye.polygon(&face),
            Stroke::new(if lit { 2.0 } else { 1.0 }, rim),
        ));
        top
    }

    /// `control`'s engraved name beside it, and its reading under the name.
    fn paint_labels(&self, canvas: &Canvas, control: Control, reading: &str, colour: Color32) {
        let eye = &self.eye;
        let (name_y, reading_y) = LABEL_LINES;
        // On the panel's slope, like the knobs.
        let on = |line: f32| {
            let start = on_panel(control.label_x(), CONTROLS_Y + line);
            move |across: f32, down: f32| {
                add(
                    add(start, [across, 0.0, 0.0]),
                    panel_offset([0.0; 3], down, 180.0),
                )
            }
        };
        let left = Align2::LEFT_CENTER;
        paint_flat_text(canvas, eye, control.name(), 3.0, ENGRAVED, left, on(name_y));
        paint_flat_text(canvas, eye, reading, 4.0, colour, left, on(reading_y));
    }
}

/// `count` index marks spread over `span` degrees, centred on the top.
fn notches(count: usize, span: f32) -> Vec<f32> {
    if count < 2 {
        return vec![0.0];
    }
    (0..count)
        .map(|i| -span / 2.0 + span * i as f32 / (count - 1) as f32)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_controls_sit_apart_on_the_panel() {
        let reach = KNOB_RADIUS * INDEX_MARKS.1;
        let spans: Vec<(f32, f32)> = Control::ALL
            .iter()
            .map(|c| (c.x() - reach, c.label_x() + c.label_width()))
            .collect();
        for pair in spans.windows(2) {
            assert!(pair[0].1 <= pair[1].0 + 2.5e-3, "{pair:?}");
        }
        let half = PANEL_HALF_BOTTOM - 8.0;
        assert!(
            spans
                .iter()
                .all(|&(left, right)| left > -half && right < half)
        );
    }

    #[test]
    fn shadows_clear_of_the_lamp_and_labels() {
        let shadow = |height: f32| {
            let top = add(
                on_panel(0.0, CONTROLS_Y),
                panel_normal().map(|n| n * height),
            );
            cast_on_panel(top, height)
        };
        let lamp = on_panel(LAMP_OUT, CONTROLS_Y);
        let apart = [0, 1, 2].map(|k| lamp[k] - shadow(BUTTON_HEIGHT)[k]);
        let gap = dot(apart, apart).sqrt();
        assert!(gap > KNOB_RADIUS + LAMP_RIM, "{gap}");
        assert!(shadow(KNOB_HEIGHT)[0] + KNOB_RADIUS < LABEL_GAP);
    }
}
