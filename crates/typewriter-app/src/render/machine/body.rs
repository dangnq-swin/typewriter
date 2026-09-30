//! The still body behind the sheet: its shadow on the desk, its top under
//! the carriage, the shaded gap under the platen.

use eframe::egui::{Color32, Painter, Pos2, Shape};

use super::IVORY_SHADE;
use super::case::{CASE_FRONT, FRAME_ROUNDING};
use super::cover::COVER_HALF;
use super::eye::Eye;
use super::geometry::{rounded, soft};
use super::light::{brighten, matte};
use super::panel::PANEL_HALF_BOTTOM;

/// The light is also to the writer's left: the machine's shadow falls right
/// and a little back on the desk, `(x, y)` inches.
const CAST_SHADOW: (f32, f32) = (0.9, -0.5);
/// The body's top under the carriage, as wide as the ribbon cover's back and
/// reaching forward under it: its `y` range and `z`.
const DECK: (f32, f32) = (-1.8, 0.35);
const DECK_Z: f32 = -0.55;
/// The gap under the platen, from where the platen's front turns out of
/// sight, `(y, z)`, forward to the deck's front.
pub(super) const THROAT_TOP: (f32, f32) = (0.05, -0.3);
/// The body's back on the desk, under the carriage, and the desk's top.
const BODY_BACK: f32 = -2.6;
pub(super) const DESK_Z: f32 = -4.35;

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

/// The body's top under the carriage, showing where the carriage has
/// travelled off it: in the carriage's shade toward the back.
pub(super) fn paint_deck(painter: &Painter, eye: &Eye) {
    let ((back, front), half) = (DECK, COVER_HALF.0);
    let deck = [
        [-half, back, DECK_Z],
        [half, back, DECK_Z],
        [half, front, DECK_Z],
        [-half, front, DECK_Z],
    ];
    let lit = matte(IVORY_SHADE, [0.0, 0.0, 1.0]);
    eye.fill(painter, &deck, |[_, y, _]| {
        brighten(lit, 0.7).lerp_to_gamma(lit, (y - back) / (front - back))
    });
}

/// The gap under the platen, between the carriage's ends `x` but no wider
/// than the body, travelling with it: dark under the platen, the body's top
/// in its shade forward.
pub(super) fn paint_throat(painter: &Painter, eye: &Eye, ends: [f32; 2]) {
    let half = COVER_HALF.0;
    let [left, right] = ends.map(|x| x.clamp(-half, half));
    let ((y, z), front) = (THROAT_TOP, DECK.1);
    let throat = [
        [left, y, z],
        [right, y, z],
        [right, front, DECK_Z],
        [left, front, DECK_Z],
    ];
    let (under, shade) = (brighten(IVORY_SHADE, 0.15), brighten(IVORY_SHADE, 0.45));
    eye.fill(painter, &throat, |[_, py, _]| {
        under.lerp_to_gamma(shade, (py - y) / (front - y))
    });
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
