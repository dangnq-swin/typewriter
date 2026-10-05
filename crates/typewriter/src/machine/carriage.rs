//! The carriage, travelling with the sheet: the platen, the carriage's back
//! and side plates. Its paper bail is `bail.rs`.

use std::f32::consts::{FRAC_PI_2, PI, TAU};

use eframe::egui::Color32;
use glam::{Vec2, Vec3};

use super::bail;
use super::canvas::Canvas;
use super::eye::Eye;
use super::light::{matte, polished};
use super::{CHROME, METAL, METAL_SHINE};
use crate::depth::{Layer, Solid};

/// Centre to the carriage's ends, where the knobs are.
const PLATEN_HALF_MM: f32 = 150.0;
pub(super) const PLATEN_DIAMETER_MM: f32 = 33.0;
/// Up the platen's front from level with its axis, where the type strikes:
/// its lower part goes behind the cover and the ribbon.
pub(super) const STRIKE_DEGREES: f32 = 35.0;
/// The metal rings at the platen's ends.
const PLATEN_END_MM: f32 = 2.0;
/// Bands of shading round a roller: enough for a smooth curve.
pub(super) const ROLLER_BANDS: u16 = 28;
/// Round the platen, in depth: its flats chord under the paper wound on it.
const PLATEN_BANDS: u16 = 64;
/// The wound sheet's thickness: its print face is what the machine measures
/// by, so the platen's rubber stands this far under it.
const PAPER_THICKNESS_MM: f32 = 0.1;
/// How tight the highlight on a roller or the platen is.
const ROLLER_SHARPNESS: f32 = 4.0;
/// The carriage's side plates, inside its ends: their thickness, and from
/// the platen's axis their back and front `y` and their foot's `z`. Their
/// front, straight, just ahead of the platen, hiding its ends; their top
/// level with the paper bail's discs, a pocket beside each.
pub(super) const SIDE_PLATE_MM: f32 = 5.0;
pub(super) const SIDE_PLATE_BACK: f32 = -27.0;
const SIDE_PLATE_FRONT: f32 = 17.0;
const _: () = assert!(SIDE_PLATE_FRONT > PLATEN_DIAMETER_MM / 2.0);
const SIDE_PLATE_FOOT: f32 = -19.0;
/// The pocket: the plate's skin left outside it, and its lower corners'
/// radius.
const POCKET_SKIN: f32 = 1.0;
const POCKET_ROUNDING: f32 = 2.0;
const _: () = assert!(POCKET_SKIN < SIDE_PLATE_MM);
/// Round each of its corners.
const POCKET_STEPS: u16 = 6;
/// The rod across the carriage's back, from the platen's axis `(y, z)`, and
/// its radius.
const CARRIAGE_BACK: (f32, f32) = (-19.0, 11.0);
const CARRIAGE_BACK_RADIUS: f32 = 6.0;
pub(super) const RUBBER: Color32 = Color32::from_rgb(0x16, 0x15, 0x14);
pub(super) const RUBBER_SHINE: Color32 = Color32::from_rgb(0x55, 0x53, 0x50);

/// The carriage's ends, millimetres across, for the sheet centred `middle`
/// millimetres across.
pub(super) fn ends(middle: f32) -> [f32; 2] {
    [middle - PLATEN_HALF_MM, middle + PLATEN_HALF_MM]
}

/// The platen's axis from the printing point.
pub(super) fn platen_axis() -> Vec3 {
    let Vec2 { x: cos, y: sin } = Vec2::from_angle(STRIKE_DEGREES.to_radians());
    let radius = PLATEN_DIAMETER_MM / 2.0;
    Vec3::new(0.0, -radius * cos, -radius * sin)
}

/// The carriage for the sheet centred `middle` millimetres across: the rod at
/// its back, the platen and its metal ends, and the side plates.
pub(super) fn paint(canvas: &Canvas, eye: &Eye, middle: f32) {
    let [left, right] = ends(middle);
    let axis = platen_axis();
    let (axis_y, axis_z) = (axis.y, axis.z);
    let radius = PLATEN_DIAMETER_MM / 2.0;
    let mut solid = Solid::default();
    let back = Vec2::new(axis_y + CARRIAGE_BACK.0, axis_z + CARRIAGE_BACK.1);
    let rod = ([left, right], back, CARRIAGE_BACK_RADIUS);
    cylinder(eye, &mut solid, rod, 2 * ROLLER_BANDS, [METAL, CHROME]);
    let (inner_left, inner_right) = (left + SIDE_PLATE_MM, right - SIDE_PLATE_MM);
    let end = PLATEN_END_MM;
    let rubber = [inner_left + end, inner_right - end];
    let axis = Vec2::new(axis_y, axis_z);
    let under = radius - PAPER_THICKNESS_MM;
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
    paint_side_plate(canvas, eye, left, 1.0);
    paint_side_plate(canvas, eye, right, -1.0);
}

