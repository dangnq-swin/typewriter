//! At the printing point: the alignment guide's perspex plates over the line
//! being typed, a scale stuck along their feet with a line-drawing notch at
//! its end; the ribbon, black over red, through its vibrator below the line;
//! the card holder's wire.
//!
//! The typing line's cell runs down from the origin a sixth of an inch.

use std::f32::consts::PI;

use eframe::egui::{Color32, Painter, Rect, Shape, Stroke, pos2};

use super::eye::Eye;
use super::geometry::rounded;
use super::light::{matte, paint_chrome};
use super::{EDGE, METAL_SHINE};
use typewriter_app::draw::Metrics;

/// The alignment guide's plates, pressed on the paper either side of the
/// printing point, `(y, z)`: their foot down behind the ribbon, their top
/// leaning back with the platen. Their inner and outer `x`. The line being
/// typed shows through them.
const GUIDE_FOOT: (f32, f32) = (0.09, -0.5);
const GUIDE_TOP: (f32, f32) = (-0.1, 0.27);
const GUIDE_X: (f32, f32) = (0.7, 3.8);
/// The plates' top corners, rounded: outside and in.
const OUTER_TOP_ROUNDING: f32 = 0.25;
const INNER_TOP_ROUNDING: f32 = 0.08;
/// The scale stuck along each plate's foot, white marks on black: its top at
/// the typing line's foot, `z`, and its depth down the plate, the cover's
/// back edge hiding its foot; its marks, on the columns' edges out from the
/// notch, and their length.
const SCALE_Z: f32 = -0.2;
const SCALE_DEPTH: f32 = 0.2;
const SCALE_MARKS: i32 = 12;
const MARK_LENGTH: f32 = 0.07;
/// The line-drawing notch at each scale's inner end, a pencil's point in its
/// V: in from the plate's inner edge, and its width.
const PENCIL_NOTCH: (f32, f32) = (0.05, 0.14);
/// The ribbon, on edge in front of the paper: its `y`, top `z` and width.
/// Straight across under the line, over the cover's back edge, out to `x`;
/// then forward to its spools under the cover, out to `(x, y)`, dropping to
/// pass under it.
const RIBBON: (f32, f32, f32) = (0.12, -0.28, 0.5);
const RIBBON_ACROSS: f32 = 3.6;
const RIBBON_OUT: (f32, f32, f32) = (4.4, 0.9, 0.65);
/// The ribbon vibrator, holding the ribbon below the line until a key
/// lifts it: its `y`, top and bottom `z`, its body's half width, its lugs'
/// inner and outer `x`, and the slot in its foot (half width, height).
const VIBRATOR: (f32, f32, f32) = (0.15, -0.26, -0.74);
const VIBRATOR_HALF: f32 = 0.3;
const LUGS: (f32, f32) = (0.4, 0.47);
const VIBRATOR_SLOT: (f32, f32) = (0.08, 0.2);
/// The card holder: a wire arch from the vibrator up the guide's face, half
/// its width.
const CARD_HOLDER_HALF: f32 = 0.27;
/// Smoked perspex.
const GLASS: Color32 = Color32::from_rgba_unmultiplied_const(0x9C, 0xA4, 0xA6, 0x34);
const GLASS_EDGE: Color32 = Color32::from_rgba_unmultiplied_const(0x70, 0x74, 0x72, 0xB0);
const SCALE: Color32 = Color32::from_rgb(0x16, 0x16, 0x17);
const SCALE_MARK: Color32 = Color32::from_rgb(0xEC, 0xEC, 0xE6);
const RIBBON_INK: Color32 = Color32::from_rgb(0x1E, 0x1D, 0x20);
const RIBBON_RED: Color32 = Color32::from_rgb(0xB0, 0x2C, 0x28);

/// Behind the guide first, then what stands in front of it. The guide only
/// between the carriage's side plates, `inside` on screen: they pass behind
/// it, and its edges would cross them.
pub(super) fn paint(painter: &Painter, eye: &Eye, metrics: &Metrics, inside: [f32; 2]) {
    let clip = painter.clip_rect();
    let guide = painter.with_clip_rect(Rect::from_x_y_ranges(
        inside[0].max(clip.left())..=inside[1].min(clip.right()),
        clip.y_range(),
    ));
    paint_plates(&guide, eye);
    paint_scale(&guide, eye, metrics);
    paint_ribbon(painter, eye);
    paint_card_holder(painter, eye);
    paint_vibrator(painter, eye);
}

/// A point on the guide's face, `t` from its foot (0) to its top (1).
fn on_guide(x: f32, t: f32) -> [f32; 3] {
    let ((y0, z0), (y1, z1)) = (GUIDE_FOOT, GUIDE_TOP);
    [x, y0 + (y1 - y0) * t, z0 + (z1 - z0) * t]
}

