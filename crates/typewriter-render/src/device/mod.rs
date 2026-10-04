//! The renderer's core: the pipelines it draws with, the camera that
//! projects, its own depth buffer, and the [`Scene`] a frame is gathered
//! into — the interface the machine's parts, the lamp and the sheets build
//! against. It knows no window and no egui: a library the app feeds
//! scenes, on a device of its own or the window's.
//!
//! A frame is millimetres end to end: meshes stand in their own machine
//! millimetres, placements carry them into absolute ones, and the shader
//! projects — the CPU stops re-projecting, which is where the old pass's
//! ~1.8 ms went. What a frame draws, [`Scene::opaque`] and its layer
//! siblings say; the device keeps each named mesh's triangles uploaded
//! and moves standing solids by transform alone; [`Placing::casting`] is
//! the hook the lamp's casters register into. The frame's passes run in
//! one order: the lamp's shadow map, then opaque, then decals, then text.
//!
//! The bare-device tests beside it all run on a headless device in plain
//! `cargo test`, reading back what was rendered: `capture` is their rig.

mod camera;
#[cfg(test)]
mod capture;
mod mesh;
mod passes;
mod pipelines;
mod scene;
mod textures;
mod vertex;

pub use camera::{Camera, CameraUniform, NEAR_MM};
pub use mesh::{Mesh, Texture};
pub use passes::{DEPTH_BITS, Depth};
pub use pipelines::{LIGHTING_BYTES, LightingUniform, SHADOW_MAP};
pub use scene::{Layer, Placing, Scene};
pub use vertex::{Shade, VERTEX_BYTES, Vertex};

use std::collections::HashMap;

#[cfg(test)]
use std::cell::Cell;

use passes::Buffers;
use pipelines::Pipelines;
use textures::Textures;

/// The renderer's core, on a device: its pipelines, textures and standing
/// meshes, ready to draw scenes.
pub struct Device {
    device: wgpu::Device,
    queue: wgpu::Queue,
    pipelines: Pipelines,
    textures: Textures,
    /// Standing meshes by name: uploaded once, re-uploaded only where a
    /// frame replaces them.
    meshes: HashMap<String, passes::Standing>,
    /// The frame's camera and placements, grown when too small.
    frame: Option<Buffers>,
    /// Mesh re-uploads, for the standing-solid test.
    #[cfg(test)]
    uploads: Cell<u32>,
}

/// Where a frame draws: the target's view, the depth beside it, and what
/// the frame clears to.
pub struct Target<'a> {
    pub view: &'a wgpu::TextureView,
    pub depth: &'a wgpu::TextureView,
    pub clear: wgpu::Color,
}

impl Device {
    /// Builds the core on the app's device, for a target in `format` —
    /// the adapter rides along for what its capabilities decide.
    pub fn new(
        adapter: &wgpu::Adapter,
        device: wgpu::Device,
        queue: wgpu::Queue,
        format: wgpu::TextureFormat,
    ) -> Self {
        let pipelines = Pipelines::build(&device, format, clamp(adapter));
        let mut textures = Textures::new(&device);
        // The white texel every mesh stands on where no texels are wanted:
        // colour reaches the shader through vertices, not the texture.
        textures.upload(&device, &queue, "white".into(), [1, 1], &[0xFF; 4]);
        Self {
            device,
            queue,
            pipelines,
            textures,
            meshes: HashMap::new(),
            frame: None,
            #[cfg(test)]
            uploads: Cell::new(0),
        }
    }

    /// Uploads `rgba`'s texels — premultiplied sRGB gamma, as egui's — as
    /// `name`, and returns the handle meshes name it by. A name replaced
    /// keeps its handle, so meshes already named on it draw the new
    /// texels: the font atlas grows this way.
    pub fn texture(&mut self, name: impl Into<String>, size: [u32; 2], rgba: &[u8]) -> Texture {
        self.textures
            .upload(&self.device, &self.queue, name.into(), size, rgba)
    }

    /// The core's own depth buffer, sized to `size`: what its passes
    /// depth-test into, or what a window owes them beside its surface.
    pub fn depth(&self, size: [u32; 2]) -> Depth {
        Depth::new(&self.device, size)
    }

    /// Draws `scene` as the frame does, onto `target`: the lamp's shadow
    /// pass first, then opaque, then what lies on it, then text — encoded
    /// into `encoder` in that order, so the caller submits it where its
    /// frame wants it.
    pub fn encode(
        &mut self,
        scene: &Scene,
        camera: &Camera,
        lighting: &LightingUniform,
        target: Target<'_>,
        encoder: &mut wgpu::CommandEncoder,
    ) {
        let plan = self.receive(scene, camera, lighting);
        self.lamp_pass(lighting, &plan, encoder);
        self.main_pass(target, &plan, encoder);
    }

    /// The lamp's shadow map, for the tests' readback.
    #[cfg(test)]
    pub(crate) fn shadow_texture(&self) -> &wgpu::Texture {
        &self.pipelines.shadow
    }
}

/// Whether the backend caps the decal bias's slope term at all: devices
/// without the clamping lift by the constant alone.
fn clamp(adapter: &wgpu::Adapter) -> bool {
    adapter
        .get_downlevel_capabilities()
        .flags
        .contains(wgpu::DownlevelFlags::DEPTH_BIAS_CLAMP)
}
