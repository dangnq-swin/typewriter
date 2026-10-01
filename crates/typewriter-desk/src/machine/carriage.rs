//! The carriage, travelling with the sheet: the platen, the carriage's back
//! and side plates, and the paper bail above the printing point, its scale
//! printed on it by the app.

use std::f32::consts::TAU;

use eframe::egui::Color32;

use super::canvas::Canvas;
use super::eye::Eye;
use super::geometry::{add, normalized, scaled};
use super::light::{matte, polished};
use super::sheet::face_y;
use super::{CHROME, METAL, METAL_SHINE};
use crate::depth::{Layer, Solid};
use typewriter_app::draw::{Metrics, ruler};

/// Centre to the carriage's ends, where the knobs are.
const PLATEN_HALF_INCHES: f32 = 5.9;
pub(super) const PLATEN_DIAMETER_INCHES: f32 = 1.3;
/// Up the platen's front from level with its axis, where the type strikes:
/// its lower part goes behind the cover and the ribbon.
pub(super) const STRIKE_DEGREES: f32 = 35.0;
/// The metal rings at the platen's ends.
const PLATEN_END_INCHES: f32 = 0.08;
/// Bands of shading round a roller: enough for a smooth curve.
const ROLLER_BANDS: u16 = 28;
/// Round the platen, in depth: fine enough that its flats stay under the
/// paper wound on it.
const PLATEN_BANDS: u16 = 64;
/// The platen drawn this far under the paper on it.
const UNDER_PAPER_INCHES: f32 = 0.01;
/// How tight the highlight on a roller or the platen is.
const ROLLER_SHARPNESS: i32 = 4;
/// The carriage's side plates, inside its ends: their thickness, and from
/// the platen's axis their back and front `y`, their top and foot `z`. Their
/// front behind the alignment guide, which the carriage runs past.
const SIDE_PLATE_INCHES: f32 = 0.2;
pub(super) const SIDE_PLATE_Y: (f32, f32) = (-1.05, 0.4);
const SIDE_PLATE_Z: (f32, f32) = (0.8, -0.75);
/// The rod across the carriage's back, from the platen's axis `(y, z)`, and
/// its radius.
const CARRIAGE_BACK: (f32, f32) = (-0.75, 0.45);
const CARRIAGE_BACK_RADIUS: f32 = 0.22;
/// The paper bail above the typing line: its scale's centre, the bar's
/// height past the scale's and its depth, its rollers either side of the
/// sheet's centre, their length and how much larger round than the scale is
/// high; the arms holding it to the side plates, pivoting at `(y, z)` clear
/// of the platen, behind the alignment guide.
const BAIL_ABOVE_INCHES: f32 = 0.6;
const BAIL_EXTRA_INCHES: f32 = 0.08;
const BAIL_DEPTH: f32 = 0.06;
const BAIL_ROLLER_INCHES: f32 = 1.65;
const BAIL_ROLLER_HALF: f32 = 0.25;
const BAIL_ROLLER_EXTRA: f32 = 0.07;
const BAIL_ARM_INCHES: f32 = 0.14;
const BAIL_PIVOT: (f32, f32) = (-0.15, 0.3);
/// Searching for where the bail shows: how high at most, and how finely.
const BAIL_HIGHEST: f32 = 3.0;
const SEARCH_STEPS: u16 = 30;
const RUBBER: Color32 = Color32::from_rgb(0x16, 0x15, 0x14);
const RUBBER_SHINE: Color32 = Color32::from_rgb(0x55, 0x53, 0x50);
const BAIL_TOP: Color32 = Color32::from_rgb(0xD4, 0xD6, 0xD2);
const BAIL_MID: Color32 = Color32::from_rgb(0xEE, 0xEF, 0xEC);
const BAIL_LOW: Color32 = Color32::from_rgb(0xAE, 0xB0, 0xAC);

/// The carriage's ends, inches across, for the sheet centred `middle`
/// inches across.
pub(super) fn ends(middle: f32) -> [f32; 2] {
    [middle - PLATEN_HALF_INCHES, middle + PLATEN_HALF_INCHES]
}