/// How far up the guide's face its scale is, 0..=1.
fn scale_t() -> f32 {
    (SCALE_Z - GUIDE_FOOT.1) / (GUIDE_TOP.1 - GUIDE_FOOT.1)
}

/// The guide's height up its face, in inches.
fn guide_height() -> f32 {
    let ((y0, z0), (y1, z1)) = (GUIDE_FOOT, GUIDE_TOP);
    (y1 - y0).hypot(z1 - z0)
}

/// The guide's perspex plates, their top edges catching the light.
fn paint_plates(painter: &Painter, eye: &Eye) {
    let (inner, outer) = GUIDE_X;
    for side in [-1.0, 1.0] {
        let outline = rounded(&[
            (on_guide(side * inner, 0.0), 0.03),
            (on_guide(side * outer, 0.0), 0.03),
            (on_guide(side * outer, 1.0), OUTER_TOP_ROUNDING),
            (on_guide(side * inner, 1.0), INNER_TOP_ROUNDING),
        ]);
        eye.fill(painter, &outline, |_| GLASS);
        painter.add(Shape::closed_line(
            eye.polygon(&outline),
            Stroke::new(1.0, GLASS_EDGE),
        ));
        // Along the top's straight run, between its rounded corners.
        let top = [
            on_guide(side * (inner + INNER_TOP_ROUNDING), 1.0),
            on_guide(side * (outer - OUTER_TOP_ROUNDING), 1.0),
        ];
        eye.line(painter, &top, 0.015, Color32::from_white_alpha(170));
    }
}

/// The scale along each plate's foot, from a black notch with a white V at
/// its inner end out `SCALE_MARKS` columns, marked on the columns' edges so
/// it lines up with the type.
fn paint_scale(painter: &Painter, eye: &Eye, metrics: &Metrics) {
    let inner = GUIDE_X.0;
    let (ppi, column) = (eye.ppi, metrics.column_width);
    let (notch_in, notch_width) = PENCIL_NOTCH;
    let (top, foot) = (scale_t(), scale_t() - SCALE_DEPTH / guide_height());
    let notch_out = inner + notch_in + notch_width;
    // The first column edge clear of the notch.
    let first = (notch_out * ppi / column + 1.0).ceil() as i32;
    let last = first + SCALE_MARKS - 1;
    let strip_out = (last as f32 + 0.3) * column / ppi;
    for side in [-1.0, 1.0] {
        let strip = [
            on_guide(side * notch_out, top),
            on_guide(side * strip_out, top),
            on_guide(side * strip_out, foot),
            on_guide(side * notch_out, foot),
        ];
        eye.fill(painter, &strip, |_| SCALE);
        let [a, b] = [inner + notch_in, notch_out].map(|x| side * x);
        let notch = rounded(&[
            (on_guide(a, top + 0.02), 0.03),
            (on_guide(b, top + 0.02), 0.03),
            (on_guide(b, foot), 0.0),
            (on_guide(a, foot), 0.0),
        ]);
        eye.fill(painter, &notch, |_| SCALE);
        let middle = side * (inner + notch_in + notch_width / 2.0);
        let v_top = top - 0.02 / guide_height();
        let v_foot = top - 0.09 / guide_height();
        let v = [
            on_guide(middle - 0.035, v_top),
            on_guide(middle + 0.035, v_top),
            on_guide(middle, v_foot),
        ];
        painter.add(Shape::closed_line(
            eye.polygon(&v),
            Stroke::new(1.0, SCALE_MARK),
        ));
        for k in first..=last {
            let x = side * (k as f32 - 0.5) * column / ppi;
            let mark_top = eye.at(on_guide(x, top - 0.015 / guide_height())).y;
            // On screen, so the marks fall on the sheet's columns exactly.
            let x = eye.origin.x + side * (k as f32 - 0.5) * column;
            let mark = [pos2(x, mark_top), pos2(x, mark_top + MARK_LENGTH * ppi)];
            painter.line_segment(mark, Stroke::new(1.0, SCALE_MARK));
        }
    }
}

/// The ribbon's top edge, left spool to right: out of sight under the cover
/// at its ends, straight across through the vibrator.
fn ribbon_path() -> [[f32; 3]; 4] {
    let (y, top, _) = RIBBON;
    let (out_x, out_y, drop) = RIBBON_OUT;
    [
        [-out_x, out_y, top - drop],
        [-RIBBON_ACROSS, y, top],
        [RIBBON_ACROSS, y, top],
        [out_x, out_y, top - drop],
    ]
}

