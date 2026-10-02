//! The desk's lamp, where a writer would place one: to the left, above and a
//! little in front, its head out over the keys and clear of the work — usual
//! enough to be believed, far enough never to be in the way. It burns the
//! room's own white: what it lights is what is there, brighter where it
//! turns. The shader lights what stands in depth from it, per fragment;
//! [`Paint::lit`] is its twin at the machine's middle for the few parts the
//! CPU still lights, one colour a frame.

use eframe::egui::Color32;
use glam::Vec3;

use super::canvas::Canvas;
use super::eye::{Eye, toward_eye};
use super::{METAL, METAL_SHINE};
use crate::depth::{CHROME_BANDS, Layer, Lighting, Shade, Solid};

/// Where the lamp stands, in machine millimetres from the printing point:
/// over the desk's left front, its head about 400 mm above the desk's
/// top. Seen from the machine's middle it lies where the old direction
/// pointed, so what the CPU lights keeps its look.
pub(super) fn lamp() -> Vec3 {
    Vec3::new(-254.0, 170.0, 297.0)
}

/// Toward the lamp from the machine's middle: for lines and cast shadows,
/// which stand nowhere of their own. The shader reaches each fragment's own
/// direction from [`lamp`] instead.
pub(super) fn toward_light() -> Vec3 {
    lamp().normalize_or_zero()
}

/// The frame's lighting: this lamp, this eye, and the room chrome mirrors.
pub(super) fn frame() -> Lighting {
    Lighting {
        lamp: lamp(),
        eye: toward_eye(),
        chrome: chrome_bands(),
    }
}

/// A vertex's colour, and how it takes the light.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct Paint {
    pub(super) colour: Color32,
    pub(super) shade: Shade,
}

impl From<Color32> for Paint {
    fn from(colour: Color32) -> Self {
        Self {
            colour,
            shade: Shade::Unlit,
        }
    }
}

impl Paint {
    /// Lit at the machine's middle, for the parts the CPU still lights
    /// itself, one colour a frame.
    pub(super) fn lit(self) -> Color32 {
        self.shade.colour(self.colour, &frame(), Vec3::ZERO)
    }
}

/// Plastic facing `normal`: lit over an ambient floor (Lambert), per pixel.
pub(super) fn matte(colour: Color32, normal: Vec3) -> Paint {
    Paint {
        colour,
        shade: Shade::Matte(normal.normalize_or_zero()),
    }
}

/// Metal facing `normal`, polished to a `shine` whose highlight `sharpness`
/// narrows: lit per pixel.
pub(super) fn polished(colour: Color32, shine: Color32, normal: Vec3, sharpness: f32) -> Paint {
    Paint {
        colour,
        shade: Shade::Polished {
            normal: normal.normalize_or_zero(),
            shine,
            sharpness,
        },
    }
}

/// Brushed metal running along `tangent`, streaked with `shine` across its
/// grain, `sharpness` narrow: lit per pixel.
pub(super) fn brushed(colour: Color32, shine: Color32, tangent: Vec3, sharpness: f32) -> Paint {
    Paint {
        colour,
        shade: Shade::Streak {
            tangent: tangent.normalize_or_zero(),
            shine,
            sharpness,
        },
    }
}

/// How bright a thin metal part running along `tangent` catches the light,
/// 0..=1: brushed and milled metal streaks along its grain (Heidrich–Seidel).
/// `sharpness` narrows the streak. For parts the CPU lights, one colour a
/// frame: what stands in depth is [`brushed`] instead.
pub(super) fn streak(tangent: Vec3, sharpness: i32) -> f32 {
    let tangent = tangent.normalize_or_zero();
    let (lt, vt) = (toward_light().dot(tangent), toward_eye().dot(tangent));
    let across = (1.0 - lt * lt).max(0.0).sqrt() * (1.0 - vt * vt).max(0.0).sqrt();
    (across - lt * vt).max(0.0).powi(sharpness)
}

/// A steel bar of the machine — key lever or type bar — one width, its hair
/// of light one aside, one floor under the streak.
const BAR_MM: f32 = 1.6;
const BAR_HAIR_MM: f32 = 0.4;
const BAR_HAIR_ASIDE_MM: f32 = 0.5;
const BAR_HAIR_SHARPNESS: f32 = 10.0;
const BAR_HAIR_FLOOR: f32 = 0.2;

/// A steel bar along `path`: a dark stroke with a hair of `shine` down its
/// lit edge, in `body`'s steel. Both stand in depth; the hair takes the
/// lamp's streak per pixel, along the bar it runs, brightening where it
/// turns to catch the light.
pub(super) fn paint_steel(
    canvas: &Canvas,
    eye: &Eye,
    path: &[Vec3],
    body: Color32,
    shine: Color32,
) {
    for pair in path.windows(2) {
        let [a, b] = [pair[0], pair[1]];
        eye.line(canvas, &[a, b], BAR_MM, body);
        let along = (b - a).normalize_or_zero();
        let aside = lit_side(along) * BAR_HAIR_ASIDE_MM;
        let hair = [a + aside, b + aside];
        let base = body.lerp_to_gamma(shine, BAR_HAIR_FLOOR);
        eye.line(
            canvas,
            &hair,
            BAR_HAIR_MM,
            brushed(base, shine, along, BAR_HAIR_SHARPNESS),
        );
    }
}

