//! The case round the keyboard: its well, the thin side walls swooping down
//! into the shelf, the rounded front and the plinth.

use std::f32::consts::FRAC_PI_2;

use eframe::egui::Color32;

use super::body::DESK_Z;
use super::canvas::Canvas;
use super::eye::Eye;
use super::geometry::{fillet, rounded, sub};
use super::light::{brighten, matte};
use super::panel::{PANEL_BOTTOM, PANEL_HALF_BOTTOM};
use super::{EDGE, IVORY, IVORY_SHADE};
use crate::depth::{Layer, Solid};

/// The keyboard's well: the thin side walls either side of it, running the
/// case's full depth; the panel's thickness over it; the key bed.
pub(super) const OPENING_HALF: f32 = 149.0;
pub(super) const PANEL_EDGE_MM: f32 = 11.0;
pub(super) const KEY_BED_Z: f32 = -108.0;
/// The walls' tops: easing from the panel's foot to `WALL_EASE_Z`, then
/// swooping down over `WALL_SWOOP` (`y`) to the case's front corners.
const WALL_EASE_Z: f32 = -65.0;
const WALL_SWOOP: (f32, f32) = (178.0, CASE_FRONT);
/// The shelf between the walls, flush with the space bar and reaching just
/// past it: its back `y`, the case's front `y`, its top `z`. The notch in
/// it that the space bar and the keys beside it sit in: half its width, its
/// front `y`.
const SHELF_BACK: f32 = 215.0;
pub(super) const CASE_FRONT: f32 = 232.0;
pub(super) const SHELF_Z: f32 = -99.0;
const NOTCH_HALF: f32 = 114.0;
const NOTCH_FRONT: f32 = 229.0;
/// Where the shelf rounds off into the well beside the notch; the notch's
/// front corners; the case's front corners.
const SHELF_LIP: f32 = 8.0;
const NOTCH_ROUNDING: f32 = 4.0;
pub(super) const FRAME_ROUNDING: f32 = 13.0;
/// The case's front face down to the plinth.
const PLINTH_TOP: f32 = -107.0;
/// Dark enough to read as the machine's insides, light enough for the keys'
/// shadows to show on.
const KEY_BED: Color32 = Color32::from_rgb(0x2A, 0x29, 0x26);
const KEY_BED_FRONT: Color32 = Color32::from_rgb(0x40, 0x3F, 0x3A);
const PLINTH: Color32 = Color32::from_rgb(0x8C, 0x8A, 0x86);

/// The side walls' top at `y`: easing down from the panel's foot, then
/// swooping down into the shelf, and level with it to the front.
pub(super) fn wall_top(y: f32) -> f32 {
    let (y0, z0) = PANEL_BOTTOM;
    let (start, end) = WALL_SWOOP;
    if y <= start {
        let t = ((y - y0) / (start - y0)).clamp(0.0, 1.0);
        z0 + (WALL_EASE_Z - z0) * t
    } else {
        let t = ((y - start) / (end - start)).clamp(0.0, 1.0);
        WALL_EASE_Z + (SHELF_Z - WALL_EASE_Z) * t * t * (3.0 - 2.0 * t)
    }
}

/// The case's outer top edge on the right, back to front: straight along
/// the wall, then round its front corner by angle, so the curve is as fine
/// at its end as at its start. Its `x` alone: mirror it for the left.
fn case_side() -> Vec<[f32; 3]> {
    let (y0, _) = PANEL_BOTTOM;
    let (front, r, outer) = (CASE_FRONT, FRAME_ROUNDING, PANEL_HALF_BOTTOM);
    let straight = (0..64u8).map(|i| y0 + (front - r - y0) * f32::from(i) / 64.0);
    let straight = straight.map(|y| (outer, y));
    let corner = (0..=24u8).map(|i| {
        let angle = FRAC_PI_2 * f32::from(i) / 24.0;
        (outer - r + r * angle.cos(), front - r + r * angle.sin())
    });
    straight
        .chain(corner)
        .map(|(x, y)| [x, y, wall_top(y)])
        .collect()
}

/// The case's front edge, right to left, round its rounded front corners
/// at the walls' height: never its back edge.
fn case_front_edge() -> Vec<[f32; 3]> {
    let corner: Vec<[f32; 3]> = case_side()
        .into_iter()
        .filter(|p| p[1] >= CASE_FRONT - FRAME_ROUNDING - 2.5e-3)
        .collect();
    let left = corner.iter().rev().map(|&[x, y, z]| [-x, y, z]);
    corner.iter().copied().chain(left).collect()
}

