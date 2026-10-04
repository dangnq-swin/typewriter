//! The triangles a scene draws: one texture's [`Mesh`], standing in the
//! mesh's own machine millimetres, and the [`Texture`] its uvs read.

use super::vertex::Vertex;

/// A texture the device holds, by handle: what a mesh's uvs read.
/// [`Texture::WHITE`], the device's own one-texel white, stands where no
/// texels are wanted — the colour rides the vertices.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Texture(pub(super) u32);

impl Texture {
    /// The device's white texel, uploaded under this handle at birth.
    pub const WHITE: Self = Self(0);
}

impl Default for Texture {
    /// The white texel: a mesh built default stands on it.
    fn default() -> Self {
        Self::WHITE
    }
}

/// One texture's triangles, standing in the mesh's own millimetres: a
/// placement's transform carries them into the frame's absolute ones. A
/// mesh built once can be kept and placed afresh every frame — the
/// placement moves it, the millimetres stay.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Mesh {
    pub vertices: Vec<Vertex>,
    pub indices: Vec<u32>,
    pub texture: Texture,
}

impl Mesh {
    /// An empty mesh on `texture`.
    pub fn new(texture: Texture) -> Self {
        Self {
            vertices: Vec::new(),
            indices: Vec::new(),
            texture,
        }
    }

    /// Whether nothing would draw.
    pub fn is_empty(&self) -> bool {
        self.indices.is_empty()
    }

    /// Adds `triangle`.
    pub fn triangle(&mut self, a: Vertex, b: Vertex, c: Vertex) {
        // Safe cast: a mesh's vertices fit its u32 indices.
        let base = self.vertices.len() as u32;
        self.vertices.extend([a, b, c]);
        self.indices.extend([base, base + 1, base + 2]);
    }

    /// Adds `quad` as two triangles, a–b–c and a–c–d.
    pub fn quad(&mut self, a: Vertex, b: Vertex, c: Vertex, d: Vertex) {
        self.triangle(a, b, c);
        self.triangle(a, c, d);
    }
}