/// The unit side of a bar running `along` that shows the lamp: squared off
/// in the desk's plane and turned to the writer's left, where the lamp
/// stands; of an upright bar, straight left.
fn lit_side(along: Vec3) -> Vec3 {
    let aside = Vec3::new(-along.y, along.x, 0.0).normalize_or_zero();
    if aside.x < 0.0 { aside } else { -Vec3::X }
}

/// An upright chrome plate facing the writer over `x` at depth `y`, from
/// `top` down `height` millimetres, outlined. Chrome mirrors the room, so the
/// shader bands the plate: bright sky above, the light's band, the dark room
/// below.
pub(super) fn paint_chrome(
    canvas: &Canvas,
    eye: &Eye,
    [left, right]: [f32; 2],
    y: f32,
    top: f32,
    height: f32,
) {
    // Its own colour is the room's, not the plate's: the material reads it.
    let chrome = |t| Paint {
        colour: Color32::WHITE,
        shade: Shade::Chrome(t),
    };
    let bottom = top - height;
    let outline = [
        Vec3::new(left, y, top),
        Vec3::new(right, y, top),
        Vec3::new(right, y, bottom),
        Vec3::new(left, y, bottom),
    ];
    let mut solid = Solid::default();
    eye.quad(
        &mut solid,
        [
            (outline[0], chrome(0.0)),
            (outline[1], chrome(0.0)),
            (outline[2], chrome(1.0)),
            (outline[3], chrome(1.0)),
        ],
    );
    canvas.mesh(Layer::Opaque, solid);
    eye.outline(canvas, &outline);
}

/// Chrome from its top (0) to its bottom (1): the sky, a bright band where
/// it turns to the light, the dark room.
fn chrome_bands() -> [(f32, Color32); CHROME_BANDS] {
    [
        (0.0, METAL_SHINE),
        (0.3, brighten(METAL, 0.8)),
        (0.55, Color32::WHITE),
        (0.75, METAL),
        (1.0, brighten(METAL, 0.6)),
    ]
}

/// Chrome `t` of the way down a plate, as the material bands it: for what
/// lies in no plate of its own, like the return lever, whose colour follows
/// the way its seen side faces.
pub(super) fn chrome_at(t: f32) -> Color32 {
    Shade::Chrome(t).colour(Color32::WHITE, &frame(), Vec3::ZERO)
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
        let lit = |normal| matte(IVORY, normal).lit();
        let (left, right) = (lit(-Vec3::X), lit(Vec3::X));
        assert!(left.r() > right.r());
        assert!(lit(Vec3::Z).r() > lit(-Vec3::Z).r());
        for angle in 0..36 {
            let turn = (10.0 * angle as f32).to_radians();
            let shine = streak(Vec3::new(turn.cos(), turn.sin(), 0.0), 6);
            assert!((0.0..=1.0).contains(&shine));
        }
    }

    #[test]
    fn polished_metal_shines_turned_between_the_light_and_the_eye() {
        let between = (toward_light() + toward_eye()).normalize_or_zero();
        let aside = Vec3::new(-between.y, between.x, 0.0);
        let at = |normal, sharpness| polished(METAL, METAL_SHINE, normal, sharpness).lit();
        // Wholly the shine where it turns the light to the eye, and dimmer
        // turned aside.
        assert_eq!(at(between, 4.0), METAL_SHINE);
        assert!(at(aside, 4.0).r() < METAL_SHINE.r());
        // A narrow highlight only reaches the faces turned nearest: the same
        // face, half turned, shines broad or not at all.
        let half = (between + aside).normalize_or_zero();
        assert!(at(half, 30.0).r() < at(half, 2.0).r());
    }

    #[test]
    fn brushed_metal_streaks_across_its_grain() {
        let at = |tangent| brushed(METAL, METAL_SHINE, tangent, 6.0).lit();
        // The grain at right angles to both the light and the eye: the whole
        // streak. Along the light: none of it.
        let across = toward_light().cross(toward_eye());
        assert_eq!(at(across), METAL_SHINE);
        assert_eq!(at(toward_light()), METAL);
    }

    #[test]
    fn chrome_bands_from_the_sky_down_to_the_room() {
        let (top, bottom) = (chrome_at(0.0), chrome_at(1.0));
        assert_eq!(top, METAL_SHINE);
        assert_eq!(bottom, brighten(METAL, 0.6));
        // The light's band brighter than either side of it.
        assert!(chrome_at(0.55).r() > chrome_at(0.4).r());
        assert!(chrome_at(0.55).r() > chrome_at(0.7).r());
        assert_eq!(chrome_at(2.0), bottom, "past its foot it stays the room");
    }

    #[test]
    fn a_bars_lit_edge_turns_to_the_lamp() {
        // Away from the writer, toward them, square across, and upright:
        // every bar shows its writer-left side, squared off the bar where
        // it runs flat.
        let flat = [Vec3::Y, -Vec3::Y, Vec3::new(0.7, 0.7, 0.0), -Vec3::Z];
        for along in flat {
            let side = lit_side(along);
            assert!(side.x <= 0.0, "{along:?} lit side {side:?}");
            if Vec3::new(along.x, along.y, 0.0).length_squared() > 1e-6 {
                assert!(side.dot(along).abs() < 1e-3);
            }
        }
    }
}
