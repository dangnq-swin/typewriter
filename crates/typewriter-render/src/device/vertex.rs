//! The GPU's vertex: one [`Vertex`] in machine millimetres, exactly as
//! `vs_main` in `scene.wgsl` reads it, and the [`Shade`] that packs how it
//! takes the frame's light into its material words.

use bytemuck::{Pod, Zeroable};
use glam::Vec3;

/// One vertex, exactly as `vs_main` in `scene.wgsl` reads it: the
/// millimetres it stands at in its mesh's own space, uv, colour, the way
/// it faces, its material's parameter, material and highlight. The
/// placement's transform carries `at` into the frame's absolute
/// millimetres, and the shader projects those. The compiler keeps its size
/// and order true to the pipeline's `vertex_attr_array` and the shader;
/// little-endian bytes, like both backends read.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Pod, Zeroable)]
pub struct Vertex {
    /// The mesh's own millimetres, from its origin.
    pub at: Vec3,
    pub uv: [f32; 2],
    /// Premultiplied sRGB gamma.
    pub color: [u8; 4],
    /// What the vertex faces (matte, polished) or runs along (streak),
    /// in the mesh's own millimetres.
    pub vector: Vec3,
    pub spec: f32,
    pub material: u32,
    /// The material's highlight, packed like `color`: a part shines one
    /// colour all over.
    pub shine: [u8; 4],
}

/// The vertices' stride, in bytes, as the pipelines and the uploads read
/// it.
pub const VERTEX_BYTES: u64 = std::mem::size_of::<Vertex>() as u64;

impl Vertex {
    /// A vertex standing at `at`, its uvs `uv`, coloured `color`
    /// (premultiplied sRGB gamma), taking the frame's light as `shade`.
    pub fn placed(at: Vec3, uv: [f32; 2], color: [u8; 4], shade: Shade) -> Self {
        let (material, vector, spec, shine) = shade.parts();
        Self {
            at,
            uv,
            color,
            vector,
            spec,
            material,
            shine,
        }
    }
}

/// How a vertex takes the frame's light, as the shader's materials number
/// them: the packing half of the old `Shade`, whose lighting maths stays
/// in `typewriter`'s `depth::lighting` until T3 lands it here.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub enum Shade {
    /// Its colour as it is.
    #[default]
    Unlit,
    /// Plastic facing the unit `normal`: lit over an ambient floor.
    Matte(Vec3),
    /// Metal polished to a `shine` facing the unit `normal`: lit, then a
    /// highlight `sharpness` narrow.
    Polished {
        normal: Vec3,
        shine: [u8; 4],
        sharpness: f32,
    },
    /// Brushed metal running along the unit `tangent`: its colour turned
    /// to `shine` by the light streaked across the grain, `sharpness`
    /// narrow.
    Streak {
        tangent: Vec3,
        shine: [u8; 4],
        sharpness: f32,
    },
    /// Chrome, `t` of the way down. Its own colour is unused.
    Chrome(f32),
    /// What stands between the lamp and the light: its own colour and
    /// alpha, dimmed the deeper the shadow it is `strength` of.
    Catcher(f32),
}

impl Shade {
    /// As the shader takes it: its material, the way it faces or runs
    /// along, its parameter and its highlight. The parameter is how narrow
    /// a highlight is, how far down a chrome plate lies, or how dark a
    /// catcher's shadow.
    fn parts(self) -> (u32, Vec3, f32, [u8; 4]) {
        const WHITE: [u8; 4] = [0xFF; 4];
        match self {
            Self::Unlit => (0, Vec3::ZERO, 0.0, WHITE),
            Self::Matte(normal) => (1, normal, 0.0, WHITE),
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
            Self::Chrome(t) => (4, Vec3::ZERO, t, WHITE),
            Self::Catcher(strength) => (5, Vec3::ZERO, strength, WHITE),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The vertex's bytes are what the pipeline's attributes and the
    /// shader's locations read: each field where its `location` says, in
    /// the little endianness both backends read.
    #[test]
    fn a_vertex_is_the_bytes_the_shader_reads() {
        assert_eq!(VERTEX_BYTES, 48);
        let vertex = Vertex::placed(
            Vec3::new(1.0, 2.0, 3.0),
            [4.0, 5.0],
            [0x10, 0x20, 0x30, 0x40],
            Shade::Polished {
                normal: Vec3::new(6.0, 7.0, 8.0),
                shine: [0x50, 0x60, 0x70, 0x80],
                sharpness: 9.0,
            },
        );
        let bytes = bytemuck::bytes_of(&vertex);
        // `location` 0..6, at offsets 0, 12, 20, 24, 36, 40, 44.
        let f32_at = |at: usize| f32::from_le_bytes(bytes[at..at + 4].try_into().unwrap());
        let u32_at = |at: usize| u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap());
        assert_eq!((f32_at(0), f32_at(4), f32_at(8)), (1.0, 2.0, 3.0));
        assert_eq!((f32_at(12), f32_at(16)), (4.0, 5.0));
        assert_eq!(u32_at(20), 0x40302010);
        assert_eq!((f32_at(24), f32_at(28), f32_at(32)), (6.0, 7.0, 8.0));
        assert_eq!(f32_at(36), 9.0);
        assert_eq!(u32_at(40), 2, "polished's material");
        assert_eq!(u32_at(44), 0x80706050);
    }

    /// Each shade rides down as its material's number, vector, parameter
    /// and highlight: what the shader's `switch` reads.
    #[test]
    fn shades_carry_their_materials_down() {
        let shine = [0xDC, 0xDE, 0xDA, 0xFF];
        let (material, vector, spec, got) = Shade::Polished {
            normal: Vec3::Z,
            shine,
            sharpness: 24.0,
        }
        .parts();
        assert_eq!((material, vector, spec, got), (2, Vec3::Z, 24.0, shine));
        let (material, _, spec, _) = Shade::Chrome(0.75).parts();
        assert_eq!((material, spec), (4, 0.75), "how far down the plate");
        let (material, _, spec, _) = Shade::Streak {
            tangent: Vec3::X,
            shine,
            sharpness: 6.0,
        }
        .parts();
        assert_eq!((material, spec), (3, 6.0));
        let (material, _, spec, _) = Shade::Catcher(0.4).parts();
        assert_eq!((material, spec), (5, 0.4));
        let (material, vector, _, _) = Shade::Matte(Vec3::Y).parts();
        assert_eq!((material, vector), (1, Vec3::Y));
        let (material, _, _, shine) = Shade::Unlit.parts();
        assert_eq!((material, shine), (0, [0xFF; 4]));
    }
}
