//! How a frame's solids take the light: [`Lighting`] — where the lamp
//! stands, the way to the eye, the room chrome mirrors, and [`Shadow`], the
//! lamp's own eye that decides what hides what — and each vertex's material
//! [`Shade`]. `Shade::apply` and `chrome` are the CPU twins of the maths
//! `depth.wgsl` runs per fragment: keep them in step, and a parse test below
//! keeps the shader itself readable.

use bytemuck::{Pod, Zeroable};
use eframe::egui::{Color32, lerp};
use glam::{Vec3, Vec4};

/// How many bands chrome's room has.
pub const CHROME_BANDS: usize = 5;
/// The uniform the shader reads: `r_lighting` in `depth.wgsl`.
pub(super) const LIGHTING_BYTES: u64 = std::mem::size_of::<LightingUniform>() as u64;

/// How a frame's lighting reaches the shader, laid out as `r_lighting`: the
/// lamp as `vec4` with its head's radius in `w`, the eye with the falloff's
/// reference distance in `w`, chrome's bands, then the shadow map's three
/// rows, and its parameters: bias, side, the penumbra's scale, and the map's
/// depth run in millimetres.
#[repr(C)]
#[derive(Debug, Copy, Clone, Pod, Zeroable)]
pub(super) struct LightingUniform {
    lamp: Vec4,
    eye: Vec4,
    bands: [Vec4; CHROME_BANDS],
    shadow: [Vec4; 3],
    shadow_params: Vec4,
}

/// How near the head its light may pool: the cap on the falloff's square,
/// so a fragment under the lamp does not burn out the whole desk.
const FALLOFF_MAX: f32 = 2.5;

/// The lamp's eye, casting the frame's shadows: an orthographic view along
/// the way to the lamp, whose depth is how far each millimetre stands off
/// it. What the shadow map holds, the shader and `rasterize` both read with
/// these rows.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Shadow {
    /// The rows taking absolute machine millimetres to the map: across and
    /// up it in `−1..=1`, and toward the lamp in `0..=1`; each row's `w` is
    /// its constant.
    pub rows: [Vec4; 3],
    /// The map's side in texels; 0 is a frame asking for no shadows.
    pub side: f32,
    /// How much nearer the lamp than its own texel a fragment must stand to
    /// still be held for lit, in `0..=1` of the map's whole run: enough to
    /// outface a texel's spread over the slanted face it was written from.
    pub bias: f32,
    /// Where the head's centre stands, in machine millimetres: how far a
    /// fragment is from the light, and so how wide its penumbra opens.
    pub lamp: Vec3,
    /// The head's radius in millimetres; 0 is the old point, with hard
    /// shadows the whole way.
    pub radius: f32,
    /// The map's depth run over its width, `span / (2 · half)`: what turns
    /// a depth difference into millimetres for the penumbra's reach.
    pub penumbra: f32,
    /// The map's depth run in millimetres, its rows' step along the lamp's
    /// ray: one unit of the map's `z` is this far. Rides to the shader as
    /// the shadow parameters' fourth word.
    pub span: f32,
}

/// How the shadow's edge is felt: the spiral of `PCF_TAPS` taps `depth.wgsl`
/// computes about each spot, tap for tap — equal-area rings at the golden
/// angle, spun by the same hash of the spot's texel.
#[cfg(test)]
const PCF_TAPS: usize = 16;
#[cfg(test)]
#[allow(clippy::excessive_precision)]
const GOLDEN_ANGLE: f32 = 2.39996323;
/// The spin hash's digits, verbatim from `depth.wgsl`.
#[cfg(test)]
#[allow(clippy::excessive_precision)]
const SPIN: [f32; 3] = [12.9898, 78.233, 43758.5453];

/// How far the disk reaches, in texels: `PCF_RADIUS`'s twin.
#[cfg(test)]
const PCF_RADIUS: f32 = 1.5;

/// How far a penumbra may open past the disk, in texels: `PENUMBRA_MAX`'s
/// twin, near the map's half-side. Past a shadow's own width the head is
/// wholly off the caster and the shadow has faded anyway; the bound only
/// keeps the disk from wandering the map's whole side for nothing.
#[cfg(test)]
const PENUMBRA_MAX: f32 = 96.0;

/// A unit across for a lamp whose `toward` leaves no room in `up`: the world
/// axis furthest from `toward` crossed with it, so the cross stands at least
/// √(2/3) long before normalising. A zero `toward` — no lamp at all — has no
/// across anywhere; its rows clamp as they ever did.
fn across_fallback(toward: Vec3) -> Vec3 {
    let axis = if toward.x.abs() <= toward.y.abs() && toward.x.abs() <= toward.z.abs() {
        Vec3::X
    } else if toward.y.abs() <= toward.z.abs() {
        Vec3::Y
    } else {
        Vec3::Z
    };
    let skew = toward.cross(axis);
    // `axis` is the one furthest from `toward`, so any real lamp leaves
    // this cross at least √(2/3) long: a short one is a bug in the pick,
    // not degenerate profile data.
    debug_assert!(
        skew.length_squared() > 0.5 || toward.length_squared() < 1e-12,
        "the fallback across degenerated for lamp {toward:?}"
    );
    skew.normalize_or_zero()
}

