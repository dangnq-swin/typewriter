//! The desk's lamp, where a writer would place one: to the left, above and a
//! little in front, its head out over the keys and clear of the work — usual
//! enough to be believed, far enough never to be in the way. It burns the
//! room's own white: what it lights is what is there, brighter where it
//! turns. The shader lights what stands in depth from it, per fragment;
//! [`Paint::lit`] is its twin at the machine's middle for what is drawn flat
//! or as a line.

use eframe::egui::Color32;

use super::canvas::Canvas;
use super::eye::{Eye, toward_eye};
use super::geometry::{dot, normalized};
use super::{METAL, METAL_SHINE};
use crate::depth::{CHROME_BANDS, Layer, Lighting, Shade, Solid};

/// Where the lamp stands, in machine millimetres from the printing point:
/// over the desk's left front, its head about 400 mm above the desk's
/// top. Seen from the machine's middle it lies where the old direction
/// pointed, so what the CPU lights keeps its look.
pub(super) fn lamp() -> [f32; 3] {
    [-254.0, 170.0, 297.0]
}

/// Toward the lamp from the machine's middle: for lines and cast shadows,
/// which stand nowhere of their own. The shader reaches each fragment's own
/// direction from [`lamp`] instead.
pub(super) fn toward_light() -> [f32; 3] {
    normalized(lamp())
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
    /// Lit at the machine's middle, for what takes no shade in the shader:
    /// lines, and flat drawing.
    pub(super) fn lit(self) -> Color32 {
        self.shade.colour(self.colour, &frame(), [0.0; 3])
    }
}

/// Plastic facing `normal`: lit over an ambient floor (Lambert), per pixel.
pub(super) fn matte(colour: Color32, normal: [f32; 3]) -> Paint {
    Paint {
        colour,
        shade: Shade::Matte(normalized(normal)),
    }
}

/// Metal facing `normal`, polished to a `shine` whose highlight `sharpness`
/// narrows: lit per pixel.
pub(super) fn polished(colour: Color32, shine: Color32, normal: [f32; 3], sharpness: f32) -> Paint {
    Paint {
        colour,
        shade: Shade::Polished {
            normal: normalized(normal),
            shine,
            sharpness,
        },
    }
}

/// Brushed metal running along `tangent`, streaked with `shine` across its
/// grain, `sharpness` narrow: lit per pixel.
pub(super) fn brushed(colour: Color32, shine: Color32, tangent: [f32; 3], sharpness: f32) -> Paint {
    Paint {
        colour,
        shade: Shade::Streak {
            tangent: normalized(tangent),
            shine,
            sharpness,
        },
    }
}

/// How bright a thin metal part running along `tangent` catches the light,
/// 0..=1: brushed and milled metal streaks along its grain (Heidrich–Seidel).
/// `sharpness` narrows the streak. For lines, which take no shade: what
/// stands in depth is [`brushed`] instead.
pub(super) fn streak(tangent: [f32; 3], sharpness: i32) -> f32 {
    let tangent = normalized(tangent);
    let (lt, vt) = (dot(toward_light(), tangent), dot(toward_eye(), tangent));
    let across = (1.0 - lt * lt).max(0.0).sqrt() * (1.0 - vt * vt).max(0.0).sqrt();
    (across - lt * vt).max(0.0).powi(sharpness)
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
        [left, y, top],
        [right, y, top],
        [right, y, bottom],
        [left, y, bottom],
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
    Shade::Chrome(t).colour(Color32::WHITE, &frame(), [0.0; 3])
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
    use super::super::geometry::{add, cross};
    use super::*;

    #[test]
    fn one_light_from_the_front_left() {
        let lit = |normal| matte(IVORY, normal).lit();
        let (left, right) = (lit([-1.0, 0.0, 0.0]), lit([1.0, 0.0, 0.0]));
        assert!(left.r() > right.r());
        assert!(lit([0.0, 0.0, 1.0]).r() > lit([0.0, 0.0, -1.0]).r());
        for angle in 0..36 {
            let turn = (10.0 * angle as f32).to_radians();
            let shine = streak([turn.cos(), turn.sin(), 0.0], 6);
            assert!((0.0..=1.0).contains(&shine));
        }
    }

    #[test]
    fn polished_metal_shines_turned_between_the_light_and_the_eye() {
        let between = normalized(add(toward_light(), toward_eye()));
        let aside = [-between[1], between[0], 0.0];
        let at = |normal, sharpness| polished(METAL, METAL_SHINE, normal, sharpness).lit();
        // Wholly the shine where it turns the light to the eye, and dimmer
        // turned aside.
        assert_eq!(at(between, 4.0), METAL_SHINE);
        assert!(at(aside, 4.0).r() < METAL_SHINE.r());
        // A narrow highlight only reaches the faces turned nearest: the same
        // face, half turned, shines broad or not at all.
        let half = normalized(add(between, aside));
        assert!(at(half, 30.0).r() < at(half, 2.0).r());
    }

    #[test]
    fn brushed_metal_streaks_across_its_grain() {
        let at = |tangent| brushed(METAL, METAL_SHINE, tangent, 6.0).lit();
        // The grain at right angles to both the light and the eye: the whole
        // streak. Along the light: none of it.
        let across = cross(toward_light(), toward_eye());
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
}