/// A cylinder across `x` round `(y, z)` of `radius`, in `bands` round it,
/// polished: `[shade, shine]`.
pub(super) fn cylinder(
    eye: &Eye,
    solid: &mut Solid,
    ([left, right], axis, radius): ([f32; 2], Vec2, f32),
    bands: u16,
    [shade, shine]: [Color32; 2],
) {
    let at = |i: u16, x: f32| {
        // From the top round the front, the bottom and the back.
        let around = TAU * f32::from(i) / f32::from(bands);
        let Vec2 { x: cos, y: sin } = Vec2::from_angle(around);
        let p = Vec3::new(x, axis.x + radius * sin, axis.y + radius * cos);
        (
            p,
            polished(shade, shine, Vec3::new(0.0, sin, cos), ROLLER_SHARPNESS),
        )
    };
    for i in 0..bands {
        eye.quad(
            solid,
            [at(i, left), at(i, right), at(i + 1, right), at(i + 1, left)],
        );
    }
}

/// The side plates' fronts' `y`.
pub(super) fn plate_front() -> f32 {
    platen_axis().y + SIDE_PLATE_FRONT
}

/// The side plate with its outer face at `outer`, its inner face `inward`
/// (1 or -1) toward the middle: chrome, each face lit as it turns. A pocket
/// in its top beside the bail's disc, open at the top, rounded below, its
/// outer skin left whole.
fn paint_side_plate(canvas: &Canvas, eye: &Eye, outer: f32, inward: f32) {
    let axis = platen_axis();
    let (axis_y, axis_z) = (axis.y, axis.z);
    let (back, front) = (axis_y + SIDE_PLATE_BACK, plate_front());
    let foot = axis_z + SIDE_PLATE_FOOT;
    let bail::Pocket {
        back: pocket_back,
        front: pocket_front,
        floor,
        top,
    } = bail::pocket();
    let (x_o, x_i, x_n) = (
        outer,
        outer + inward * SIDE_PLATE_MM,
        outer + inward * POCKET_SKIN,
    );
    let r = POCKET_ROUNDING;
    // The pocket's rounded corners, `(y, z)`: its back's, from its back down
    // to its floor; its front's, from its floor up to its front.
    let corner = |centre: Vec2, from: f32| -> Vec<Vec2> {
        (0..=POCKET_STEPS)
            .map(|i| {
                let angle = from + FRAC_PI_2 * f32::from(i) / f32::from(POCKET_STEPS);
                centre + r * Vec2::from_angle(angle)
            })
            .collect()
    };
    let (back_centre, front_centre) = (
        Vec2::new(pocket_back + r, floor + r),
        Vec2::new(pocket_front - r, floor + r),
    );
    let back_corner = corner(back_centre, PI);
    let front_corner = corner(front_centre, 3.0 * FRAC_PI_2);
    let at_x = |x: f32| move |at: Vec2| Vec3::new(x, at.x, at.y);
    let face = |points: &[Vec3], normal: Vec3| {
        // Culling, not ordering: a face the eye cannot see may still stand
        // in the lamp's way. Same for the outlines below.
        if eye.faces_lit(points[0], normal) {
            let lit = matte(CHROME, normal);
            eye.fill(canvas, points, |_| lit);
        }
    };
    let outline = |points: &[Vec3], normal: Vec3| {
        if eye.faces(points[0], normal) {
            eye.outline(canvas, points);
        }
    };
    let in_plane = |x: f32, [y0, y1]: [f32; 2], [z0, z1]: [f32; 2]| {
        [
            Vec2::new(y0, z1),
            Vec2::new(y1, z1),
            Vec2::new(y1, z0),
            Vec2::new(y0, z0),
        ]
        .map(at_x(x))
    };
    let (out, inn) = (-Vec3::X * inward, Vec3::X * inward);
    let whole = in_plane(x_o, [back, front], [foot, top]);
    face(&whole, out);
    outline(&whole, out);
    // The inner face, round the pocket: behind it, before it, under it, and
    // filling its corners.
    face(&in_plane(x_i, [back, pocket_back], [foot, top]), inn);
    face(&in_plane(x_i, [pocket_front, front], [foot, top]), inn);
    face(
        &in_plane(x_i, [pocket_back, pocket_front], [foot, floor]),
        inn,
    );
    for (arc, solid) in [
        (&back_corner, Vec2::new(pocket_back, floor)),
        (&front_corner, Vec2::new(pocket_front, floor)),
    ] {
        for pair in arc.windows(2) {
            face(&[solid, pair[0], pair[1]].map(at_x(x_i)), inn);
        }
    }
    let rim: Vec<Vec2> = [Vec2::new(pocket_back, top)]
        .into_iter()
        .chain(back_corner.iter().copied())
        .chain(front_corner.iter().copied())
        .chain([Vec2::new(pocket_front, top)])
        .collect();
    let inner_outline: Vec<Vec3> = [
        Vec2::new(back, top),
        Vec2::new(back, foot),
        Vec2::new(front, foot),
        Vec2::new(front, top),
    ]
    .into_iter()
    .chain(rim.iter().rev().copied())
    .map(at_x(x_i))
    .collect();
    outline(&inner_outline, inn);
    // The pocket: its skin, its back, floor and front, and its corners.
    let skin: Vec<Vec3> = rim.iter().copied().map(at_x(x_n)).collect();
    face(&skin, inn);
    outline(&skin, inn);
    let across = |from: Vec2, to: Vec2| {
        [
            Vec3::new(x_n, from.x, from.y),
            Vec3::new(x_i, from.x, from.y),
            Vec3::new(x_i, to.x, to.y),
            Vec3::new(x_n, to.x, to.y),
        ]
    };
    let walls = [
        (
            Vec2::new(pocket_back, top),
            Vec2::new(pocket_back, floor + r),
            Vec3::Y,
        ),
        (
            Vec2::new(pocket_back + r, floor),
            Vec2::new(pocket_front - r, floor),
            Vec3::Z,
        ),
        (
            Vec2::new(pocket_front, floor + r),
            Vec2::new(pocket_front, top),
            -Vec3::Y,
        ),
    ];
    for (from, to, normal) in walls {
        face(&across(from, to), normal);
    }
    for (arc, centre) in [(&back_corner, back_centre), (&front_corner, front_centre)] {
        for pair in arc.windows(2) {
            let mid = (pair[0] + pair[1]) / 2.0;
            face(
                &across(pair[0], pair[1]),
                Vec3::new(0.0, centre.x - mid.x, centre.y - mid.y),
            );
        }
    }
    // The top round the pocket, and the front.
    let up = Vec3::Z;
    let on_top = |[x0, x1]: [f32; 2], [y0, y1]: [f32; 2]| {
        [
            Vec3::new(x0, y0, top),
            Vec3::new(x1, y0, top),
            Vec3::new(x1, y1, top),
            Vec3::new(x0, y1, top),
        ]
    };
    face(&on_top([x_o, x_i], [back, pocket_back]), up);
    face(&on_top([x_o, x_n], [pocket_back, pocket_front]), up);
    face(&on_top([x_o, x_i], [pocket_front, front]), up);
    let top_outline = [
        Vec3::new(x_o, back, top),
        Vec3::new(x_i, back, top),
        Vec3::new(x_i, pocket_back, top),
        Vec3::new(x_n, pocket_back, top),
        Vec3::new(x_n, pocket_front, top),
        Vec3::new(x_i, pocket_front, top),
        Vec3::new(x_i, front, top),
        Vec3::new(x_o, front, top),
    ];
    outline(&top_outline, up);
    let forward = Vec3::Y;
    let before = [
        Vec3::new(x_o, front, top),
        Vec3::new(x_i, front, top),
        Vec3::new(x_i, front, foot),
        Vec3::new(x_o, front, foot),
    ];
    face(&before, forward);
    outline(&before, forward);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_side_plates_hold_the_pocket() {
        let axis = platen_axis();
        let (axis_y, axis_z) = (axis.y, axis.z);
        let bail::Pocket {
            back,
            front,
            floor,
            top,
        } = bail::pocket();
        assert!(front - back > 2.0 * POCKET_ROUNDING && top - floor > POCKET_ROUNDING);
        assert!(back > axis_y + SIDE_PLATE_BACK && front < plate_front());
        assert!(floor > axis_z + SIDE_PLATE_FOOT);
    }
}
