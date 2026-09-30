//! One light, to the writer's left, above and a little in front, and how
//! plastic and metal take it.

use eframe::egui::Color32;

use super::eye::toward_eye;
use super::geometry::{dot, normalized};

/// Toward the light from the machine: to the writer's left, above, a little
/// in front.
pub(super) fn toward_light() -> [f32; 3] {
    normalized([-0.6, 0.4, 0.7])
}

/// Plastic facing `normal`: lit over an ambient floor (Lambert).
pub(super) fn matte(colour: Color32, normal: [f32; 3]) -> Color32 {
    let lit = dot(normalized(normal), toward_light()).max(0.0);
    brighten(colour, 0.7 + 0.4 * lit)
}

/// How bright a thin metal part running along `tangent` catches the light,
/// 0..=1: brushed and milled metal streaks along its grain
/// (Heidrich–Seidel). `sharpness` narrows the streak.
pub(super) fn streak(tangent: [f32; 3], sharpness: i32) -> f32 {
    let tangent = normalized(tangent);
    let (lt, vt) = (dot(toward_light(), tangent), dot(toward_eye(), tangent));
    let across = (1.0 - lt * lt).max(0.0).sqrt() * (1.0 - vt * vt).max(0.0).sqrt();
    (across - lt * vt).max(0.0).powi(sharpness)
}

/// `colour` lit `by` times as bright, alpha kept.
pub(super) fn brighten(colour: Color32, by: f32) -> Color32 {
    // Safe casts: clamped to a byte.
    let channel = |c: u8| (f32::from(c) * by).clamp(0.0, 255.0) as u8;
    Color32::from_rgba_premultiplied(
        channel(colour.r()),
        channel(colour.g()),
        channel(colour.b()),
        colour.a(),
    )
}

#[cfg(test)]
mod tests {
    use super::super::IVORY;
    use super::*;

    #[test]
    fn one_light_from_the_front_left() {
        let (left, right) = (
            matte(IVORY, [-1.0, 0.0, 0.0]),
            matte(IVORY, [1.0, 0.0, 0.0]),
        );
        assert!(left.r() > right.r());
        let up = matte(IVORY, [0.0, 0.0, 1.0]);
        assert!(up.r() > matte(IVORY, [0.0, 0.0, -1.0]).r());
        for angle in 0..36 {
            let turn = (10.0 * angle as f32).to_radians();
            let shine = streak([turn.cos(), turn.sin(), 0.0], 6);
            assert!((0.0..=1.0).contains(&shine));
        }
    }
}