impl Shadow {
    /// The lamp's orthographic eye, looking from `lamp` toward the printing
    /// point along `up` the world's up: the scene lands on the square
    /// `center` ± `half` millimetres wide, its depth runs from `near`
    /// millimetres off the lamp over a `span`, and `bias_mm`, in the same
    /// millimetres, is held for the lamp's own. The map is `SHADOW_MAP` on
    /// every side; `rows` come out of this as `vs_lamp` projects and `spot`
    /// reads. The head's `radius`, in millimetres, is what opens each
    /// shadow's penumbra wider the further its fragment stands behind the
    /// caster hiding it.
    #[allow(clippy::too_many_arguments)]
    pub fn lamp(
        lamp: Vec3,
        radius: f32,
        up: Vec3,
        center: Vec3,
        half: f32,
        near: f32,
        span: f32,
        bias_mm: f32,
    ) -> Self {
        let toward = lamp.normalize_or_zero();
        // A profile can stand the lamp straight up or down, where `up` gives
        // no across; fall back on the world axis furthest from `toward` so
        // the basis stays orthonormal instead of clamping to one texel.
        let skew = toward.cross(up);
        let across = if skew.length_squared() < 1e-12 {
            across_fallback(toward)
        } else {
            skew.normalize_or_zero()
        };
        let up = across.cross(toward).normalize_or_zero();
        let axis = |v: Vec3| Vec4::new(v.x / half, v.y / half, v.z / half, -v.dot(center) / half);
        Shadow {
            rows: [
                axis(across),
                axis(up),
                Vec4::new(
                    -toward.x / span,
                    -toward.y / span,
                    -toward.z / span,
                    (toward.dot(lamp) - near) / span,
                ),
            ],
            side: super::SHADOW_MAP as f32,
            bias: bias_mm / span,
            lamp,
            radius,
            penumbra: span / (2.0 * half),
            span,
        }
    }

    /// Whether these shadows are cast at all.
    pub fn is_on(&self) -> bool {
        self.side > 0.0
    }

    /// How far the map's reading reaches along `x` and `y` across the
    /// plane `z = z_mm`, in machine millimetres: `(left, right, back,
    /// front)`. The map reads a prism along the way to the lamp and the
    /// plane cuts it in a parallelogram; these are its bounds, derived
    /// from the rows alone. A shadow catcher spanning them holds every
    /// shadow the map can cast onto the plane. `None` with no map, or a
    /// lamp lying in the plane, whose reach runs off without bound.
    #[cfg(test)]
    pub fn plane_reach(&self, z_mm: f32) -> Option<(f32, f32, f32, f32)> {
        use glam::{Mat2, Vec2};
        if !self.is_on() {
            return None;
        }
        let (a, b) = (self.rows[0], self.rows[1]);
        // `u = a·mm` and `v = b·mm` each hold over `−1..=1`: at the
        // plane's height that is two equations in `(x, y)`, and the
        // bounds of their solution lie at the map's four corners.
        let coefficients = Mat2::from_cols(Vec2::new(a.x, b.x), Vec2::new(a.y, b.y));
        if coefficients.determinant().abs() < 1e-9 {
            return None;
        }
        let inverse = coefficients.inverse();
        let corner = |(u, v)| {
            let rhs = Vec2::new(u - a.z * z_mm - a.w, v - b.z * z_mm - b.w);
            let solved = inverse * rhs;
            (solved.x, solved.y)
        };
        let corners = [
            corner((1.0, 1.0)),
            corner((1.0, -1.0)),
            corner((-1.0, 1.0)),
            corner((-1.0, -1.0)),
        ];
        let (mut left, mut right, mut back, mut front) =
            (corners[0].0, corners[0].0, corners[0].1, corners[0].1);
        for &(x, y) in corners[1..].iter() {
            left = left.min(x);
            right = right.max(x);
            back = back.min(y);
            front = front.max(y);
        }
        Some((left, right, back, front))
    }

    /// Where absolute machine millimetres `mm` fall in the map: its texel
    /// axes and its depth, `None` only with no map at all. Past the map's
    /// edge, and beyond its near and far, the reading clamps into it — a
    /// caster that reaches the edge keeps hiding what lies beyond — as
    /// `shadow_lit` does. The CPU twin's reading of the rows the shader
    /// projects with.
    #[cfg(test)]
    pub fn spot(&self, mm: Vec3) -> Option<[f32; 3]> {
        if !self.is_on() {
            return None;
        }
        let mm = mm.extend(1.0);
        let [u, v, d] = self.rows.map(|row| row.dot(mm));
        // NDC across a top-left origin: the shader's `textureLoad` texels
        // and the twin's rows are the same ones the lamp's pass wrote.
        Some([
            (u.clamp(-1.0, 1.0) + 1.0) * 0.5 * self.side,
            (1.0 - v.clamp(-1.0, 1.0)) * 0.5 * self.side,
            d.clamp(0.0, 1.0),
        ])
    }