/// The platen's axis from the printing point.
pub(super) fn platen_axis() -> [f32; 3] {
    let (sin, cos) = STRIKE_DEGREES.to_radians().sin_cos();
    let radius = PLATEN_DIAMETER_INCHES / 2.0;
    [0.0, -radius * cos, -radius * sin]
}

/// The carriage for the sheet centred `middle` inches across: the rod at
/// its back, the platen and its metal ends, and the side plates.
pub(super) fn paint(canvas: &Canvas, eye: &Eye, middle: f32) {
    let [left, right] = ends(middle);
    let [_, axis_y, axis_z] = platen_axis();
    let radius = PLATEN_DIAMETER_INCHES / 2.0;
    let mut solid = Solid::default();
    let back = (axis_y + CARRIAGE_BACK.0, axis_z + CARRIAGE_BACK.1);
    let rod = ([left, right], back, CARRIAGE_BACK_RADIUS);
    cylinder(eye, &mut solid, rod, 2 * ROLLER_BANDS, [METAL, CHROME]);
    let (inner_left, inner_right) = (left + SIDE_PLATE_INCHES, right - SIDE_PLATE_INCHES);
    let end = PLATEN_END_INCHES;
    let rubber = [inner_left + end, inner_right - end];
    let axis = (axis_y, axis_z);
    let under = radius - UNDER_PAPER_INCHES;
    let bands = PLATEN_BANDS;
    cylinder(
        eye,
        &mut solid,
        (rubber, axis, under),
        bands,
        [RUBBER, RUBBER_SHINE],
    );
    for x in [
        [inner_left, inner_left + end],
        [inner_right - end, inner_right],
    ] {
        cylinder(
            eye,
            &mut solid,
            (x, axis, radius),
            bands,
            [METAL, METAL_SHINE],
        );
    }
    canvas.mesh(Layer::Opaque, solid);
    for x in [[left, inner_left], [inner_right, right]] {
        paint_side_plate(canvas, eye, x);
    }
}

/// A cylinder across `x` round `(y, z)` of `radius`, in `bands` round it,
/// polished: `[shade, shine]`.
pub(super) fn cylinder(
    eye: &Eye,
    solid: &mut Solid,
    ([left, right], (y, z), radius): ([f32; 2], (f32, f32), f32),
    bands: u16,
    [shade, shine]: [Color32; 2],
) {
    let at = |i: u16, x: f32| {
        // From the top round the front, the bottom and the back.
        let around = TAU * f32::from(i) / f32::from(bands);
        let (sin, cos) = around.sin_cos();
        let p = [x, y + radius * sin, z + radius * cos];
        (p, polished(shade, shine, [0.0, sin, cos], ROLLER_SHARPNESS))
    };
    for i in 0..bands {
        eye.quad(
            solid,
            [at(i, left), at(i, right), at(i + 1, right), at(i + 1, left)],
        );
    }
}

/// A side plate across `x`: chrome, each face lit as it turns.
fn paint_side_plate(canvas: &Canvas, eye: &Eye, [x0, x1]: [f32; 2]) {
    let [_, axis_y, axis_z] = platen_axis();
    let (back, front) = (axis_y + SIDE_PLATE_Y.0, axis_y + SIDE_PLATE_Y.1);
    let (top, foot) = (axis_z + SIDE_PLATE_Z.0, axis_z + SIDE_PLATE_Z.1);
    let faces = [
        (
            [0.0, 1.0, 0.0],
            [
                [x0, front, top],
                [x1, front, top],
                [x1, front, foot],
                [x0, front, foot],
            ],
        ),
        (
            [0.0, 0.0, 1.0],
            [
                [x0, back, top],
                [x1, back, top],
                [x1, front, top],
                [x0, front, top],
            ],
        ),
        (
            [-1.0, 0.0, 0.0],
            [
                [x0, back, top],
                [x0, front, top],
                [x0, front, foot],
                [x0, back, foot],
            ],
        ),
        (
            [1.0, 0.0, 0.0],
            [
                [x1, back, top],
                [x1, front, top],
                [x1, front, foot],
                [x1, back, foot],
            ],
        ),
    ];
    for (normal, face) in faces {
        if !eye.faces(face[0], normal) {
            continue;
        }
        let lit = matte(CHROME, normal);
        eye.fill(canvas, &face, |_| lit);
        eye.outline(canvas, &face);
    }
}

