//! The carriage, travelling with the sheet: the platen, the carriage's back
//! and side plates, in depth; and the paper bail, drawn flat over the sheet
//! at its scale, above the printing point on the platen's front.

use std::f32::consts::{PI, TAU};
use std::ops::RangeInclusive;

use eframe::egui::{Color32, CornerRadius, Mesh, Painter, Rect, Shape, Stroke, StrokeKind, pos2};

use super::canvas::Canvas;
use super::eye::Eye;
use super::geometry::add_quad;
use super::light::matte;
use super::{CHROME, EDGE, METAL, METAL_SHINE};
use typewriter_app::draw::depth::{Layer, Solid};
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
const ROLLER_BANDS: u16 = 14;
/// Round the platen, in depth: fine enough that its flats stay under the
/// paper wound on it.
const PLATEN_BANDS: u16 = 64;
/// The platen drawn this far under the paper on it.
const UNDER_PAPER_INCHES: f32 = 0.01;
/// Light from above and in front: angle round a roller from its top.
const LIGHT_DEGREES: f32 = 55.0;
/// The carriage's side plates, inside its ends: their thickness, and from
/// the platen's axis their back and front `y`, their top and foot `z`.
const SIDE_PLATE_INCHES: f32 = 0.2;
const SIDE_PLATE_Y: (f32, f32) = (-1.05, 0.45);
const SIDE_PLATE_Z: (f32, f32) = (0.8, -0.75);
/// The rod across the carriage's back, from the platen's axis `(y, z)`, and
/// its radius.
const CARRIAGE_BACK: (f32, f32) = (-0.75, 0.45);
const CARRIAGE_BACK_RADIUS: f32 = 0.22;
/// The paper bail above the typing line: its scale's centre, the bar's
/// height past the scale's, its rollers either side of the sheet's centre,
/// and the arms holding it to the side plates, pivoting just above the line.
const BAIL_ABOVE_INCHES: f32 = 0.6;
const BAIL_EXTRA_INCHES: f32 = 0.08;
const BAIL_ROLLER_INCHES: f32 = 1.65;
const BAIL_ARM_INCHES: f32 = 0.14;
const BAIL_PIVOT_ABOVE: f32 = 0.1;
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

/// The carriage's ends on screen, on the platen's axis where the knobs
/// turn, for the sheet centred at `carriage_x`.
pub fn platen_ends(view: Rect, metrics: &Metrics, typing_y: f32, carriage_x: f32) -> [f32; 2] {
    let eye = Eye::new(view, metrics, typing_y);
    let [_, y, z] = platen_axis();
    let middle = (carriage_x - eye.origin.x) / eye.ppi;
    ends(middle).map(|x| eye.at([x, y, z]).x)
}

/// The platen's axis from the printing point.
pub(super) fn platen_axis() -> [f32; 3] {
    let (sin, cos) = STRIKE_DEGREES.to_radians().sin_cos();
    let radius = PLATEN_DIAMETER_INCHES / 2.0;
    [0.0, -radius * cos, -radius * sin]
}

