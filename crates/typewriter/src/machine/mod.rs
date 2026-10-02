//! The typewriter on the desk, an Olympia SM9 seen from the chair.
//!
//! The machine is a model in millimetres, projected from a seated eye: `x` right
//! of the machine's centre, `y` toward the writer, `z` up, the origin at the
//! printing point. There the projection's scale is the sheet's: the line
//! being typed shows as on a sheet flat on screen.
//!
//! The machine is drawn in depth, in one pass a frame from [`paint_behind`]
//! to [`paint_front`]: what stands in front hides what is behind, whatever
//! the order. Flat are only its shadow on the desk, under it all, and the
//! panel's controls ([`Panel::paint`]), over it.

mod bail;
mod body;
mod canvas;
mod carriage;
mod case;
mod cover;
mod eye;
mod geometry;
mod keyboard;
mod knob;
mod lever;
mod light;
mod panel;
mod printing_point;
mod sheet;
mod side_controls;
mod support;

pub use lever::Throw;
pub use panel::{Control, Panel};
pub use sheet::paint_sheets;
pub use support::paper_table;

use eframe::egui::{Color32, Painter, Rect};

use crate::depth;
use canvas::Canvas;
use eye::Eye;
use panel::{CONTROLS_Y, INDEX_MARKS, KNOB_RADIUS, on_panel, panel_offset};
use typewriter_core::carriage::Carriage;
use typewriter_ui::draw::Metrics;
use typewriter_ui::draw::ruler::Scale;

/// The typing line's height in the view: leaning over the page at 100 %
/// zoom and above, sitting back to see the whole machine at 50 %, and far
/// back at 25 %, lower again to see the paper support and its scale. Never
/// lower than keeps the panel's controls in view, nor higher than the least.
const TYPING_LINE_LEANING: f32 = 0.62;
const TYPING_LINE_SITTING: f32 = 0.25;
const TYPING_LINE_FAR: f32 = 0.55;
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
const STEM: Color32 = Color32::from_rgb(0x4C, 0x4C, 0x48);
const STEM_SHINE: Color32 = Color32::from_rgb(0xE8, 0xE8, 0xE2);
const SHIFT_CAP: Color32 = Color32::from_rgb(0x5E, 0xB2, 0x9C);
const SHIFT_FRONT: Color32 = Color32::from_rgb(0x3F, 0x8C, 0x78);
const ENGRAVED: Color32 = Color32::from_rgb(0x6E, 0x6A, 0x5E);

/// The typing line's share of `view`'s height at `zoom_percent`, keeping
/// the panel's controls in view where it can.
pub fn typing_line_height(view: Rect, metrics: &Metrics, zoom_percent: u16) -> f32 {
    let zoom = f32::from(zoom_percent);
    let wanted = if zoom < 50.0 {
        let far = ((50.0 - zoom) / 25.0).clamp(0.0, 1.0);
        TYPING_LINE_SITTING + (TYPING_LINE_FAR - TYPING_LINE_SITTING) * far
    } else {
        let leaning = ((zoom - 50.0) / 50.0).clamp(0.0, 1.0);
        TYPING_LINE_SITTING + (TYPING_LINE_LEANING - TYPING_LINE_SITTING) * leaning
    };
    let eye = Eye::new(view, metrics, 0.0);
    let reach = KNOB_RADIUS * INDEX_MARKS.1 + 3.0;
    let readings = eye
        .at(panel_offset(on_panel(0.0, CONTROLS_Y), reach, 180.0))
        .y;
    let height = view.height().max(1.0);
    let in_view = (height - readings - CONTROLS_ROOM) / height;
    wanted.min(in_view).max(TYPING_LINE_LEAST)
}

/// Behind the sheet, before it: the machine's shadow, the body's top under
/// the carriage, the paper support, and the carriage for the sheet centred
/// at `carriage_x` with its bail and its return lever, thrown `throw`
/// (0..=1). Begins the frame's depth pass.
pub fn paint_behind(
    painter: &Painter,
    view: Rect,
    metrics: &Metrics,
    typing_y: f32,
    carriage_x: f32,
    throw: f32,
) {
    let eye = Eye::new(view, metrics, typing_y);
    body::paint_shadow(painter, &eye);
    depth::begin(painter, light::frame(), eye.camera());
    let canvas = Canvas::depth(painter);
    body::paint_deck(&canvas, &eye);
    let middle = (carriage_x - eye.origin.x) / eye.ppmm;
    support::paint(&canvas, &eye, metrics, middle);
    carriage::paint(&canvas, &eye, middle);
    bail::paint(&canvas, &eye, middle);
    let [left, _] = carriage::ends(middle);
    lever::paint(&canvas, &eye.about(carriage::platen_axis()), left, throw);
    canvas.finish();
}

