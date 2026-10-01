//! One light, to the writer's left, above and a little in front, and how
//! plastic and metal take it.

use eframe::egui::Color32;

use super::canvas::Canvas;
use super::eye::{Eye, toward_eye};
use super::geometry::{dot, normalized};
use super::{METAL, METAL_SHINE};
use crate::depth::{Layer, Solid};

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

/// An upright chrome plate facing the writer over `x` at depth `y`, from
/// `top` down `height` inches, outlined. Chrome mirrors the room: bright sky
/// above, dark room below, a bright band where it turns to the light.
pub(super) fn paint_chrome(
    canvas: &Canvas,
    eye: &Eye,
    [left, right]: [f32; 2],
    y: f32,
    top: f32,
    height: f32,
) {
    let mut solid = Solid::default();
    for pair in chrome_bands().windows(2) {
        let [(from, upper), (to, lower)] = [pair[0], pair[1]];
        let at = |x: f32, t: f32| [x, y, top - height * t];
        eye.quad(
            &mut solid,
            [
                (at(left, from), upper),
                (at(right, from), upper),
                (at(right, to), lower),
                (at(left, to), lower),
            ],
        );
    }
    canvas.mesh(Layer::Opaque, solid);
    let bottom = top - height;
    let outline = [
        [left, y, top],
        [right, y, top],
        [right, y, bottom],
        [left, y, bottom],
    ];
    eye.outline(canvas, &outline);
}

/// Chrome from its top (0) to its bottom (1): the sky, a bright band where
/// it turns to the light, the dark room.
fn chrome_bands() -> [(f32, Color32); 5] {
    [
        (0.0, METAL_SHINE),
        (0.3, brighten(METAL, 0.8)),
        (0.55, Color32::WHITE),
        (0.75, METAL),
        (1.0, brighten(METAL, 0.6)),
    ]
}

/// Chrome `t` of the way across a part, as [`paint_chrome`] shades it.
pub(super) fn chrome_at(t: f32) -> Color32 {
    let bands = chrome_bands();
    let t = t.clamp(0.0, 1.0);
    bands
        .windows(2)
        .find(|pair| t <= pair[1].0)
        .map_or(bands[4].1, |pair| {
            let [(from, upper), (to, lower)] = [pair[0], pair[1]];
            upper.lerp_to_gamma(lower, (t - from) / (to - from))
        })
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
