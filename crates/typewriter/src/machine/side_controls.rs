//! Beside the keys: the carriage lock, the touch control and the ribbon
//! selector, and their marks on the walls.

use eframe::egui::Color32;
use glam::Vec3;

use super::canvas::Canvas;
use super::case::{OPENING_HALF, wall_top};
use super::eye::Eye;
use super::geometry::rounded_rect;
use super::keyboard::{INTO_MACHINE, KEY_FRONT, key_row, paint_cap};
use super::light::{brighten, matte, paint_chrome, paint_steel};
use super::panel::{PANEL_BOTTOM, PANEL_HALF_BOTTOM};
use super::{ENGRAVED, METAL_SHINE, SHIFT_CAP, SHIFT_FRONT, STEM, STEM_SHINE};

/// In the gaps between the keys and the walls, `x` from the middle: the
/// carriage lock by the far row, its green cap's half width and half depth.
const LOCK_X: f32 = 140.0;
const LOCK_CAP: (f32, f32) = (2.0, 3.0);
/// The lock's cap stands this far above the far row's tops, and this far
/// behind them.
const LOCK_RISE: f32 = 6.0;
const LOCK_BACK: f32 = 3.0;
/// The touch control and ribbon selectors by the third row: how far each
/// bends toward the writer from its post, and its chrome paddle's half width
/// and height.
const SELECTOR_X: f32 = 144.0;
const SELECTOR_REACH: f32 = 6.0;
const PADDLE: (f32, f32) = (2.0, 9.0);
/// The paddles reach up to just under the walls' tops, level with their
/// marks.
const PADDLE_BELOW_WALL: f32 = 1.0;
/// How far the lock's post runs on below its cap, and the selectors' below
/// their bends, before their rods run back into the machine: the selectors'
/// rods well under the lock's.
const LOCK_DROP: f32 = 10.0;
const SELECTOR_DROP: f32 = 23.0;
/// The ribbon colour selector's marks, back to front: blue (the ribbon's
/// black half), white (stencil), red; and where each is from the third
/// row's middle.
const RIBBON_MARKS: [Color32; 3] = [
    Color32::from_rgb(0x2A, 0x4F, 0x9A),
    Color32::from_rgb(0xF2, 0xF2, 0xEE),
    Color32::from_rgb(0xB0, 0x30, 0x2A),
];
const RIBBON_ALONG: [f32; 3] = [-6.0, 0.0, 6.0];
/// Typing in black: the selector at blue.
const RIBBON_SET: usize = 0;
/// A ribbon mark's half size, rounding, and how far it stands proud.
const RIBBON_MARK: (f32, f32, f32) = (1.5, 0.5, 0.5);
/// The touch control's less and more marks: where from the third row's
/// middle, their half length and stroke.
const TOUCH_MARKS: (f32, f32, f32) = (8.0, 3.0, 1.0);
/// The dark outline round small raised marks.
const MARK_EDGE: Color32 = Color32::from_rgb(0x30, 0x2C, 0x26);

/// In the gaps between the keys and the walls: on the left, the carriage
/// lock by the far row, standing above it, and the touch control's selector
/// by the third, midway between less and more; on the right, the ribbon
/// colour selector at its setting. Each selector's paddle stands up level
/// with its marks on the wall.
pub(super) fn paint(canvas: &Canvas, eye: &Eye) {
    let row = key_row(0.0);
    paint_lock(
        canvas,
        eye,
        Vec3::new(-LOCK_X, row.x - LOCK_BACK, row.y + LOCK_RISE),
    );
    let y = key_row(2.0).x;
    let ribbon = y + RIBBON_ALONG[RIBBON_SET];
    for (x, paddle_y) in [(-SELECTOR_X, y), (SELECTOR_X, ribbon)] {
        let top = wall_top(paddle_y) - PADDLE_BELOW_WALL;
        paint_selector(canvas, eye, Vec3::new(x, paddle_y - SELECTOR_REACH, top));
    }
}

/// The selectors' marks on the walls' tops beside them: a bold less and more
/// on the left; on the right, the ribbon's colours back to front, each a
/// small rounded square standing proud of the case, outlined dark.
pub(super) fn paint_marks(canvas: &Canvas, eye: &Eye) {
    let on_wall = (OPENING_HALF + PANEL_HALF_BOTTOM) / 2.0;
    let y = key_row(2.0).x;
    let (along, half, stroke) = TOUCH_MARKS;
    let bar = |centre: f32, upright: bool| {
        let (dx, dy) = if upright { (0.0, half) } else { (half, 0.0) };
        [
            Vec3::new(-on_wall - dx, centre - dy, wall_top(centre - dy)),
            Vec3::new(-on_wall + dx, centre + dy, wall_top(centre + dy)),
        ]
    };
    for part in [
        bar(y - along, false),
        bar(y + along, false),
        bar(y + along, true),
    ] {
        eye.line(canvas, &part, stroke, ENGRAVED);
    }
    let (size, round, proud) = RIBBON_MARK;
    for (colour, along) in RIBBON_MARKS.into_iter().zip(RIBBON_ALONG) {
        let (x, y) = (on_wall, y + along);
        let z = wall_top(y) + proud;
        // Its front standing off the case, then its face.
        let foot = z - proud;
        let front = [
            Vec3::new(x - size, y + size, z),
            Vec3::new(x + size, y + size, z),
            Vec3::new(x + size, y + size, foot),
            Vec3::new(x - size, y + size, foot),
        ];
        eye.fill(canvas, &front, |_| brighten(colour, 0.6));
        let face = rounded_rect([x - size, x + size], [y - size, y + size], z, round);
        eye.fill(canvas, &face, |_| matte(colour, Vec3::Z));
        eye.outline_in(canvas, &face, MARK_EDGE);
    }
}