/// The top of a well wall's inner face, back to front, `from` to `to` (`y`):
/// `side` -1 left, +1 right.
fn inner_edge(side: f32, from: f32, to: f32) -> Vec<[f32; 3]> {
    (0..=16u8)
        .map(|i| {
            let y = from + (to - from) * f32::from(i) / 16.0;
            [side * OPENING_HALF, y, wall_top(y)]
        })
        .collect()
}

/// The well's floor, reaching under the shelf: its rounded lip lets the eye
/// in past its corner. The machine's insides under the panel's edge, and the
/// shade at the walls' foot.
pub(super) fn paint_well(canvas: &Canvas, eye: &Eye) {
    let (y0, z0) = PANEL_BOTTOM;
    let half = OPENING_HALF;
    let bed = KEY_BED_Z;
    let floor = [
        [-half, y0, bed],
        [half, y0, bed],
        [half, NOTCH_FRONT, bed],
        [-half, NOTCH_FRONT, bed],
    ];
    eye.fill(canvas, &floor, |[_, y, _]| {
        KEY_BED.lerp_to_gamma(KEY_BED_FRONT, (y - y0) / (NOTCH_FRONT - y0))
    });
    // The machine's insides under the panel's edge.
    let under = z0 - PANEL_EDGE_MM;
    let inside = [
        [-half, y0, under],
        [half, y0, under],
        [half, y0, bed],
        [-half, y0, bed],
    ];
    eye.fill(canvas, &inside, |_| KEY_BED);
    paint_well_shade(canvas, eye);
}

/// Shade on the key bed at the foot of the walls and under the panel.
fn paint_well_shade(canvas: &Canvas, eye: &Eye) {
    let (y0, _) = PANEL_BOTTOM;
    let (half, reach, bed) = (OPENING_HALF, 13.0, KEY_BED_Z);
    let (dark, clear) = (Color32::from_black_alpha(170), Color32::TRANSPARENT);
    let mut solid = Solid::default();
    for (a, b, inward) in [
        ((-half, y0), (half, y0), (0.0, reach)),
        ((-half, y0), (-half, SHELF_BACK), (reach, 0.0)),
        ((half, y0), (half, SHELF_BACK), (-reach, 0.0)),
    ] {
        let at = |(x, y): (f32, f32)| [x, y, bed];
        let inside = |(x, y): (f32, f32)| at((x + inward.0, y + inward.1));
        eye.lying_quad(
            &mut solid,
            [
                (at(a), dark),
                (at(b), dark),
                (inside(b), clear),
                (inside(a), clear),
            ],
        );
    }
    canvas.mesh(Layer::Decal, solid);
}

/// The walls' inner faces, down to the bed along the well and to the shelf
/// beside it; the notch's sides down to the bed.
pub(super) fn paint_inner_walls(canvas: &Canvas, eye: &Eye) {
    let (y0, _) = PANEL_BOTTOM;
    let half = OPENING_HALF;
    let bed = KEY_BED_Z;
    let mut solid = Solid::default();
    for side in [-1.0, 1.0] {
        // The notch's side follows the shelf's edge round its rounded lip.
        let at = |x: f32, y: f32| [x, y, SHELF_Z];
        let notch_side: Vec<[f32; 3]> = std::iter::once(at(side * half, SHELF_BACK))
            .chain(fillet(
                at(side * half, SHELF_BACK),
                at(side * NOTCH_HALF, SHELF_BACK),
                at(side * NOTCH_HALF, NOTCH_FRONT),
                SHELF_LIP,
            ))
            .chain([at(side * NOTCH_HALF, NOTCH_FRONT)])
            .collect();
        // Each face's path, what it drops to, and its shade at top and foot:
        // deep down to the bed; only a step down to the shelf; the notch's
        // sides in the shelf's shade.
        for (path, bed, [top, foot]) in [
            (inner_edge(side, y0, SHELF_BACK), bed, [1.0, 0.55]),
            (
                inner_edge(side, SHELF_BACK, CASE_FRONT),
                SHELF_Z,
                [1.0, 0.9],
            ),
            (notch_side, bed, [0.75, 0.38]),
        ] {
            for pair in path.windows(2) {
                let [a, b] = [pair[0], pair[1]];
                let along = sub(b, a);
                let facing = [-side * along[1], side * along[0], 0.0];
                if !Eye::sees(a, facing) {
                    continue;
                }
                let [colour, low] = [top, foot].map(|by| matte(brighten(IVORY_SHADE, by), facing));
                eye.quad(
                    &mut solid,
                    [
                        (a, colour),
                        (b, colour),
                        ([b[0], b[1], bed], low),
                        ([a[0], a[1], bed], low),
                    ],
                );
            }
        }
    }
    canvas.mesh(Layer::Opaque, solid);
}

