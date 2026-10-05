//! The still body behind the sheet: the desk's plane that catches the
//! machine's shadow, and the body's top under the carriage.

use glam::Vec3;

use super::IVORY_SHADE;
use super::canvas::Canvas;
use super::case::CASE_FRONT;
use super::cover::{COVER_BACK, COVER_CORNER, COVER_HALF, INSIDE};
use super::eye::Eye;
use super::light::{SHADOW_CENTRE_X, SHADOW_HALF, catcher, matte};

/// The body's top under the carriage, as wide as the ribbon cover's back,
/// to under the platen's front, behind the ribbon: its `y` range and `z`.
const DECK: (f32, f32) = (-46.0, 0.0);
const DECK_Z: f32 = -14.0;
pub(crate) const DESK_Z: f32 = -110.0;

/// The catcher quad's bounds: `left`, `right`, `front`, `back`. The two
/// `x`s are the map's span — `SHADOW_HALF` about `SHADOW_CENTRE_X`, the
/// map's own constants — not its whole reach across the desk: the map's
/// tilted axes cut a wider parallelogram (`Shadow::plane_reach`), but
/// nothing out there can throw a shadow the desk shows. The quad must
/// stay wholly inside the map, or its catcher reads clamped edge texels
/// and shadows cut at low zoom: `light`'s test holds that, the one below
/// that the quad lies within the reach.
pub(super) const DESK: (f32, f32, f32, f32) = (
    SHADOW_CENTRE_X - SHADOW_HALF,
    SHADOW_CENTRE_X + SHADOW_HALF,
    CASE_FRONT + 28.0,
    crate::room::WALL_Y,
);

/// The desk's plane, laid at the machine's feet over the room's backdrop:
/// black where the lamp's own map says the machine hides it, clear where
/// the lamp reaches. It runs the width the map reads — past its cleared
/// edge no shadow can fall — and stops at the room's wall
/// ([`crate::room::WALL_Y`]): run on past there, it would catch the sheet's
/// long shadow over the wall, which the room's own horizon knows nothing
/// of. A decal: it hides nothing and casts nothing.
pub(super) fn paint_desk(canvas: &Canvas, eye: &Eye) {
    let (left, right, front, back) = DESK;
    let desk = [
        Vec3::new(left, back, DESK_Z),
        Vec3::new(right, back, DESK_Z),
        Vec3::new(right, front, DESK_Z),
        Vec3::new(left, front, DESK_Z),
    ];
    eye.fill(canvas, &desk, |_| catcher());
}

/// The body's top under the carriage, showing where the carriage has
/// travelled off it: one ivory face square to the lamp, which lights it and
/// hides it as the carriage, the sheet and the cover stand between. Under
/// its front edge, a hidden riser closes the slot to the cover's back foot:
/// the chair never sees it — the cover's slope cuts the sight line — but
/// without it the lamp would shine through the slot and cut a bright band
/// across the machine's shadow on the desk.
pub(super) fn paint_deck(canvas: &Canvas, eye: &Eye) {
    let (back, front) = DECK;
    let half = COVER_HALF.0;
    let deck = [
        Vec3::new(-half, back, DECK_Z),
        Vec3::new(half, back, DECK_Z),
        Vec3::new(half, front, DECK_Z),
        Vec3::new(-half, front, DECK_Z),
    ];
    eye.fill(canvas, &deck, |_| matte(IVORY_SHADE, Vec3::Z));

    // From the deck's front edge up to the cover's back foot, dark, as it
    // reads as the machine's inside if any sight line ever finds it. Its
    // ends stop short of the cover's own, which round away there: square to
    // the corner, its ends would bare themselves past the cover's fillet.
    let (foot_y, foot_z) = COVER_BACK;
    let half = half - COVER_CORNER;
    let riser = [
        Vec3::new(-half, front, DECK_Z),
        Vec3::new(half, front, DECK_Z),
        Vec3::new(half, foot_y, foot_z),
        Vec3::new(-half, foot_y, foot_z),
    ];
    let normal = Vec3::new(0.0, DECK_Z - foot_z, foot_y - front).normalize();
    eye.fill(canvas, &riser, |_| matte(INSIDE, normal));
}

#[cfg(test)]
mod tests {
    use super::super::light::shadow;
    use super::*;
    use crate::depth::Shade;

    #[test]
    fn the_desk_catcher_lies_flat_and_casts_nothing() {
        // Black, a shade under opaque so the canvas lays it as a decal,
        // shown only by the shadow it stands in.
        let paint = catcher();
        assert!(paint.colour.a() < u8::MAX, "a decal, not a face");
        assert_eq!(
            paint.shade,
            Shade::Catcher(0.38),
            "as dark as the old painted shadow"
        );
    }

    /// The quad's bounds ride the map's own constants and sit wholly
    /// inside the reach `plane_reach` gives it across the desk: if
    /// `SHADOW_HALF`, the centre or `lamp()` drift, the bounds move with
    /// them instead of cutting the map short or reading clamped edge
    /// texels — `light`'s test holds the corners inside the reading.
    #[test]
    fn the_desk_catcher_lies_within_the_maps_projected_reach() {
        let (left, right, front, back) = DESK;
        let (reach_left, reach_right, reach_back, reach_front) =
            shadow().plane_reach(DESK_Z).unwrap();
        assert!(
            reach_left < left && right < reach_right,
            "x span inside the reach {reach_left}..{reach_right}: {left}..{right}"
        );
        assert!(
            reach_back < back && front < reach_front,
            "y ends inside the reach {reach_back}..{reach_front}: {back}..{front}"
        );
        // And the room's own bounds still end the quad in y, as ever.
        assert_eq!((front, back), (CASE_FRONT + 28.0, crate::room::WALL_Y));
    }
}
