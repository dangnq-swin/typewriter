//! How a frame's solids take the light: [`Lighting`] — where the lamp
//! stands, the way to the eye, the room chrome mirrors — and each vertex's
//! material [`Shade`]. `Shade::apply` and `chrome` are the CPU twins of the
//! maths `depth.wgsl` runs per fragment: keep them in step, and a parse
//! test below keeps the shader itself readable.

use eframe::egui::Color32;

/// How many bands chrome's room has.
pub const CHROME_BANDS: usize = 5;
/// The lamp and the eye as `vec4`s, then chrome's bands as `vec4`s: as many
/// as the shader's `r_lighting`.
pub(super) const LIGHTING_BYTES: u64 = 7 * 16;

/// A frame's lighting: where its lamp stands, the directions its materials
/// need and the room chrome mirrors. Kept in step with `r_lighting` in
/// `depth.wgsl`.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Lighting {
    /// Where the lamp stands, in machine millimetres from the printing
    /// point: each fragment is lit from there toward its own millimetres.
    pub lamp: [f32; 3],
    /// Toward the eye, unit length: a highlight is where a surface turns them
    /// both.
    pub eye: [f32; 3],
    /// Chrome from its top (0) to its bottom (1), each band's colour and how
    /// far down it starts.
    pub chrome: [(f32, Color32); CHROME_BANDS],
}

impl Lighting {
    /// The uniform the shader reads: the lamp, the eye, then the bands.
    pub(super) fn uniform(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(LIGHTING_BYTES as usize);
        let mut word = |value: [f32; 4]| {
            for part in value {
                out.extend_from_slice(&part.to_le_bytes());
            }
        };
        word([self.lamp[0], self.lamp[1], self.lamp[2], 0.0]);
        word([self.eye[0], self.eye[1], self.eye[2], 0.0]);
        for &(down, colour) in &self.chrome {
            let [r, g, b] = gamma_rgb(colour);
            word([down, r, g, b]);
        }
        out
    }
}

/// How a vertex takes the frame's light.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub enum Shade {
    /// Its colour as it is.
    #[default]
    Unlit,
    /// Plastic facing the unit `normal`: lit over an ambient floor (Lambert).
    Matte([f32; 3]),
    /// Metal polished to a `shine` facing the unit `normal`: lit over an
    /// ambient floor, then a highlight `sharpness` narrow (Blinn–Phong).
    Polished {
        normal: [f32; 3],
        shine: Color32,
        sharpness: f32,
    },
    /// Brushed metal running along the unit `tangent`: its colour turned to
    /// `shine` by the light streaked across the grain (Heidrich–Seidel),
    /// `sharpness` narrow.
    Streak {
        tangent: [f32; 3],
        shine: Color32,
        sharpness: f32,
    },
    /// Chrome, `t` of the way down: bright sky above, a band where it turns
    /// to the light, the dark room below. Its own colour is unused.
    Chrome(f32),
}