/// The panel's edge over the well: the levers go in under it.
pub(super) fn paint_panel_edge(canvas: &Canvas, eye: &Eye) {
    let (y0, z0) = PANEL_BOTTOM;
    let (half, under) = (OPENING_HALF, z0 - PANEL_EDGE_MM);
    let edge = [
        [-half, y0, z0],
        [half, y0, z0],
        [half, y0, under],
        [-half, y0, under],
    ];
    eye.fill(canvas, &edge, |_| IVORY_SHADE);
}

/// The case round the keys, over all in its well: the walls' tops, the
/// shelf, the rounded front and the plinth, and the walls' outer edges.
pub(super) fn paint_frame(canvas: &Canvas, eye: &Eye) {
    let outline = case_side();
    paint_wall_tops(canvas, eye, &outline);
    paint_shelf(canvas, eye);
    paint_case_front(canvas, eye);
    for side in [-1.0, 1.0] {
        let edge: Vec<[f32; 3]> = outline.iter().map(|&[x, y, z]| [side * x, y, z]).collect();
        eye.line(canvas, &edge, 0.3, EDGE);
    }
}

/// The walls' tops, lit as they swoop down to the case's front corners:
/// across from each point of the case's outer edge `outline` to the well.
fn paint_wall_tops(canvas: &Canvas, eye: &Eye, outline: &[[f32; 3]]) {
    let half = OPENING_HALF;
    let mut solid = Solid::default();
    for side in [-1.0, 1.0] {
        for pair in outline.windows(2) {
            let [a, b] = [pair[0], pair[1]];
            let colour = matte(IVORY, [0.0, a[2] - b[2], b[1] - a[1]]);
            let (outer_a, outer_b) = ([side * a[0], a[1], a[2]], [side * b[0], b[1], b[2]]);
            eye.quad(
                &mut solid,
                [
                    ([side * half, a[1], a[2]], colour),
                    (outer_a, colour),
                    (outer_b, colour),
                    ([side * half, b[1], b[2]], colour),
                ],
            );
        }
    }
    canvas.mesh(Layer::Opaque, solid);
}

/// The shelf between the walls: a block each side, rounding off into the
/// well beside the notch, and the strip in front of the notch.
fn paint_shelf(canvas: &Canvas, eye: &Eye) {
    let (half, notch) = (OPENING_HALF, NOTCH_HALF);
    let shelf_colour = matte(IVORY, [0.0, 0.0, 1.0]);
    let at = |x: f32, y: f32| [x, y, SHELF_Z];
    for side in [-1.0, 1.0] {
        let block = rounded(&[
            (at(side * half, SHELF_BACK), 0.0),
            (at(side * notch, SHELF_BACK), SHELF_LIP),
            (at(side * notch, CASE_FRONT), 0.0),
            (at(side * half, CASE_FRONT), 0.0),
        ]);
        eye.fill(canvas, &block, |_| shelf_colour);
        let corner = at(side * notch, NOTCH_FRONT);
        let fill: Vec<[f32; 3]> = std::iter::once(corner)
            .chain(fillet(
                at(side * notch, SHELF_BACK),
                corner,
                at(-side * notch, NOTCH_FRONT),
                NOTCH_ROUNDING,
            ))
            .collect();
        eye.fill(canvas, &fill, |_| shelf_colour);
    }
    let strip = [
        at(-notch, NOTCH_FRONT),
        at(notch, NOTCH_FRONT),
        at(notch, CASE_FRONT),
        at(-notch, CASE_FRONT),
    ];
    eye.fill(canvas, &strip, |_| shelf_colour);
}