/// The top of the bail's scale for the typing line at `typing_y`.
pub fn bail_scale_top(metrics: &Metrics, typing_y: f32) -> f32 {
    typing_y - BAIL_ABOVE_INCHES * metrics.points_per_inch - ruler::HEIGHT / 2.0
}

/// The `z` whose point at depth `y(z)` shows at screen height `target`:
/// higher on screen further up.
fn z_showing_at(eye: &Eye, y: impl Fn(f32) -> f32, target: f32) -> f32 {
    let (mut low, mut high) = (0.0, BAIL_HIGHEST);
    for _ in 0..SEARCH_STEPS {
        let mid = (low + high) / 2.0;
        if eye.at([0.0, y(mid), mid]).y > target {
            low = mid;
        } else {
            high = mid;
        }
    }
    high
}

/// The paper bail across the carriage for the sheet centred `middle` inches
/// across: its rollers pressing the paper, the bar's front where the app
/// prints its scale, and the arms holding it to the side plates.
pub(super) fn paint_bail(
    canvas: &Canvas,
    eye: &Eye,
    metrics: &Metrics,
    typing_y: f32,
    middle: f32,
) {
    let ppi = metrics.points_per_inch;
    let scale_top = bail_scale_top(metrics, typing_y);
    let extra = BAIL_EXTRA_INCHES * ppi;
    let roller = ruler::HEIGHT / 2.0 / ppi + BAIL_ROLLER_EXTRA;
    // The rollers on the paper, the bar's front a little before their axis.
    let front_at = |z: f32| face_y(z) + roller + BAIL_DEPTH / 2.0;
    let middle_z = z_showing_at(eye, front_at, scale_top + ruler::HEIGHT / 2.0);
    let front = front_at(middle_z);
    let axis = (front - BAIL_DEPTH / 2.0, middle_z);
    let [top, foot] = [scale_top - extra, scale_top + ruler::HEIGHT + extra]
        .map(|target| z_showing_at(eye, |_| front, target));
    let [left, right] = ends(middle);
    let (inner_left, inner_right) = (left + SIDE_PLATE_INCHES, right - SIDE_PLATE_INCHES);
    let arm = BAIL_ARM_INCHES;
    paint_bail_bar(
        canvas,
        eye,
        [inner_left + arm, inner_right - arm],
        front,
        [top, foot],
    );
    let mut solid = Solid::default();
    for side in [-1.0, 1.0] {
        let x = middle + side * BAIL_ROLLER_INCHES;
        let across = [x - BAIL_ROLLER_HALF, x + BAIL_ROLLER_HALF];
        cylinder(
            eye,
            &mut solid,
            (across, axis, roller),
            ROLLER_BANDS,
            [RUBBER, RUBBER_SHINE],
        );
    }
    canvas.mesh(Layer::Opaque, solid);
    for x in [
        [inner_left, inner_left + arm],
        [inner_right - arm, inner_right],
    ] {
        paint_bail_arm(canvas, eye, x, (axis.0, top), BAIL_PIVOT);
    }
}