    /// `mm`'s share of the lamp's light, from the map the lamp cast: the
    /// twin of `shadow_lit` in `depth.wgsl` — a spiral of map depths about
    /// the spot, its reach starting at `PCF_RADIUS`
    /// in texels and opening with the penumbra, the head's width thrown by
    /// how deep this fragment stands behind the caster hiding it, each
    /// depth blended by hand from the four texels about it, and each ramped
    /// from dark to light against the depth it stands at: wholly lit within
    /// [`Shadow::bias`] widened by how fast the map's own depth runs across
    /// the disk — a face slanted across the lamp's eye mislays its depth by
    /// just that much, and must not hide itself for it — darkening over the
    /// half-gap it stands in behind its caster, so the disk's answers blend
    /// continuously and the head's edge reads as one soft shade, not
    /// steps over each other. The caster's own texels, standing nearer the
    /// lamp, never widen a shadow's own bias.
    #[cfg(test)]
    pub fn occlusion(&self, map: &[f32], mm: Vec3) -> f32 {
        use typewriter_ui::draw::smoothstep;
        let Some([x, y, d]) = self.spot(mm) else {
            return 1.0;
        };
        let (side, last) = (self.side, self.side - 1.0);
        // The map's depth at continuous texel coordinates: blended by hand,
        // the twin of the GPU's filterable sampler.
        let depth = |xy: [f32; 2]| {
            let base = [xy[0] - 0.5, xy[1] - 0.5].map(f32::floor);
            let share = [xy[0] - 0.5 - base[0], xy[1] - 0.5 - base[1]];
            let texel = |column: f32, row: f32| {
                map[(row.clamp(0.0, last) * side + column.clamp(0.0, last)) as usize]
            };
            let corner = |dx: f32, dy: f32| texel(base[0] + dx, base[1] + dy);
            let across = |dy: f32| corner(0.0, dy) + (corner(1.0, dy) - corner(0.0, dy)) * share[0];
            across(0.0) + (across(1.0) - across(0.0)) * share[1]
        };
        let xy = [x, y];
        let centre = depth(xy);
        // The head's width thrown past the caster this fragment stands
        // behind, by similar triangles at the caster's surface: how wide
        // the head looms from there sets how far its rays slip past the
        // edge, the deeper behind it the fragment stands.
        let dist = (self.lamp - mm).length().max(1e-3);
        let deep = (d - centre).max(0.0);
        let span = self.span;
        let to_caster = (dist - deep * span).max(1e-3);
        let reach = PCF_RADIUS
            + (deep * self.penumbra * side * (self.radius / to_caster)).min(PENUMBRA_MAX);
        let spin = ((xy[0] * SPIN[0] + xy[1] * SPIN[1]).sin() * SPIN[2]).rem_euclid(1.0)
            * std::f32::consts::TAU;
        let mut stored = [0.0f32; PCF_TAPS];
        let mut slope: f32 = 0.0;
        for (i, there) in stored.iter_mut().enumerate() {
            let ring = ((i as f32 + 0.5) / PCF_TAPS as f32).sqrt() * reach;
            let angle = i as f32 * GOLDEN_ANGLE + spin;
            *there = depth([xy[0] + angle.cos() * ring, xy[1] + angle.sin() * ring]);
            if centre + self.bias >= d && *there + self.bias >= d {
                slope = slope.max((*there - centre).abs());
            }
        }
        let bias = self.bias + 2.0 * slope;
        // The fade's width: half the gap this fragment stands in behind its
        // caster, where the head's disk is wholly past the edge; only a
        // floor at the contact, which keeps its old hard answer.
        let soft = (deep * 0.5 + self.bias).max(1e-6);
        let edge = d - bias;
        // WGSL's `smoothstep`, twin for twin: dark a full `soft` behind the
        // bias'd depth, wholly lit at it, ramping between.
        let lit = |there: f32| smoothstep((there - edge + soft) / soft);
        let clear: f32 = stored.iter().map(|&there| lit(there)).sum();
        clear / stored.len() as f32
    }
}

/// A frame's lighting: where its lamp stands and how wide its head burns,
/// the directions its materials need, the room chrome mirrors and the way
/// its shadows are cast. Kept in step with `r_lighting` in `depth.wgsl`.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Lighting {
    /// Where the lamp's head stands, in machine millimetres from the
    /// printing point, facing back at it: each fragment is lit from there
    /// toward its own millimetres.
    pub lamp: Vec3,
    /// How wide the head burns, in millimetres: 0 lights as a bare point.
    pub lamp_radius: f32,
    /// Toward the eye, unit length: a highlight is where a surface turns them
    /// both.
    pub eye: Vec3,
    /// How far off the lamp a fragment stands for its light to read even:
    /// nearer burns brighter and further dips, toward the square of this.
    /// 0 is light the whole height of the room.
    pub falloff: f32,
    /// Chrome from its top (0) to its bottom (1), each band's colour and how
    /// far down it starts.
    pub chrome: [(f32, Color32); CHROME_BANDS],
    /// The lamp's eye: what it hides from what.
    pub shadow: Shadow,
}

