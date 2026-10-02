//! The still body behind the sheet: its shadow on the desk, its top under
//! the carriage.

use eframe::egui::{Color32, Painter, Pos2, Shape};

use super::IVORY_SHADE;
use super::canvas::Canvas;
use super::case::{CASE_FRONT, FRAME_ROUNDING};
use super::cover::COVER_HALF;
use super::eye::Eye;
use super::geometry::{rounded, soft};
use super::light::{brighten, matte};
use super::panel::PANEL_HALF_BOTTOM;

/// The light is also to the writer's left: the machine's shadow falls right
/// and a little back on the desk, `(x, y)` millimetres.
const CAST_SHADOW: (f32, f32) = (23.0, -13.0);
/// The body's top under the carriage, as wide as the ribbon cover's back,
/// to under the platen's front, behind the ribbon: its `y` range and `z`.
const DECK: (f32, f32) = (-46.0, 0.0);
const DECK_Z: f32 = -14.0;
/// The body's back on the desk, under the carriage, and the desk's top.
const BODY_BACK: f32 = -66.0;
pub(super) const DESK_Z: f32 = -110.0;

/// Its shadows on the desk: cast away from the light, and the contact
/// shadow round its base.
pub(super) fn paint_shadow(painter: &Painter, eye: &Eye) {
    let (cast, contact) = (footprint(eye, CAST_SHADOW), footprint(eye, (0.0, 0.0)));
    let (cast_blur, contact_blur) = (23.0 * eye.ppmm, 4.0 * eye.ppmm);
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

/// Where the body stands on the desk, moved `(x, y)` millimetres, on screen:
/// its front corners rounded as the case's are.
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
pub(super) fn paint_deck(canvas: &Canvas, eye: &Eye) {
    let ((back, front), half) = (DECK, COVER_HALF.0);
    let deck = [
        [-half, back, DECK_Z],
        [half, back, DECK_Z],
        [half, front, DECK_Z],
        [-half, front, DECK_Z],
    ];
    eye.fill(canvas, &deck, |[_, y, _]| {
        let shaded =
            brighten(IVORY_SHADE, 0.7).lerp_to_gamma(IVORY_SHADE, (y - back) / (front - back));
        matte(shaded, [0.0, 0.0, 1.0])
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