/// The two-colour ribbon on edge, black over red, each run lit as it faces.
fn paint_ribbon(painter: &Painter, eye: &Eye) {
    let width = RIBBON.2;
    let down = |p: [f32; 3], by: f32| [p[0], p[1], p[2] - by];
    for pair in ribbon_path().windows(2) {
        let [a, b] = [pair[0], pair[1]];
        // Toward the writer, square to the run.
        let facing = [a[1] - b[1], b[0] - a[0], 0.0];
        let facing = if facing[1] < 0.0 {
            facing.map(|c| -c)
        } else {
            facing
        };
        for (colour, from, to) in [
            (RIBBON_INK, 0.0, width / 2.0),
            (RIBBON_RED, width / 2.0, width),
        ] {
            let band = [down(a, from), down(b, from), down(b, to), down(a, to)];
            let lit = matte(colour, facing);
            eye.fill(painter, &band, |_| lit);
        }
    }
}

/// The card holder's chrome wire: up from the vibrator's shoulders along
/// the guide's face, arching over the printing point level with its top.
fn paint_card_holder(painter: &Painter, eye: &Eye) {
    let half = CARD_HOLDER_HALF;
    let (y, top, _) = VIBRATOR;
    // Just proud of the guide's face.
    let on_face = |x: f32, t: f32| {
        let [px, py, pz] = on_guide(x, t);
        [px, py + 0.03, pz]
    };
    let bend = 1.0 - half / guide_height();
    let arch = (0..=16u8).map(|i| {
        let angle = PI * f32::from(i) / 16.0;
        on_face(-half * angle.cos(), bend + (1.0 - bend) * angle.sin())
    });
    let wire: Vec<[f32; 3]> = [[-half, y, top], on_face(-half, scale_t())]
        .into_iter()
        .chain(arch)
        .chain([on_face(half, scale_t()), [half, y, top]])
        .collect();
    eye.line(painter, &wire, 0.065, EDGE);
    eye.line(painter, &wire, 0.045, METAL_SHINE);
    let glint: Vec<[f32; 3]> = wire.iter().map(|&[x, y, z]| [x - 0.008, y, z]).collect();
    eye.line(painter, &glint, 0.014, Color32::WHITE);
}

/// The vibrator: a chrome body over the ribbon, the slot in its foot showing
/// the ribbon's red; chrome lugs either side holding the ribbon's edges.
fn paint_vibrator(painter: &Painter, eye: &Eye) {
    let (y, top, bottom) = VIBRATOR;
    let half = VIBRATOR_HALF;
    let height = top - bottom;
    paint_chrome(painter, eye, [-half, half], y, top, height);
    let (slot_half, slot_height) = VIBRATOR_SLOT;
    let slot = rounded(&[
        ([-slot_half, y, bottom], 0.0),
        ([slot_half, y, bottom], 0.0),
        ([slot_half, y, bottom + slot_height], slot_half),
        ([-slot_half, y, bottom + slot_height], slot_half),
    ]);
    eye.fill(painter, &slot, |_| matte(RIBBON_RED, [0.0, 1.0, 0.0]));
    let (inner, outer) = LUGS;
    for side in [-1.0, 1.0] {
        let [a, b] = [side * inner, side * outer];
        let x = [a.min(b), a.max(b)];
        paint_chrome(painter, eye, x, y, top + 0.03, height + 0.1);
    }
}

#[cfg(test)]
mod tests {
    use super::super::cover::{COVER_BACK, on_cover};
    use super::*;

    /// At 96 points an inch, the typing line's foot is 16 points down.
    const LINE_FOOT: f32 = 96.0 / 6.0;

    #[test]
    fn the_guide_stands_over_the_line_and_the_ribbon_below_it() {
        let eye = Eye::testing(0.0, 96.0);
        let foot = eye.at(on_guide(0.0, scale_t())).y;
        assert!((LINE_FOOT..LINE_FOOT + 3.0).contains(&foot), "{foot}");
        assert!(eye.at(on_guide(0.0, 1.0)).y < -20.0);
        // The descenders show above the ribbon and the vibrator.
        let (y, top, _) = RIBBON;
        let (vy, vtop, _) = VIBRATOR;
        assert!(eye.at([0.0, y, top]).y > foot);
        assert!(eye.at([0.0, vy, vtop]).y > LINE_FOOT + 1.0);
    }

    #[test]
    fn the_cover_hides_the_ribbon_but_not_the_scale() {
        let eye = Eye::testing(0.0, 96.0);
        let (back_y, back_z) = COVER_BACK;
        let edge = eye.at([2.0, back_y, back_z]).y;
        let (y, top, _) = RIBBON;
        assert!(edge < eye.at([2.0, y, top]).y);
        let scale = eye.at(on_guide(2.0, scale_t())).y;
        assert!(edge > scale + MARK_LENGTH * 96.0, "{edge} {scale}");
    }

    #[test]
    fn the_ribbon_runs_under_the_cover_to_its_spools() {
        let [end, ..] = ribbon_path();
        assert!(end[2] < on_cover(end[0], end[1])[2]);
    }
}