impl Lighting {
    /// The uniform the shader reads.
    pub(super) fn uniform(&self) -> LightingUniform {
        let mut bands = [Vec4::ZERO; CHROME_BANDS];
        for (band, &(down, colour)) in bands.iter_mut().zip(&self.chrome) {
            let [r, g, b] = gamma_rgb(colour);
            *band = Vec4::new(down, r, g, b);
        }
        LightingUniform {
            lamp: self.lamp.extend(self.lamp_radius),
            eye: self.eye.extend(self.falloff),
            bands,
            shadow: self.shadow.rows,
            shadow_params: Vec4::new(
                self.shadow.bias,
                self.shadow.side,
                self.shadow.penumbra,
                self.shadow.span,
            ),
        }
    }
}

/// How a vertex takes the frame's light.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub enum Shade {
    /// Its colour as it is.
    #[default]
    Unlit,
    /// Plastic facing the unit `normal`: lit over an ambient floor (Lambert).
    Matte(Vec3),
    /// Metal polished to a `shine` facing the unit `normal`: lit over an
    /// ambient floor, then a highlight `sharpness` narrow (Blinn–Phong).
    Polished {
        normal: Vec3,
        shine: Color32,
        sharpness: f32,
    },
    /// Brushed metal running along the unit `tangent`: its colour turned to
    /// `shine` by the light streaked across the grain (Heidrich–Seidel),
    /// `sharpness` narrow.
    Streak {
        tangent: Vec3,
        shine: Color32,
        sharpness: f32,
    },
    /// Chrome, `t` of the way down: bright sky above, a band where it turns
    /// to the light, the dark room below. Its own colour is unused.
    Chrome(f32),
    /// What stands between the lamp and the light: its own colour and
    /// alpha, dimmed the deeper the shadow it is `strength` of — laid on a
    /// surface the pass does not draw, as the desk over its backdrop.
    Catcher(f32),
}

