//! The desk edition's typewriter, an Olympia SM9 seen from the chair.
//!
//! The body is a model in inches, projected from a seated eye: `x` right of
//! the machine's centre, `y` toward the writer, `z` up, the origin at the
//! printing point. There the projection's scale is the sheet's, so the flat
//! sheet and the carriage (platen, knobs, bail), which travel with it, line
//! up with the still body around them.
//!
//! Nothing sorts by depth: each part is drawn after what it hides, in the
//! order [`paint_behind`] and [`paint_front`] call them.

mod body;
mod card_holder;
mod carriage;
mod case;
mod cover;
mod eye;
mod geometry;
mod keyboard;
mod light;
mod panel;
mod side_controls;

pub use carriage::{bail_scale_top, paint_bail, platen_ends};
pub use panel::{Control, Panel, PanelState};

use eframe::egui::{Color32, Painter, Rect};

use crate::render::Metrics;
use card_holder::TABLE_FRONT;
use eye::Eye;
use panel::{CONTROLS_Y, INDEX_MARKS, KNOB_RADIUS, on_panel, panel_offset};

/// The typing line's height in the view: leaning over the page at 100 %
/// zoom and above, sitting back to see the whole machine at 50 %. Never
/// lower than keeps the panel's controls in view, nor higher than the least.
const TYPING_LINE_LEANING: f32 = 0.62;
const TYPING_LINE_SITTING: f32 = 0.25;
const TYPING_LINE_LEAST: f32 = 0.2;
/// Between the panel's readings and the window's bottom edge.
const CONTROLS_ROOM: f32 = 12.0;

// What more than one part is made of; the rest is with its part.
const METAL: Color32 = Color32::from_rgb(0x8C, 0x8E, 0x8C);
const METAL_SHINE: Color32 = Color32::from_rgb(0xDC, 0xDE, 0xDA);
const CHROME: Color32 = Color32::from_rgb(0xC8, 0xCA, 0xC6);
const IVORY_LIT: Color32 = Color32::from_rgb(0xEE, 0xEC, 0xE2);
const IVORY: Color32 = Color32::from_rgb(0xE2, 0xDF, 0xD2);
const IVORY_SHADE: Color32 = Color32::from_rgb(0xC6, 0xC2, 0xB3);
const EDGE: Color32 = Color32::from_rgb(0x9C, 0x97, 0x88);
const DARK: Color32 = Color32::from_rgb(0x1A, 0x1A, 0x19);
const STEM: Color32 = Color32::from_rgb(0x4C, 0x4C, 0x48);
const STEM_SHINE: Color32 = Color32::from_rgb(0xE8, 0xE8, 0xE2);
const SHIFT_CAP: Color32 = Color32::from_rgb(0x5E, 0xB2, 0x9C);
const SHIFT_FRONT: Color32 = Color32::from_rgb(0x3F, 0x8C, 0x78);
const ENGRAVED: Color32 = Color32::from_rgb(0x6E, 0x6A, 0x5E);

/// The typing line's share of `view`'s height at `zoom_percent`, keeping
/// the panel's controls in view where it can.
pub fn typing_line_height(view: Rect, metrics: &Metrics, zoom_percent: u16) -> f32 {
    let leaning = ((f32::from(zoom_percent) - 50.0) / 50.0).clamp(0.0, 1.0);
    let wanted = TYPING_LINE_SITTING + (TYPING_LINE_LEANING - TYPING_LINE_SITTING) * leaning;
    let eye = Eye::new(view, metrics, 0.0);
    let reach = KNOB_RADIUS * INDEX_MARKS.1 + 0.1;
    let readings = eye
        .at(panel_offset(on_panel(0.0, CONTROLS_Y), reach, 180.0))
        .y;
    let height = view.height().max(1.0);
    let in_view = (height - readings - CONTROLS_ROOM) / height;
    wanted.min(in_view).max(TYPING_LINE_LEAST)
}