impl Shade {
    /// `rgba` (premultiplied sRGB gamma, 0..=1) standing at `at`, in machine
    /// millimetres, as `lighting`'s lamp lights it. Keep in step with `depth.wgsl`.
    pub fn apply(self, rgba: [f32; 4], lighting: &Lighting, at: [f32; 3]) -> [f32; 4] {
        let [r, g, b, a] = rgba;
        let gamma = [r, g, b];
        // From these millimetres toward the lamp: what a fragment sees, the
        // direction from it to the light.
        let light = unit(sub(lighting.lamp, at));
        let eye = lighting.eye;
        // Brightened `by`, then turned toward `shine` by `toward`: as the
        // shader mixes a lit colour with its highlight.
        let lit = |by: f32, shine: [f32; 3], toward: f32| {
            [0, 1, 2].map(|k| {
                let base = (gamma[k] * by).min(1.0);
                base + (shine[k] - base) * toward
            })
        };
        let shade = match self {
            Self::Unlit => return rgba,
            Self::Matte(normal) => {
                let facing = dot(unit(normal), light).max(0.0);
                gamma.map(|c| (c * (0.7 + 0.4 * facing)).min(1.0))
            }
            Self::Polished {
                normal,
                shine,
                sharpness,
            } => {
                let normal = unit(normal);
                let facing = dot(normal, light).max(0.0);
                let highlight = dot(normal, unit(add(light, eye))).max(0.0).powf(sharpness);
                lit(0.5 + 0.6 * facing, gamma_rgb(shine), highlight)
            }
            Self::Streak {
                tangent,
                shine,
                sharpness,
            } => {
                let tangent = unit(tangent);
                let (lt, vt) = (dot(tangent, light), dot(tangent, eye));
                let across = (1.0 - lt * lt).max(0.0).sqrt() * (1.0 - vt * vt).max(0.0).sqrt();
                let streak = (across - lt * vt).max(0.0).powf(sharpness);
                // Toward `shine` premultiplied, as the shader: a feathered
                // decal's rgb is premultiplied, and the raw colour would
                // make its clear halo shine.
                let toward = gamma_rgb(shine).map(|c| c * a);
                lit(1.0, toward, streak)
            }
            Self::Chrome(t) => chrome(&lighting.chrome, t),
        };
        let [r, g, b] = shade;
        [r, g, b, a]
    }

    /// `colour` as [`Shade::apply`] lights it at `at`, in machine millimetres.
    pub fn colour(self, colour: Color32, lighting: &Lighting, at: [f32; 3]) -> Color32 {
        if self == Self::Unlit {
            return colour;
        }
        let rgba = colour.to_array().map(|c| f32::from(c) / 255.0);
        // Safe cast: clamped to a byte.
        let [r, g, b, a] = self
            .apply(rgba, lighting, at)
            .map(|c| (c.clamp(0.0, 1.0) * 255.0).round() as u8);
        Color32::from_rgba_premultiplied(r, g, b, a)
    }

    /// As the shader takes it: its material, the way it faces or runs along,
    /// its parameter and its highlight. The parameter is how narrow a
    /// highlight is, or how far down a chrome plate lies.
    pub(super) fn parts(self) -> (u32, [f32; 3], f32, Color32) {
        match self {
            Self::Unlit => (0, [0.0; 3], 0.0, Color32::WHITE),
            Self::Matte(normal) => (1, normal, 0.0, Color32::WHITE),
            Self::Polished {
                normal,
                shine,
                sharpness,
            } => (2, normal, sharpness, shine),
            Self::Streak {
                tangent,
                shine,
                sharpness,
            } => (3, tangent, sharpness, shine),
            Self::Chrome(t) => (4, [0.0; 3], t, Color32::WHITE),
        }
    }
}

/// Chrome `t` of the way down `bands`, in gamma 0..=1. Keep in step with
/// `chrome` in `depth.wgsl`.
fn chrome(bands: &[(f32, Color32); CHROME_BANDS], t: f32) -> [f32; 3] {
    let at = t.clamp(0.0, 1.0);
    let last = bands.len() - 1;
    for i in 0..last {
        let (from, to) = (bands[i], bands[i + 1]);
        if at <= to.0 {
            let across = (at - from.0) / (to.0 - from.0);
            let (upper, lower) = (gamma_rgb(from.1), gamma_rgb(to.1));
            return [0, 1, 2].map(|k| upper[k] + (lower[k] - upper[k]) * across);
        }
    }
    gamma_rgb(bands[last].1)
}

/// `colour`'s rgb in gamma 0..=1.
fn gamma_rgb(colour: Color32) -> [f32; 3] {
    let [r, g, b, _] = colour.to_array();
    [r, g, b].map(|c| f32::from(c) / 255.0)
}