impl Shade {
    /// `rgba` (premultiplied sRGB gamma, 0..=1) standing at `at`, in machine
    /// millimetres, as `lighting`'s lamp lights `occlusion` of it: the
    /// lamp's own share reaching these millimetres, 1 where nothing hides
    /// it. Keep in step with `depth.wgsl`.
    pub fn apply(self, rgba: [f32; 4], lighting: &Lighting, at: Vec3, occlusion: f32) -> [f32; 4] {
        let [r, g, b, a] = rgba;
        let gamma = [r, g, b];
        // From these millimetres toward the head's centre: what a fragment
        // sees, its distance, and the two the head's width makes of it —
        // `ang`, how wide it looms, softening every term of the light, and
        // `fall`, the share of its lamp-light these millimetres stand in.
        let to_lamp = lighting.lamp - at;
        let dist = to_lamp.length().max(1e-3);
        let light = to_lamp / dist;
        let ang = (lighting.lamp_radius / dist).min(1.0);
        let wide = 1.0 + ang;
        let fall = if lighting.falloff > 0.0 {
            (lighting.falloff * lighting.falloff / (dist * dist)).clamp(0.0, FALLOFF_MAX)
        } else {
            1.0
        };
        let eye = lighting.eye;
        // Brightened `by`, then turned toward `shine` by `toward`: as the
        // shader mixes a lit colour with its highlight.
        let lit = |by: f32, shine: [f32; 3], toward: f32| {
            [0, 1, 2].map(|k| lerp((gamma[k] * by).min(1.0)..=shine[k], toward))
        };
        let shade = match self {
            Self::Unlit => return rgba,
            // The catcher is its own answer: the colour it lies out with,
            // shown only as deep as the shadow it stands in.
            Self::Catcher(strength) => {
                let by = strength * (1.0 - occlusion);
                return [r * by, g * by, b * by, a * by];
            }
            Self::Matte(normal) => {
                // A disc, not a point: its light wraps past the terminator.
                let facing =
                    ((normal.normalize_or_zero().dot(light) + ang) / wide).max(0.0) * occlusion;
                gamma.map(|c| (c * (0.7 + 0.4 * facing * fall)).min(1.0))
            }
            Self::Polished {
                normal,
                shine,
                sharpness,
            } => {
                let normal = normal.normalize_or_zero();
                let facing = ((normal.dot(light) + ang) / wide).max(0.0) * occlusion;
                // The head's width rides on the lobe: broadened by its
                // angular size, dimmed to keep the shine's energy. Both
                // fall out of `peak`, the lobe's share of what a point kept.
                let peak = 1.0 / (1.0 + ang * sharpness);
                let highlight = normal
                    .dot((light + eye).normalize_or_zero())
                    .max(0.0)
                    .powf(sharpness * peak)
                    * peak
                    * occlusion
                    * fall;
                lit(0.5 + 0.6 * facing * fall, gamma_rgb(shine), highlight)
            }
            Self::Streak {
                tangent,
                shine,
                sharpness,
            } => {
                let tangent = tangent.normalize_or_zero();
                let (lt, vt) = (tangent.dot(light), tangent.dot(eye));
                let across = (1.0 - lt * lt).max(0.0).sqrt() * (1.0 - vt * vt).max(0.0).sqrt();
                let peak = 1.0 / (1.0 + ang * sharpness);
                // Capped at 1: the mix must not overrun the premultiplied
                // shine it turns toward, pooled light included.
                let streak =
                    ((across - lt * vt).max(0.0).powf(sharpness * peak) * peak * fall).min(1.0);
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

    /// `colour` as [`Shade::apply`] lights it at `at`, in machine
    /// millimetres, wholly of the lamp's light: what lies this way is drawn
    /// outside the pass, where no shadow map is to be had.
    pub fn colour(self, colour: Color32, lighting: &Lighting, at: Vec3) -> Color32 {
        if self == Self::Unlit {
            return colour;
        }
        let rgba = colour.to_normalized_gamma_f32();
        // Safe cast: clamped to a byte.
        let [r, g, b, a] = self
            .apply(rgba, lighting, at, 1.0)
            .map(|c| (c.clamp(0.0, 1.0) * 255.0).round() as u8);
        Color32::from_rgba_premultiplied(r, g, b, a)
    }

    /// As the shader takes it: its material, the way it faces or runs along,
    /// its parameter and its highlight. The parameter is how narrow a
    /// highlight is, how far down a chrome plate lies, or how dark a
    /// catcher's shadow.
    pub(super) fn parts(self) -> (u32, Vec3, f32, Color32) {
        match self {
            Self::Unlit => (0, Vec3::ZERO, 0.0, Color32::WHITE),
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
            Self::Chrome(t) => (4, Vec3::ZERO, t, Color32::WHITE),
            Self::Catcher(strength) => (5, Vec3::ZERO, strength, Color32::WHITE),
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
            return [0, 1, 2].map(|k| lerp(upper[k]..=lower[k], across));
        }
    }
    gamma_rgb(bands[last].1)
}

/// `colour`'s rgb in gamma 0..=1.
fn gamma_rgb(colour: Color32) -> [f32; 3] {
    let [r, g, b, _] = colour.to_normalized_gamma_f32();
    [r, g, b]
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

    /// The uniform's bytes are what the shader reads: lamp and eye in the
    /// first words, their `w`s the head's radius and the falloff's reference,
    /// then the bands, `w` packed as the shader's `unpack_color`, then the
    /// shadow's rows and its parameters.
    #[test]
    fn the_uniform_words_match_the_shader() {
        assert_eq!(LIGHTING_BYTES, 11 * 16);
        let bytes = bytemuck::bytes_of(&above().uniform()).to_vec();
        let word = |k: usize| {
            let at = k * 16;
            [0, 1, 2, 3]
                .map(|i| f32::from_le_bytes(bytes[at + i * 4..at + i * 4 + 4].try_into().unwrap()))
        };
        assert_eq!(word(0), [0.0, 0.0, 10.0, 0.0]);
        assert_eq!(word(1), [0.0, 0.0, 1.0, 0.0]);
        assert_eq!(word(2), [0.0, 1.0, 1.0, 1.0]);
        assert_eq!(word(4), [0.5, 1.0, 1.0, 1.0]);
        assert_eq!(word(6), [1.0, 0.0, 0.0, 0.0]);
        // The shadow map's words, off: no rows, no side.
        assert_eq!(word(7), [0.0, 0.0, 0.0, 0.0]);
        assert_eq!(word(10), [0.0, 0.0, 0.0, 0.0]);
        // A head of a frame, its light pooling: the scalars ride the `w`s,
        // and the penumbra's scale the shadow parameters' third.
        let head = Lighting {
            lamp_radius: 80.0,
            falloff: 426.0,
            shadow: Shadow::lamp(
                Vec3::new(0.0, 0.0, 10.0),
                80.0,
                Vec3::Y,
                Vec3::ZERO,
                10.0,
                2.0,
                20.0,
                1.0,
            ),
            ..above()
        };
        let bytes = bytemuck::bytes_of(&head.uniform()).to_vec();
        let word = |k: usize| {
            let at = k * 16;
            [0, 1, 2, 3]
                .map(|i| f32::from_le_bytes(bytes[at + i * 4..at + i * 4 + 4].try_into().unwrap()))
        };
        assert_eq!(word(0)[3], 80.0);
        assert_eq!(word(1)[3], 426.0);
        assert_eq!(word(10)[2], 1.0, "span 20 over twice half 10");
        assert_eq!(word(10)[3], 20.0, "the map's depth run, in millimetres");
    }

    /// A frame with its lamp straight above, the eye above too, chrome from
    /// white at its top to black at its foot: a bare point, the old light.
    fn above() -> Lighting {
        let band = |grey: u8| Color32::from_rgb(grey, grey, grey);
        Lighting {
            lamp: Vec3::new(0.0, 0.0, 10.0),
            lamp_radius: 0.0,
            eye: Vec3::Z,
            falloff: 0.0,
            chrome: [
                (0.0, band(0xFF)),
                (0.25, band(0x80)),
                (0.5, band(0xFF)),
                (0.75, band(0x40)),
                (1.0, band(0x00)),
            ],
            shadow: Shadow::default(),
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
            .apply(grey, &above(), Vec3::ZERO, 1.0)
        };
        // Side-on to both: the ambient floor alone, half as bright.
        assert_eq!(polished(Vec3::X, 4.0), [0.25, 0.25, 0.25, 1.0]);
        // Facing them both: wholly the shine.
        assert!(polished(Vec3::Z, 4.0)[0] > 0.999);
        // A narrower highlight catches less of what turns aside.
        let aside = Vec3::new(0.0, 0.2, 1.0);
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
            .apply(grey, &above(), Vec3::ZERO, 1.0)[0]
        };
        // Across the light: the whole highlight. Along it: none at all.
        assert!(streak(Vec3::X) > 0.99);
        assert!((streak(Vec3::Z) - 0.5).abs() < 1e-3);
    }

    #[test]
    fn a_streaked_feather_turns_to_its_shine_premultiplied() {
        let shine = Color32::from_rgb(0xFF, 0xFF, 0xFF);
        let streaked = |rgba: [f32; 4]| {
            Shade::Streak {
                tangent: Vec3::X,
                shine,
                sharpness: 6.0,
            }
            .apply(rgba, &above(), Vec3::ZERO, 1.0)
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
        let chrome =
            |t| Shade::Chrome(t).apply([0.0, 0.0, 0.0, 1.0], &light, Vec3::ZERO, 1.0)[0] * 255.0;
        assert_eq!(chrome(0.0), 255.0);
        assert_eq!(chrome(0.5), 255.0);
        assert_eq!(chrome(1.0), 0.0);
        // Halfway between two bands.
        assert!((chrome(0.875) - 32.0).abs() < 0.01, "{:?}", chrome(0.875));
        // Past the bottom it stays the room's dark.
        assert_eq!(chrome(1.5), 0.0);
    }

    #[test]
    fn a_shadow_leaves_a_matte_face_its_ambient_and_a_chrome_its_room() {
        let grey = [0.5, 0.5, 0.5, 1.0];
        let light = above();
        // Facing the lamp from under: wholly lit, then wholly the shadow's.
        let matte = |occlusion| Shade::Matte(Vec3::Z).apply(grey, &light, Vec3::ZERO, occlusion)[0];
        assert!((matte(1.0) - 0.55).abs() < 1e-3, "1.1 times the colour");
        assert!((matte(0.0) - 0.35).abs() < 1e-3, "the 0.7 floor alone");
        // A polished face loses its highlight too.
        let shine = Color32::from_rgb(0xFF, 0xFF, 0xFF);
        let polished = |occlusion| {
            Shade::Polished {
                normal: Vec3::Z,
                shine,
                sharpness: 4.0,
            }
            .apply(grey, &light, Vec3::ZERO, occlusion)[0]
        };
        assert!(polished(1.0) > 0.999);
        assert!(polished(0.0) < 0.8, "still shine of the ambient");
        // What mirrors the room, and what takes no light, keep their look.
        let (chrome, unlit) = (
            Shade::Chrome(0.0).apply(grey, &light, Vec3::ZERO, 0.0),
            Shade::Unlit.apply(grey, &light, Vec3::ZERO, 0.0),
        );
        assert_eq!(chrome, [1.0, 1.0, 1.0, 1.0]);
        assert_eq!(unlit, grey);
    }

    #[test]
    fn a_catcher_shows_only_the_shadow_it_stands_in() {
        let black = [0.0, 0.0, 0.0, 1.0];
        let catcher =
            |occlusion| Shade::Catcher(0.4).apply(black, &above(), Vec3::ZERO, occlusion)[3];
        assert_eq!(catcher(1.0), 0.0);
        assert!((catcher(0.0) - 0.4).abs() < 1e-6);
        assert!((catcher(0.5) - 0.2).abs() < 1e-6);
    }

    /// Shadows straight down a lamp above: across and up map the x and y,
    /// and the depth runs from the lamp's `10` down over `20` millimetres.
    fn downward(shadow_side: f32) -> Shadow {
        let row = |x: f32| Vec4::new(x / 10.0, 0.0, 0.0, 0.0);
        Shadow {
            rows: [
                row(1.0),
                Vec4::new(0.0, 1.0 / 10.0, 0.0, 0.0),
                Vec4::new(0.0, 0.0, -1.0 / 20.0, 0.5),
            ],
            side: shadow_side,
            bias: 0.0,
            lamp: Vec3::new(0.0, 0.0, 10.0),
            radius: 0.0,
            penumbra: 1.0,
            span: 20.0,
        }
    }

    #[test]
    fn a_wide_head_wraps_light_past_the_terminator() {
        let grey = [0.5, 0.5, 0.5, 1.0];
        let head = Lighting {
            lamp_radius: 5.0,
            ..above()
        };
        let matte = |normal: Vec3, lighting: &Lighting| {
            Shade::Matte(normal).apply(grey, lighting, Vec3::ZERO, 1.0)[0]
        };
        // Turned aside from the point: the ambient floor alone.
        assert!((matte(Vec3::X, &above()) - 0.35).abs() < 1e-3);
        // A disc a half-lamp wide looms over the edge it used to stop at.
        assert!(matte(Vec3::X, &head) > 0.35);
        // Turned wholly away it keeps the floor, wrap included.
        assert!((matte(-Vec3::Z, &head) - 0.35).abs() < 1e-3);
    }

    #[test]
    fn a_wide_head_broadens_and_dims_the_highlight() {
        let grey = [0.5, 0.5, 0.5, 1.0];
        let shine = Color32::from_rgb(0xFF, 0xFF, 0xFF);
        let polished = |normal: Vec3, sharpness: f32, radius: f32| {
            let lighting = Lighting {
                lamp_radius: radius,
                ..above()
            };
            Shade::Polished {
                normal,
                shine,
                sharpness,
            }
            .apply(grey, &lighting, Vec3::ZERO, 1.0)[0]
        };
        // Facing them both: the point is wholly the shine, the head a share.
        assert!(polished(Vec3::Z, 4.0, 0.0) > 0.999);
        assert!(polished(Vec3::Z, 4.0, 5.0) < 0.95);
        // A face turned aside, where the narrow point goes dark, still
        // catches the broad lobe the head drew.
        let aside = Vec3::new(0.0, 0.5, 1.0);
        assert!(polished(aside, 24.0, 5.0) > polished(aside, 24.0, 0.0));
    }

    #[test]
    fn the_lamp_pools_brighter_near_the_head_and_thins_away() {
        let grey = [0.5, 0.5, 0.5, 1.0];
        let lit_at = |z: f32, falloff: f32| {
            let lighting = Lighting { falloff, ..above() };
            Shade::Matte(Vec3::Z).apply(grey, &lighting, Vec3::new(0.0, 0.0, z), 1.0)[0]
        };
        // Of no reference: the whole room lit alike, as ever.
        assert!((lit_at(-10.0, 0.0) - 0.55).abs() < 1e-3);
        // At the reference, even; twice its way off, a quarter of the
        // lamp's share rides over the floor.
        assert!((lit_at(0.0, 10.0) - 0.55).abs() < 1e-3);
        assert!((lit_at(-10.0, 10.0) - 0.4).abs() < 1e-3);
        // Right under the head the pool caps rather than burns out.
        assert!((lit_at(8.0, 10.0) - 0.5 * (0.7 + 0.4 * FALLOFF_MAX)).abs() < 1e-3);
    }

    #[test]
    fn the_lamp_reads_a_hidden_spot_and_its_edge_rules_beyond_the_map() {
        let shadow = downward(16.0);
        // Nothing cast anywhere: the cleared map, and every spot its own.
        let clear = vec![1.0; 256];
        assert_eq!(shadow.occlusion(&clear, Vec3::ZERO), 1.0);
        // A caster nearer the lamp than the fragment hides it: the map's
        // lower rows hold depth 0.25, a millimetre-run of 5 above, where
        // the fragment at the origin stands at 0.5.
        let mut cast = vec![1.0; 256];
        for row in 4..16 {
            for column in 0..16 {
                cast[row * 16 + column] = 0.25;
            }
        }
        let spot = shadow.spot(Vec3::ZERO).unwrap();
        assert_eq!(spot, [8.0, 8.0, 0.5]);
        // Wholly under the caster, its whole disk hidden: nothing clear.
        assert_eq!(shadow.occlusion(&cast, Vec3::new(0.0, -5.0, 0.0)), 0.0);
        // Straddling the caster's edge, the disk reads both sides: a share
        // of the lamp's light, continuous across the map instead of the
        // one hard jump a per-texel snap would give.
        let across = shadow.occlusion(&cast, Vec3::new(0.0, 4.375, 0.0));
        assert!(across > 0.0 && across < 1.0, "{across}");
        // A fragment standing on a face whose map depth races — here to a
        // caster a mere 1 mm nearer the lamp — is held wholly lit by the
        // widened bias: slanted paper never stripes itself.
        let mut racing = vec![1.0; 256];
        for row in 8..16 {
            for column in 0..16 {
                racing[row * 16 + column] = 0.45;
            }
        }
        assert_eq!(shadow.occlusion(&racing, Vec3::ZERO), 1.0);
        // Past the map's edge, its last readings rule: a caster that
        // reaches the edge keeps hiding what lies beyond it, and an edge
        // the lamp found clear holds its shadow off.
        assert_eq!(
            shadow.spot(Vec3::new(0.0, -30.0, 0.0)),
            Some([8.0, 16.0, 0.5])
        );
        assert_eq!(shadow.occlusion(&cast, Vec3::new(0.0, -30.0, 0.0)), 0.0);
        assert_eq!(shadow.occlusion(&cast, Vec3::new(0.0, 30.0, 0.0)), 1.0);
        // Beyond the map's far, the casters it holds still hide the spot;
        // nearer than its near, nothing can.
        assert_eq!(shadow.occlusion(&cast, Vec3::new(0.0, 0.0, -20.0)), 0.0);
        assert_eq!(shadow.occlusion(&cast, Vec3::new(0.0, 0.0, 20.0)), 1.0);
        // Off entirely: no spot, no shade.
        assert_eq!(Shadow::default().occlusion(&[], Vec3::ZERO), 1.0);
    }

    #[test]
    fn a_head_opens_shadows_softer_with_size_and_depth() {
        // The downward map, its lamp a head of `radius` millimetres.
        let head = |radius: f32| Shadow {
            radius,
            ..downward(16.0)
        };
        let mut cast = vec![1.0; 256];
        for row in 4..16 {
            for column in 0..16 {
                cast[row * 16 + column] = 0.25;
            }
        }
        // A fragment a little under the caster's edge: the point's disk
        // falls wholly inside the shadow, as the old hard edge had it.
        let at = Vec3::new(0.0, 2.5, 0.0);
        assert_eq!(head(0.0).occlusion(&cast, at), 0.0);
        // A head half the lamp's way off reaches past the edge from this
        // far behind: soft, and short of the light.
        let narrow = head(5.0).occlusion(&cast, at);
        assert!(narrow > 0.0 && narrow < 1.0, "{narrow}");
        // Deeper behind the same caster, the same head looms wider from
        // its surface: its rays slip further past the edge, and the shade
        // thins as it goes.
        let deeper = head(5.0).occlusion(&cast, Vec3::new(0.0, 2.5, -5.0));
        assert!(deeper > narrow, "{deeper} against {narrow}");
        // The wider the head, the further its rays slip past the caster:
        // the shade thins as its width grows.
        let wide = head(8.0).occlusion(&cast, at);
        assert!(wide > narrow, "{wide} against {narrow}");
        // And the reach caps: a fragment the map's whole run behind the
        // caster, under a head wider than the lamp's way off, blurs no
        // further than the disk's bound — and still keeps some shade.
        let buried = Vec3::new(0.0, 2.5, -19.0);
        assert!(head(400.0).occlusion(&cast, buried) < 1.0, "still a shadow");
    }

    /// A profile standing the lamp straight up or down — `toward` parallel to
    /// `up` — must not collapse the map's basis to zero rows clamped into one
    /// texel: the fallback keeps the rows unit and mutually square.
    #[test]
    fn an_overhead_lamp_falls_back_on_a_stable_across() {
        for lamp in [Vec3::new(0.0, 0.0, 10.0), Vec3::new(0.0, 0.0, -10.0)] {
            let shadow = Shadow::lamp(lamp, 0.0, Vec3::Z, Vec3::ZERO, 10.0, 2.0, 20.0, 1.0);
            let across = shadow.rows[0].truncate() * 10.0;
            let up = shadow.rows[1].truncate() * 10.0;
            let toward = -shadow.rows[2].truncate() * 20.0;
            for row in [across, up, toward] {
                assert!((row.length() - 1.0).abs() < 1e-6, "{row:?}");
            }
            assert!(across.dot(up).abs() < 1e-6);
            assert!(across.dot(toward).abs() < 1e-6);
            assert!(up.dot(toward).abs() < 1e-6);
            assert_eq!(
                shadow,
                Shadow::lamp(lamp, 0.0, Vec3::Z, Vec3::ZERO, 10.0, 2.0, 20.0, 1.0),
                "the fallback picks a fixed axis, not rounding noise"
            );
        }
    }

    /// The ordinary path is untouched: a lamp an `up` can span keeps the
    /// exact rows it always had, down to the bit.
    #[test]
    fn a_side_lamp_keeps_its_exact_rows() {
        let shadow = Shadow::lamp(
            Vec3::new(0.0, 0.0, 10.0),
            0.0,
            Vec3::Y,
            Vec3::ZERO,
            10.0,
            2.0,
            20.0,
            1.0,
        );
        assert_eq!(shadow.rows[0], Vec4::new(-1.0 / 10.0, 0.0, 0.0, 0.0));
        assert_eq!(shadow.rows[1], Vec4::new(0.0, 1.0 / 10.0, 0.0, 0.0));
        assert_eq!(shadow.rows[2], Vec4::new(0.0, 0.0, -1.0 / 20.0, 8.0 / 20.0));
    }

    /// The map reads a prism along the way to the lamp: a plane cuts it
    /// in the map's square turned that way, and `plane_reach` bounds it.
    #[test]
    fn the_maps_reach_across_a_plane_is_where_it_reads() {
        let near = |(left, right, back, front): (f32, f32, f32, f32), wanted: [f32; 4]| {
            for (got, want) in [left, right, back, front].into_iter().zip(wanted) {
                assert!((got - want).abs() < 1e-4, "{got} against {want}");
            }
        };
        // Straight down, axes square to the world: every plane cuts the
        // map's own square, whatever its height.
        let shadow = downward(16.0);
        near(shadow.plane_reach(0.0).unwrap(), [-10.0, 10.0, -10.0, 10.0]);
        near(
            shadow.plane_reach(-7.0).unwrap(),
            [-10.0, 10.0, -10.0, 10.0],
        );
        // A lamp at 45 degrees: the prism's slant opens the cut along
        // the stay the light runs beside, and leaves the other as it is.
        let slanted = Shadow::lamp(
            Vec3::new(0.0, 10.0, 10.0),
            0.0,
            Vec3::Z,
            Vec3::ZERO,
            10.0,
            2.0,
            20.0,
            1.0,
        );
        let open = 10.0 * std::f32::consts::SQRT_2;
        near(
            slanted.plane_reach(0.0).unwrap(),
            [-10.0, 10.0, -open, open],
        );
        // A lamp lying in the plane casts along it without bound: no
        // finite reach to answer with. And no map at all, none either.
        let flat = Shadow::lamp(
            Vec3::new(10.0, 0.0, 0.0),
            0.0,
            Vec3::Z,
            Vec3::ZERO,
            10.0,
            2.0,
            20.0,
            1.0,
        );
        assert_eq!(flat.plane_reach(0.0), None);
        assert_eq!(Shadow::default().plane_reach(0.0), None);
    }
}
