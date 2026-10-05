//! The bridge from the old depth pass's [`Solids`] to the render crate's
//! meshes: the geometry the machine's builders already stand in, carried
//! across unchanged. A solid's vertices each carry the absolute machine
//! millimetres they stand at, their uv, colour and shade; those become the
//! render crate's own vertices, and a placement's transform moves a
//! standing solid instead of re-tessellating it.
//!
//! The old pass keeps drawing the same solids until T5: nothing here
//! changes it. A frame is gathered into [`Frame`], which owns the meshes a
//! [`Scene`] borrows and is rebuilt where a part's geometry changes.

use glam::Vec3;
use typewriter_render::device::{Layer, Mesh, Placing, Scene, Shade, Texture, Vertex};

use crate::depth;

/// The render crate's take on a depth shade: the same material, its
/// highlight's colour the bytes the shader reads.
pub(crate) fn shade(shade: depth::Shade) -> Shade {
    match shade {
        depth::Shade::Unlit => Shade::Unlit,
        depth::Shade::Matte(normal) => Shade::Matte(normal),
        depth::Shade::Polished {
            normal,
            shine,
            sharpness,
        } => Shade::Polished {
            normal,
            shine: shine.to_array(),
            sharpness,
        },
        depth::Shade::Streak {
            tangent,
            shine,
            sharpness,
        } => Shade::Streak {
            tangent,
            shine: shine.to_array(),
            sharpness,
        },
        depth::Shade::Chrome(t) => Shade::Chrome(t),
        depth::Shade::Catcher(strength) => Shade::Catcher(strength),
    }
}

/// The render crate's [`Layer`] for a depth layer: opaque hides, a decal
/// lies on what it is drawn over.
fn layer(layer: depth::Layer) -> Layer {
    match layer {
        depth::Layer::Opaque => Layer::Opaque,
        depth::Layer::Decal => Layer::Decal,
    }
}

/// One depth [`depth::Solid`] as a render mesh, standing in its own
/// millimetres: each vertex where its places say, coloured and shaded as
/// the builders left it. `texture` is what its uvs read — [`Texture::WHITE`]
/// where the colour rides the vertices.
pub(crate) fn mesh(solid: &depth::Solid, texture: Texture) -> Mesh {
    let mut mesh = Mesh::new(texture);
    for (i, vertex) in solid.mesh.vertices.iter().enumerate() {
        let Some(&at) = solid.places.get(i) else {
            continue;
        };
        let take = solid.shades.get(i).copied().unwrap_or_default();
        mesh.vertices.push(Vertex::placed(
            at,
            [vertex.uv.x, vertex.uv.y],
            vertex.color.to_array(),
            shade(take),
        ));
    }
    mesh.indices = solid.mesh.indices.clone();
    mesh
}

/// A frame's meshes, owned, and the placements a [`Scene`] borrows them
/// from: the new path gathers the machine's standing parts here, then hands
/// the scene to the device.
#[derive(Default)]
pub(crate) struct Frame {
    meshes: Vec<(String, Mesh)>,
    placed: Vec<(usize, Layer, Placing)>,
}

impl Frame {
    /// An empty frame.
    pub(crate) fn new() -> Self {
        Self::default()
    }

    /// Whether nothing would draw.
    pub(crate) fn is_empty(&self) -> bool {
        self.placed.is_empty()
    }

    /// Adds every solid of `solids` under names `name`-led, standing `shift`
    /// millimetres off where it was built: the meshes are converted once,
    /// and a standing part's travel is its placement's transform.
    pub(crate) fn add(&mut self, name: &str, solids: &depth::Solids, shift: Vec3) {
        for (depth_layer, group) in solids.layers() {
            let layer = layer(depth_layer);
            for (i, solid) in group.iter().enumerate() {
                if solid.mesh.indices.is_empty() {
                    continue;
                }
                let index = self.meshes.len();
                self.meshes
                    .push((format!("{name}.{i}"), mesh(solid, Texture::WHITE)));
                let placing = match layer {
                    Layer::Opaque => Placing::at(shift),
                    Layer::Decal => Placing::flat(shift),
                    Layer::Text => Placing::flat(shift),
                };
                self.placed.push((index, layer, placing));
            }
        }
    }

    /// Adds one already-converted `mesh` as `name`, drawn in `layer` where
    /// `placing` puts it.
    pub(crate) fn add_mesh(&mut self, name: String, mesh: Mesh, layer: Layer, placing: Placing) {
        let index = self.meshes.len();
        self.meshes.push((name, mesh));
        self.placed.push((index, layer, placing));
    }

    /// The scene borrowing this frame's meshes, in the order they were
    /// added. Later registrations under a name replace earlier ones, exactly
    /// as the device expects.
    pub(crate) fn scene(&self) -> Scene<'_> {
        let mut scene = Scene::default();
        for &(index, layer, placing) in &self.placed {
            let (name, mesh) = &self.meshes[index];
            match layer {
                Layer::Opaque => scene.opaque(name.clone(), mesh, placing),
                Layer::Decal => scene.decal(name.clone(), mesh, placing),
                Layer::Text => scene.text(name.clone(), mesh, placing),
            }
        }
        scene
    }
}
