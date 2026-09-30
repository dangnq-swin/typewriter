//! The still body behind the sheet: its shadow on the desk, the deck the
//! carriage rides on, the throat under the platen.

use eframe::egui::{Color32, Painter, Pos2, Shape};

use super::case::{CASE_FRONT, FRAME_ROUNDING};
use super::eye::Eye;
use super::geometry::{rounded, soft};
use super::panel::PANEL_HALF_BOTTOM;
use super::{CHROME, DARK};

/// The light is also to the writer's left: the machine's shadow falls right
/// and a little back on the desk, `(x, y)` inches.
const CAST_SHADOW: (f32, f32) = (0.9, -0.5);
/// The deck under the carriage, and the rail in it: `y` ranges, and `z`.
const DECK: (f32, f32) = (-1.8, 0.1);
const RAIL: (f32, f32) = (-1.25, -0.75);
const DECK_Z: f32 = -0.55;
const DECK_HALF: f32 = 6.2;
/// The dark throat between platen and cover, behind the card holder.
const THROAT_TOP: (f32, f32) = (0.05, -0.3);
/// The body's back on the desk, under the carriage, and the desk's top.
const BODY_BACK: f32 = -2.6;
pub(super) const DESK_Z: f32 = -4.35;
const INSIDE_FRONT: Color32 = Color32::from_rgb(0x2A, 0x2A, 0x28);

/// Its shadows on the desk: cast away from the light, and the contact
/// shadow round its base.
pub(super) fn paint_shadow(painter: &Painter, eye: &Eye) {
    let (cast, contact) = (footprint(eye, CAST_SHADOW), footprint(eye, (0.0, 0.0)));
    let (cast_blur, contact_blur) = (0.9 * eye.ppi, 0.15 * eye.ppi);
    painter.add(Shape::mesh(soft(
        &cast,
        cast_blur,
        Color32::from_black_alpha(80),
    )));
    painter.add(Shape::mesh(soft(
        &contact,
        contact_blur,
        Color32::from_black_alpha(110),
    )));
}

/// Where the body stands on the desk, moved `(x, y)` inches, on screen: its
/// front corners rounded as the case's are.
fn footprint(eye: &Eye, (dx, dy): (f32, f32)) -> Vec<Pos2> {
    let (half, back, front) = (PANEL_HALF_BOTTOM, BODY_BACK, CASE_FRONT);
    eye.polygon(&rounded(&[
        ([-half + dx, back + dy, DESK_Z], 0.0),
        ([half + dx, back + dy, DESK_Z], 0.0),
        ([half + dx, front + dy, DESK_Z], FRAME_ROUNDING),
        ([-half + dx, front + dy, DESK_Z], FRAME_ROUNDING),
    ]))
}

/// The deck under the carriage, and the rail in it.
pub(super) fn paint_deck(painter: &Painter, eye: &Eye) {
    let across = |y: (f32, f32)| {
        [
            [-DECK_HALF, y.0, DECK_Z],
            [DECK_HALF, y.0, DECK_Z],
            [DECK_HALF, y.1, DECK_Z],
            [-DECK_HALF, y.1, DECK_Z],
        ]
    };
    eye.fill(painter, &across(DECK), |_| INSIDE_FRONT);
    eye.fill(painter, &across(RAIL), |_| DARK);
    let rail_lip = [[-DECK_HALF, RAIL.1, DECK_Z], [DECK_HALF, RAIL.1, DECK_Z]];
    eye.line(painter, &rail_lip, 0.03, CHROME);
}

/// The dark throat between platen and cover, still, behind the sheet: what
/// shows of it is where no sheet is, and below the card holder.
pub(super) fn paint_throat(painter: &Painter, eye: &Eye) {
    let (y, z) = THROAT_TOP;
    let throat = [
        [-6.0, y, z],
        [6.0, y, z],
        [6.0, 1.0, DECK_Z],
        [-6.0, 1.0, DECK_Z],
    ];
    eye.fill(painter, &throat, |_| DARK);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_machine_casts_its_shadow_one_way() {
        let eye = Eye::testing(300.0, 48.0);
        let (body, cast) = (footprint(&eye, (0.0, 0.0)), footprint(&eye, CAST_SHADOW));
        // Right and left ends of the front: hidden under the body on the
        // left, past it on the right.
        let right = |p: &[Pos2]| p.iter().map(|q| q.x).fold(f32::MIN, f32::max);
        let left = |p: &[Pos2]| p.iter().map(|q| q.x).fold(f32::MAX, f32::min);
        assert!(left(&cast) > left(&body));
        assert!(right(&cast) > right(&body));
    }
}