/// `scale`, laid out on screen at the sheet's scale, and `carriage`'s stops
/// printed on the paper bail: where it shows, for its stops. Into the
/// frame's depth pass.
pub fn print_scale(
    painter: &Painter,
    view: Rect,
    metrics: &Metrics,
    typing_y: f32,
    scale: &Scale,
    carriage: &Carriage,
) -> Scale {
    let eye = Eye::new(view, metrics, typing_y);
    let canvas = Canvas::depth(painter);
    let printed = bail::print_scale(&canvas, &eye, scale, carriage);
    canvas.finish();
    printed
}

/// The platen knobs at the carriage's ends, for the sheet centred at
/// `carriage_x`, turned by `rolled` points of paper, each lit if its grip
/// is `hovered`: their grips on screen, left then right. Into the frame's
/// depth pass.
pub fn paint_knobs(
    painter: &Painter,
    view: Rect,
    metrics: &Metrics,
    typing_y: f32,
    (carriage_x, rolled): (f32, f32),
    hovered: impl Fn(Rect) -> bool,
) -> [Rect; 2] {
    let eye = Eye::new(view, metrics, typing_y);
    let middle = (carriage_x - eye.origin.x) / eye.ppmm;
    let ends = carriage::ends(middle);
    let eye = eye.about(carriage::platen_axis());
    let grips = knob::grips(&eye, ends);
    let canvas = Canvas::depth(painter);
    let turned = rolled / eye.ppmm / knob::DISC.1;
    knob::paint(&canvas, &eye, ends, turned, grips.map(hovered));
    canvas.finish();
    grips
}

/// In front of the sheet, after it: the alignment guide, ribbon and card
/// holder at the printing point, the ribbon cover over the type bars, the
/// front panel and the keyboard. Ends the frame's depth pass.
pub fn paint_front(painter: &Painter, view: Rect, metrics: &Metrics, typing_y: f32) {
    let eye = Eye::new(view, metrics, typing_y);
    let canvas = Canvas::depth(painter);
    cover::paint_opening(&canvas, &eye);
    printing_point::paint(&canvas, &eye, metrics);
    cover::paint(&canvas, &eye);
    panel::paint_face(&canvas, &eye);
    let levers = keyboard::key_levers();
    case::paint_well(&canvas, &eye);
    keyboard::paint_rod(&canvas, &eye, &levers);
    case::paint_inner_walls(&canvas, &eye);
    // Decals blend in order: the levers over the shadows.
    keyboard::paint_shadows(&canvas, &eye);
    keyboard::paint_levers(&canvas, &eye, &levers);
    side_controls::paint(&canvas, &eye);
    case::paint_panel_edge(&canvas, &eye);
    keyboard::paint_caps(&canvas, &eye);
    case::paint_frame(&canvas, &eye);
    side_controls::paint_marks(&canvas, &eye);
    canvas.finish();
    // The behind, the sheets and this in one pass.
    depth::end(painter);
}

#[cfg(test)]
mod tests {
    use super::body::DESK_Z;
    use super::case::CASE_FRONT;
    use super::*;
    use eframe::egui::{Pos2, vec2};

    fn metrics(zoom_percent: u16) -> Metrics {
        let sm9 = include_str!("../../../../profiles/olympia-sm9.toml");
        let profile = typewriter_core::Profile::from_toml_str(sm9).unwrap();
        Metrics::new(&profile, typewriter_ui::draw::points_per_inch(zoom_percent))
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
        assert_eq!(at(25), TYPING_LINE_FAR);
        assert!(at(35) > TYPING_LINE_SITTING && at(35) < TYPING_LINE_FAR);
    }

    #[test]
    fn far_back_a_whole_sheet_fits_above_the_typing_line() {
        // Where the support's scale reads the sheet's top edge near its end.
        let view = Rect::from_min_size(Pos2::ZERO, vec2(1536.0, 960.0));
        let metrics = metrics(25);
        let typing_y = view.height() * typing_line_height(view, &metrics, 25);
        assert!(typing_y > metrics.paper_size.y, "{typing_y}");
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

    #[test]
    fn every_vertex_stands_where_it_is_drawn() {
        // The GPU draws where a solid's millimetres show, the CPU twin
        // where its vertices stand: `depth::end` checks the two agree, so
        // a part cannot go missing on screen unseen by snapshots.
        let ctx = eframe::egui::Context::default();
        let mut output = ctx.run_ui(Default::default(), |ui| {
            let painter = ui.painter();
            let view = Rect::from_min_size(Pos2::ZERO, vec2(1600.0, 1000.0));
            let metrics = metrics(100);
            let typing_y = view.height() * typing_line_height(view, &metrics, 100);
            paint_behind(painter, view, &metrics, typing_y, 800.0, 0.0);
            paint_knobs(painter, view, &metrics, typing_y, (800.0, 0.0), |_| false);
            paint_front(painter, view, &metrics, typing_y);
        });
        output.textures_delta.clear();
    }
}