/// The case's rounded front, its face turning with the corners, and the
/// plinth under it.
fn paint_case_front(canvas: &Canvas, eye: &Eye) {
    let front = case_front_edge();
    // Each point faces between its two neighbours: the shading blends round
    // the corners instead of stepping.
    let facing: Vec<[f32; 3]> = (0..front.len())
        .map(|i| {
            let before = front[i.saturating_sub(1)];
            let after = front[(i + 1).min(front.len() - 1)];
            let along = sub(after, before);
            [along[1], -along[0], 0.0]
        })
        .collect();
    let mut solid = Solid::default();
    for i in 0..front.len() - 1 {
        let (a, b) = (front[i], front[i + 1]);
        // Round the corners it turns away onto the sides: skip that.
        let along = sub(b, a);
        if !Eye::sees(a, [along[1], -along[0], 0.0]) {
            continue;
        }
        let [top_a, top_b] = [i, i + 1].map(|k| matte(IVORY, facing[k]));
        let [low_a, low_b] = [i, i + 1].map(|k| matte(brighten(IVORY_SHADE, 0.85), facing[k]));
        let [base_a, base_b] = [i, i + 1].map(|k| matte(PLINTH, facing[k]));
        let at = |p: [f32; 3], z: f32| [p[0], p[1], z];
        eye.quad(
            &mut solid,
            [
                (a, top_a),
                (b, top_b),
                (at(b, PLINTH_TOP), low_b),
                (at(a, PLINTH_TOP), low_a),
            ],
        );
        eye.quad(
            &mut solid,
            [
                (at(a, PLINTH_TOP), base_a),
                (at(b, PLINTH_TOP), base_b),
                (at(b, DESK_Z), base_b),
                (at(a, DESK_Z), base_a),
            ],
        );
    }
    canvas.mesh(Layer::Opaque, solid);
}

#[cfg(test)]
mod tests {
    use super::super::keyboard::{KEY_CAP, SPACE_BAR, SPACE_ROW, key_row};
    use super::*;

    #[test]
    fn the_walls_stand_above_the_keys_and_the_shelf_holds_the_bar() {
        let eye = Eye::testing(300.0, 48.0);
        for row in 0..4 {
            let (y, z) = key_row(row as f32);
            assert!(wall_top(y) > z + 5.0, "row {row}");
        }
        // The walls meet the shelf at the case's front corners, above it before.
        assert!((wall_top(CASE_FRONT) - SHELF_Z).abs() < 2.5e-3);
        assert!(wall_top(SPACE_ROW.0) > SHELF_Z);
        let (space_y, space_z) = SPACE_ROW;
        assert!(SHELF_Z <= space_z);
        assert!(SHELF_BACK < space_y - 5.0 && space_y + 5.0 < NOTCH_FRONT);
        assert!(
            SPACE_BAR
                .iter()
                .all(|&(left, right, _)| -NOTCH_HALF < left && right < NOTCH_HALF)
        );
        let bar = eye.at([0.0, space_y + 5.0, space_z]).y;
        let shelf = eye.at([0.0, CASE_FRONT, SHELF_Z]).y;
        assert!(bar < shelf, "{bar} {shelf}");
        assert!(key_row(3.0).0 + KEY_CAP.1 / 2.0 < SHELF_BACK);
    }

    #[test]
    fn the_case_corner_is_sampled_finely_to_its_end() {
        let corner: Vec<[f32; 3]> = case_side()
            .into_iter()
            .filter(|p| p[1] >= CASE_FRONT - FRAME_ROUNDING - 2.5e-3)
            .collect();
        // No step cuts across the curve: every one short, the last too.
        for pair in corner.windows(2) {
            let step = sub(pair[1], pair[0]);
            assert!(step[0].hypot(step[1]) < FRAME_ROUNDING / 10.0, "{pair:?}");
        }
        let last = corner[corner.len() - 1];
        assert!((last[1] - CASE_FRONT).abs() < 2.5e-3);
        assert!((last[0] - (PANEL_HALF_BOTTOM - FRAME_ROUNDING)).abs() < 2.5e-3);
    }

    #[test]
    fn the_case_front_faces_the_writer_or_the_sides() {
        for pair in case_front_edge().windows(2) {
            let along = sub(pair[1], pair[0]);
            // Outward, right to left: never back into the well.
            assert!(-along[0] >= -1e-4, "{pair:?}");
        }
    }
}