fn dot(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn add(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

fn sub(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

pub(super) fn unit(v: [f32; 3]) -> [f32; 3] {
    let length = dot(v, v).sqrt().max(1e-6);
    v.map(|c| c / length)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The pass's shader, parsed and validated: the twins above, and the
    /// packing below in `gpu`, only mean anything while it compiles.
    #[test]
    fn the_shader_is_valid() {
        use eframe::wgpu::naga;

        use crate::depth::gpu::SHADER;
        let module = naga::front::wgsl::parse_str(SHADER).unwrap();
        let mut validator = naga::valid::Validator::new(
            naga::valid::ValidationFlags::all(),
            naga::valid::Capabilities::empty(),
        );
        validator.validate(&module).unwrap();
    }

    /// A frame with its lamp straight above, the eye above too, chrome from
    /// white at its top to black at its foot.
    fn above() -> Lighting {
        let band = |grey: u8| Color32::from_rgb(grey, grey, grey);
        Lighting {
            lamp: [0.0, 0.0, 10.0],
            eye: [0.0, 0.0, 1.0],
            chrome: [
                (0.0, band(0xFF)),
                (0.25, band(0x80)),
                (0.5, band(0xFF)),
                (0.75, band(0x40)),
                (1.0, band(0x00)),
            ],
        }
    }

    #[test]
    fn polished_metal_shines_where_it_turns_the_light_to_the_eye() {
        let grey = [0.5, 0.5, 0.5, 1.0];
        let shine = Color32::from_rgb(0xFF, 0xFF, 0xFF);
        let polished = |normal, sharpness| {
            Shade::Polished {
                normal,
                shine,
                sharpness,
            }
            .apply(grey, &above(), [0.0; 3])
        };
        // Side-on to both: the ambient floor alone, half as bright.
        assert_eq!(polished([1.0, 0.0, 0.0], 4.0), [0.25, 0.25, 0.25, 1.0]);
        // Facing them both: wholly the shine.
        assert!(polished([0.0, 0.0, 1.0], 4.0)[0] > 0.999);
        // A narrower highlight catches less of what turns aside.
        let aside = [0.0, 0.2, 1.0];
        assert!(polished(aside, 24.0)[0] < polished(aside, 4.0)[0]);
    }

    #[test]
    fn brushed_metal_streaks_across_its_grain() {
        let grey = [0.5, 0.5, 0.5, 1.0];
        let shine = Color32::from_rgb(0xFF, 0xFF, 0xFF);
        let streak = |tangent| {
            Shade::Streak {
                tangent,
                shine,
                sharpness: 6.0,
            }
            .apply(grey, &above(), [0.0; 3])[0]
        };
        // Across the light: the whole highlight. Along it: none at all.
        assert!(streak([1.0, 0.0, 0.0]) > 0.99);
        assert!((streak([0.0, 0.0, 1.0]) - 0.5).abs() < 1e-3);
    }

    #[test]
    fn a_streaked_feather_turns_to_its_shine_premultiplied() {
        let shine = Color32::from_rgb(0xFF, 0xFF, 0xFF);
        let streaked = |rgba: [f32; 4]| {
            Shade::Streak {
                tangent: [1.0, 0.0, 0.0],
                shine,
                sharpness: 6.0,
            }
            .apply(rgba, &above(), [0.0; 3])
        };
        // The half-covered edge of a feather, its rgb premultiplied, where
        // the whole highlight falls: it stays under its alpha, so the halo
        // does not shine.
        let feather = streaked([0.25, 0.25, 0.25, 0.5]);
        assert!(feather[0] <= 0.5 + 1e-3);
        // Opaque, it reaches the whole shine as ever.
        assert!(streaked([0.5, 0.5, 0.5, 1.0])[0] > 0.99);
    }

    #[test]
    fn chrome_reads_the_room_down_its_plate() {
        let light = above();
        let chrome = |t| Shade::Chrome(t).apply([0.0, 0.0, 0.0, 1.0], &light, [0.0; 3])[0] * 255.0;
        assert_eq!(chrome(0.0), 255.0);
        assert_eq!(chrome(0.5), 255.0);
        assert_eq!(chrome(1.0), 0.0);
        // Halfway between two bands.
        assert!((chrome(0.875) - 32.0).abs() < 0.01, "{:?}", chrome(0.875));
        // Past the bottom it stays the room's dark.
        assert_eq!(chrome(1.5), 0.0);
    }
}