/// Where the sheet goes out of sight into the machine, for the typing line
/// at `typing_y`: nothing of it shows below.
pub fn sheet_bottom(view: Rect, metrics: &Metrics, typing_y: f32) -> f32 {
    let eye = Eye::new(view, metrics, typing_y);
    eye.at([0.0, TABLE_FRONT.0, TABLE_FRONT.1]).y
}

/// Behind the sheet, before it: the machine's shadow, the deck the carriage
/// rides on, the carriage for the sheet centred at `carriage_x`, and the
/// throat under the platen.
pub fn paint_behind(
    painter: &Painter,
    view: Rect,
    metrics: &Metrics,
    typing_y: f32,
    carriage_x: f32,
) {
    let eye = Eye::new(view, metrics, typing_y);
    body::paint_shadow(painter, &eye);
    body::paint_deck(painter, &eye);
    carriage::paint(painter, metrics, typing_y, carriage_x);
    body::paint_throat(painter, &eye);
}

/// In front of the sheet, after it: the card holder at the printing point,
/// the ribbon cover over the type bars, the front panel and the keyboard.
pub fn paint_front(painter: &Painter, view: Rect, metrics: &Metrics, typing_y: f32) {
    let eye = Eye::new(view, metrics, typing_y);
    card_holder::paint(painter, &eye, metrics);
    cover::paint(painter, &eye);
    panel::paint_face(painter, &eye);
    // The keyboard, deepest first: the levers run back under the rows behind
    // and in under the panel's edge, the caps hide them, and the case round
    // the keys hides the caps' feet.
    let levers = keyboard::key_levers();
    case::paint_well(painter, &eye);
    keyboard::paint_rod(painter, &eye, &levers);
    case::paint_inner_walls(painter, &eye);
    keyboard::paint_shadows(painter, &eye);
    keyboard::paint_levers(painter, &eye, &levers);
    side_controls::paint(painter, &eye);
    case::paint_panel_edge(painter, &eye);
    keyboard::paint_caps(painter, &eye);
    case::paint_frame(painter, &eye);
    side_controls::paint_marks(painter, &eye);
}

#[cfg(test)]
mod tests {
    use super::body::DESK_Z;
    use super::case::CASE_FRONT;
    use super::*;
    use eframe::egui::{Pos2, vec2};

    fn metrics(zoom_percent: u16) -> Metrics {
        let sm9 = include_str!("../../../../../profiles/olympia-sm9.toml");
        let profile = typewriter_core::Profile::from_toml_str(sm9).unwrap();
        Metrics::new(&profile, super::super::points_per_inch(zoom_percent))
    }

    #[test]
    fn sitting_back_raises_the_typing_line() {
        // So tall the controls are always in view.
        let tall = Rect::from_min_size(Pos2::ZERO, vec2(1600.0, 4000.0));
        let at = |zoom| typing_line_height(tall, &metrics(zoom), zoom);
        assert_eq!(at(100), TYPING_LINE_LEANING);
        assert_eq!(at(200), TYPING_LINE_LEANING);
        assert_eq!(at(50), TYPING_LINE_SITTING);
        assert!(at(75) < TYPING_LINE_LEANING);
    }

    #[test]
    fn the_controls_stay_in_view_up_to_life_size() {
        let view = Rect::from_min_size(Pos2::ZERO, vec2(1536.0, 960.0));
        for zoom in [50, 80, 100] {
            let metrics = metrics(zoom);
            let typing_y = view.height() * typing_line_height(view, &metrics, zoom);
            let panel = Panel::new(view, &metrics, typing_y);
            for control in Control::ALL {
                let rect = panel.rect(control);
                assert!(
                    view.contains_rect(rect),
                    "{zoom} %: {control:?} at {rect:?}"
                );
            }
        }
    }

    #[test]
    fn the_whole_machine_fits_below_the_typing_line_sitting_back() {
        // 50 %: 48 points an inch, in a view 1000 points high.
        let eye = Eye::testing(1000.0 * TYPING_LINE_SITTING, 48.0);
        let front = eye.at([0.0, CASE_FRONT, DESK_Z]);
        assert!(front.y < 1000.0, "{front:?}");
    }
}