/// The platen's axis on screen, for the typing line at `typing_y`.
pub fn platen_axis_y(view: Rect, metrics: &Metrics, typing_y: f32) -> f32 {
    Eye::new(view, metrics, typing_y).at(platen_axis()).y
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
/// lit from above and in front: `[shade, shine]`.
fn cylinder(
    eye: &Eye,
    solid: &mut Solid,
    ([left, right], (y, z), radius): ([f32; 2], (f32, f32), f32),
    bands: u16,
    colours: [Color32; 2],
) {
    let light = LIGHT_DEGREES.to_radians();
    let at = |i: u16, x: f32| {
        // From the top round the front, the bottom and the back.
        let around = TAU * f32::from(i) / f32::from(bands);
        let lit = (around - light).cos().max(0.0).powi(3);
        let p = [x, y + radius * around.sin(), z + radius * around.cos()];
        (p, colours[0].lerp_to_gamma(colours[1], lit))
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

/// A horizontal cylinder from the front, lit from above: `[shade, shine]`.
pub(super) fn roller(
    mesh: &mut Mesh,
    x: RangeInclusive<f32>,
    centre_y: f32,
    radius: f32,
    colours: [Color32; 2],
) {
    let light = LIGHT_DEGREES.to_radians();
    let row = |i: u16| {
        // From the top (0) round the front to the bottom (π).
        let around = PI * f32::from(i) / f32::from(ROLLER_BANDS);
        let lit = (around - light).cos().max(0.0).powi(3);
        let colour = colours[0].lerp_to_gamma(colours[1], lit);
        (centre_y - radius * around.cos(), colour)
    };
    for i in 0..ROLLER_BANDS {
        let ((top, top_colour), (bottom, bottom_colour)) = (row(i), row(i + 1));
        add_quad(
            mesh,
            [
                (pos2(*x.start(), top), top_colour),
                (pos2(*x.end(), top), top_colour),
                (pos2(*x.end(), bottom), bottom_colour),
                (pos2(*x.start(), bottom), bottom_colour),
            ],
        );
    }
}

/// The top of the bail's scale for the typing line at `typing_y`.
pub fn bail_scale_top(metrics: &Metrics, typing_y: f32) -> f32 {
    typing_y - BAIL_ABOVE_INCHES * metrics.points_per_inch - ruler::HEIGHT / 2.0
}

/// The paper bail across the carriage for the sheet centred at
/// `carriage_x`, its scale printed on it later: the arms holding it to the
/// side plates, the bar, and the rubber rollers pressing the paper.
pub fn paint_bail(
    painter: &Painter,
    view: Rect,
    metrics: &Metrics,
    carriage_x: f32,
    typing_y: f32,
) {
    let ppi = metrics.points_per_inch;
    let [left, right] = platen_ends(view, metrics, typing_y, carriage_x);
    let (left, right) = (
        left + SIDE_PLATE_INCHES * ppi,
        right - SIDE_PLATE_INCHES * ppi,
    );
    let y = bail_scale_top(metrics, typing_y) + ruler::HEIGHT / 2.0;
    let half_height = ruler::HEIGHT / 2.0 + BAIL_EXTRA_INCHES * ppi;
    let pivot_y = typing_y - BAIL_PIVOT_ABOVE * ppi;
    let arm = BAIL_ARM_INCHES * ppi;
    for x in [left..=left + arm, right - arm..=right] {
        let arm_rect = Rect::from_x_y_ranges(x, y - half_height..=pivot_y + arm / 2.0);
        painter.rect(
            arm_rect,
            CornerRadius::same(3),
            CHROME,
            Stroke::new(1.0, EDGE),
            StrokeKind::Inside,
        );
        let rivet = pos2(arm_rect.center().x, pivot_y);
        painter.circle(rivet, arm * 0.28, METAL, Stroke::new(1.0, EDGE));
    }
    let bar = Rect::from_x_y_ranges(left + arm..=right - arm, y - half_height..=y + half_height);
    // Brushed metal, brightest just above the middle.
    let stops = [
        (bar.top(), BAIL_TOP),
        (bar.top() + 0.35 * bar.height(), BAIL_MID),
        (bar.bottom(), BAIL_LOW),
    ];
    let mut mesh = Mesh::default();
    for pair in stops.windows(2) {
        let [(upper, upper_colour), (lower, lower_colour)] = [pair[0], pair[1]];
        add_quad(
            &mut mesh,
            [
                (pos2(bar.left(), upper), upper_colour),
                (pos2(bar.right(), upper), upper_colour),
                (pos2(bar.right(), lower), lower_colour),
                (pos2(bar.left(), lower), lower_colour),
            ],
        );
    }
    painter.add(Shape::mesh(mesh));
    painter.rect_stroke(
        bar,
        CornerRadius::same(2),
        Stroke::new(1.0, EDGE),
        StrokeKind::Inside,
    );
    let mut mesh = Mesh::default();
    for side in [-1.0, 1.0] {
        let x = carriage_x + side * BAIL_ROLLER_INCHES * ppi;
        let radius = ruler::HEIGHT / 2.0 + 0.07 * ppi;
        let width = 0.25 * ppi;
        roller(
            &mut mesh,
            x - width..=x + width,
            y,
            radius,
            [RUBBER, RUBBER_SHINE],
        );
    }
    painter.add(Shape::mesh(mesh));
}