/// The bail's bar over `x`, its front at depth `front` from `top` to `foot`:
/// brushed metal, brightest just above the middle.
fn paint_bail_bar(
    canvas: &Canvas,
    eye: &Eye,
    [x0, x1]: [f32; 2],
    front: f32,
    [top, foot]: [f32; 2],
) {
    let back = front - BAIL_DEPTH;
    let stops = [
        (top, BAIL_TOP),
        (top + 0.35 * (foot - top), BAIL_MID),
        (foot, BAIL_LOW),
    ];
    let mut solid = Solid::default();
    for pair in stops.windows(2) {
        let [(upper, upper_colour), (lower, lower_colour)] = [pair[0], pair[1]];
        eye.quad(
            &mut solid,
            [
                ([x0, front, upper], upper_colour),
                ([x1, front, upper], upper_colour),
                ([x1, front, lower], lower_colour),
                ([x0, front, lower], lower_colour),
            ],
        );
    }
    let lid = matte(BAIL_TOP, [0.0, 0.0, 1.0]);
    eye.quad(
        &mut solid,
        [
            ([x0, back, top], lid),
            ([x1, back, top], lid),
            ([x1, front, top], lid),
            ([x0, front, top], lid),
        ],
    );
    canvas.mesh(Layer::Opaque, solid);
    let outline = [
        [x0, front, top],
        [x1, front, top],
        [x1, front, foot],
        [x0, front, foot],
    ];
    eye.outline(canvas, &outline);
}

/// A bail arm over `x`, from the bar's end at `from` down to its rivet on
/// the side plate at `pivot`, each `(y, z)`: chrome, its front and inner
/// side lit as they face.
fn paint_bail_arm(
    canvas: &Canvas,
    eye: &Eye,
    [x0, x1]: [f32; 2],
    from: (f32, f32),
    pivot: (f32, f32),
) {
    let down = normalized([0.0, pivot.0 - from.0, pivot.1 - from.1]);
    // Square to the arm, toward the writer.
    let forward = [0.0, -down[2], down[1]];
    let forward = if forward[1] < 0.0 {
        scaled(forward, -1.0)
    } else {
        forward
    };
    let half = scaled(forward, BAIL_DEPTH / 2.0);
    let [start, end] = [from, pivot].map(|(y, z)| [0.0, y, z]);
    // Past the rivet by half the arm's width: its rounded foot.
    let end = add(end, scaled(down, BAIL_ARM_INCHES / 2.0));
    let at = |x: f32, p: [f32; 3], side: [f32; 3]| add([x, 0.0, 0.0], add(p, side));
    let front_face = [
        at(x0, start, half),
        at(x1, start, half),
        at(x1, end, half),
        at(x0, end, half),
    ];
    let back = scaled(half, -1.0);
    // The side toward the carriage's middle.
    let inner = if x0 + x1 < 0.0 { x1 } else { x0 };
    let inner_face = [
        at(inner, start, half),
        at(inner, end, half),
        at(inner, end, back),
        at(inner, start, back),
    ];
    let toward_middle = [if inner == x1 { 1.0 } else { -1.0 }, 0.0, 0.0];
    for (face, normal) in [(front_face, forward), (inner_face, toward_middle)] {
        let lit = matte(CHROME, normal);
        eye.fill(canvas, &face, |_| lit);
        eye.outline(canvas, &face);
    }
    let rivet_at = at((x0 + x1) / 2.0, [0.0, pivot.0, pivot.1], half);
    let radius = BAIL_ARM_INCHES * 0.28;
    let (side_way, up_way) = ([1.0, 0.0, 0.0], [0.0, down[1], down[2]].map(|c| -c));
    let rivet: Vec<[f32; 3]> = (0..16u8)
        .map(|i| {
            let (sin, cos) = (TAU * f32::from(i) / 16.0).sin_cos();
            add(
                rivet_at,
                add(scaled(side_way, radius * cos), scaled(up_way, radius * sin)),
            )
        })
        .collect();
    eye.fill_lying(canvas, &rivet, |_| matte(METAL, forward));
    eye.outline(canvas, &rivet);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_bail_s_arms_pivot_on_the_side_plates_clear_of_the_platen() {
        let [_, axis_y, axis_z] = platen_axis();
        let (y, z) = BAIL_PIVOT;
        assert!(y < axis_y + SIDE_PLATE_Y.1 && z < axis_z + SIDE_PLATE_Z.0);
        // The arm's rounded foot past the rivet, and its back.
        let reach = BAIL_ARM_INCHES / 2.0 + BAIL_DEPTH / 2.0;
        let off = ((y - axis_y).powi(2) + (z - axis_z).powi(2)).sqrt();
        assert!(off - reach > PLATEN_DIAMETER_INCHES / 2.0, "{off}");
    }
}