/// The carriage lock: a lever whose green cap's top is at `top`, its post
/// running on down below the cap, then back into the machine.
fn paint_lock(canvas: &Canvas, eye: &Eye, top: Vec3) {
    let (x, y, z) = (top.x, top.y, top.z);
    let foot = Vec3::new(x, y, z - KEY_FRONT - LOCK_DROP);
    let path = [Vec3::new(x, y, z - KEY_FRONT), foot, into_machine(foot)];
    paint_steel(canvas, eye, &path, STEM, STEM_SHINE);
    let (half, depth) = LOCK_CAP;
    let colours = [SHIFT_CAP, SHIFT_FRONT];
    paint_cap(
        canvas,
        eye,
        [x - half, x + half],
        [y - depth, y + depth],
        z,
        colours,
    );
}

/// A selector: a post at `post` (its paddle's top level) up from a rod out
/// of the machine, bending toward the writer to an upright chrome paddle
/// facing them, lit in bands as polished metal is.
fn paint_selector(canvas: &Canvas, eye: &Eye, post: Vec3) {
    let (x, y, top) = (post.x, post.y, post.z);
    let (half, height) = PADDLE;
    let bend = Vec3::new(x, y, top - height);
    // Its post runs well down, then back into the machine under the lock's.
    let foot = Vec3::new(x, y, bend.z - SELECTOR_DROP);
    paint_steel(
        canvas,
        eye,
        &[bend, foot, into_machine(foot)],
        STEM,
        STEM_SHINE,
    );
    let paddle_y = y + SELECTOR_REACH;
    eye.line(
        canvas,
        &[bend, Vec3::new(x, paddle_y, top - height)],
        1.3,
        STEM,
    );
    // Its top edge behind its face.
    let rim = [
        Vec3::new(x - half, paddle_y - 1.0, top),
        Vec3::new(x + half, paddle_y - 1.0, top),
        Vec3::new(x + half, paddle_y, top),
        Vec3::new(x - half, paddle_y, top),
    ];
    eye.fill(canvas, &rim, |_| METAL_SHINE);
    paint_chrome(canvas, eye, [x - half, x + half], paddle_y, top, height);
}

/// From `foot` straight back, level, into the machine under the panel.
fn into_machine(foot: Vec3) -> Vec3 {
    Vec3::new(foot.x, PANEL_BOTTOM.0 - INTO_MACHINE, foot.z)
}

#[cfg(test)]
mod tests {
    use super::super::case::KEY_BED_Z;
    use super::super::keyboard::{KEYS, Key};
    use super::*;

    #[test]
    fn the_side_controls_fit_between_the_keys_and_the_walls() {
        let edge = |key: &Key| (key.x() - key.half_width(), key.x() + key.half_width());
        // The lock beside the far row's first key; the selectors beside the
        // third row's ends; all inside the walls.
        assert!(-LOCK_X + LOCK_CAP.0 < edge(&KEYS[0][0]).0);
        assert!(-SELECTOR_X + PADDLE.0 < edge(&KEYS[2][0]).0);
        assert!(SELECTOR_X - PADDLE.0 > edge(&KEYS[2][KEYS[2].len() - 1]).1);
        assert!(LOCK_X + LOCK_CAP.0 < OPENING_HALF);
        assert!(SELECTOR_X + PADDLE.0 < OPENING_HALF);
    }

    #[test]
    fn the_lock_stands_above_the_keys_and_the_selectors_by_their_marks() {
        // At least half the lock's cap above the far row's tops.
        let row_top = key_row(0.0).y;
        let cap_top = row_top + LOCK_RISE;
        assert!(cap_top - row_top >= row_top - (cap_top - KEY_FRONT));
        // The ribbon selector beside the blue mark, level with it.
        let eye = Eye::testing(300.0, 48.0);
        let y = key_row(2.0).x;
        let mark_y = y + RIBBON_ALONG[RIBBON_SET];
        let mark = eye.at(Vec3::new(SELECTOR_X, mark_y, wall_top(mark_y))).y;
        let paddle = eye
            .at(Vec3::new(
                SELECTOR_X,
                mark_y,
                wall_top(mark_y) - PADDLE_BELOW_WALL,
            ))
            .y;
        let next = y + RIBBON_ALONG[RIBBON_SET + 1];
        let next_mark = eye.at(Vec3::new(SELECTOR_X, next, wall_top(next))).y;
        assert!((paddle - mark).abs() < (paddle - next_mark).abs());
        assert_eq!(RIBBON_SET, 0, "blue, for black");
    }

    #[test]
    fn the_selectors_rods_run_well_under_the_locks() {
        let row_top = key_row(0.0).y;
        let lock_rod = row_top + LOCK_RISE - KEY_FRONT - LOCK_DROP;
        let y = key_row(2.0).x;
        let touch_rod = wall_top(y) - PADDLE_BELOW_WALL - PADDLE.1 - SELECTOR_DROP;
        assert!(touch_rod < lock_rod - 8.0, "{touch_rod} {lock_rod}");
        // Both above the key bed.
        assert!(touch_rod > KEY_BED_Z);
    }
}
