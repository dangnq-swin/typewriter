//! Test-only: a headless device the bare-device tests render through, and
//! the readbacks that say what was drawn — the old depth pass's rig,
//! living beside the core it now tests. The machine's GPU answers
//! offscreen, so these run in plain `cargo test`.

use glam::{Vec2, Vec3, Vec4};
use pollster::block_on;

use super::Device;
use super::camera::Camera;
use super::mesh::{Mesh, Texture};
use super::passes::Depth;
use super::pipelines::{LightingUniform, SHADOW_MAP};
use super::scene::{Placing, Scene};
use super::vertex::{Shade, Vertex};

/// A headless device with the renderer core on it, an offscreen target and
/// its depth beside it, and the readbacks that say what was drawn.
pub(crate) struct Rig {
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub backend: wgpu::Backend,
    pub core: Device,
    pub size: [u32; 2],
    target: wgpu::Texture,
    pub view: wgpu::TextureView,
    pub depth: Depth,
}

impl Rig {
    /// The machine's GPU, offscreen, with the core built for a plain
    /// `Rgba8Unorm` target.
    pub fn new(size: [u32; 2]) -> Self {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
        let adapter = block_on(instance.request_adapter(&Default::default()))
            .expect("no headless device answered");
        let info = adapter.get_info();
        eprintln!("adapter: {:?} {}", info.backend, info.name);
        let (device, queue) = block_on(adapter.request_device(&wgpu::DeviceDescriptor::default()))
            .expect("the adapter answered but the device did not");
        let core = Device::new(
            &adapter,
            device.clone(),
            queue.clone(),
            wgpu::TextureFormat::Rgba8Unorm,
        );
        let target = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("capture_target"),
            size: wgpu::Extent3d {
                width: size[0],
                height: size[1],
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let view = target.create_view(&Default::default());
        Self {
            backend: info.backend,
            core,
            depth: Depth::new(&device, size),
            device,
            queue,
            size,
            target,
            view,
        }
    }

    /// Draws `scene` as the frame does — the lamp's pass first, then the
    /// target's — and leaves the target for [`Self::pixels`].
    pub fn draw(
        &mut self,
        scene: &Scene,
        camera: &Camera,
        lighting: &LightingUniform,
        clear: wgpu::Color,
    ) {
        let mut encoder = self.device.create_command_encoder(&Default::default());
        self.core.encode(
            scene,
            camera,
            lighting,
            super::Target {
                view: &self.view,
                depth: self.depth.view(),
                clear,
            },
            &mut encoder,
        );
        self.queue.submit([encoder.finish()]);
    }

    /// The target's pixels, top row first, read back: what was drawn.
    pub fn pixels(&self) -> Vec<[u8; 4]> {
        let [width, height] = self.size;
        // The copy wants rows a multiple of the copy alignment.
        let row_bytes = (width * 4).next_multiple_of(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT);
        let readback = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("capture_readback"),
            size: u64::from(row_bytes) * u64::from(height),
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let mut encoder = self.device.create_command_encoder(&Default::default());
        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture: &self.target,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &readback,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(row_bytes),
                    rows_per_image: None,
                },
            },
            wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );
        self.queue.submit([encoder.finish()]);
        let mapped = mapped(&self.device, &readback);
        let mut pixels = Vec::with_capacity((width * height) as usize);
        for y in 0..height {
            let at = y as usize * row_bytes as usize;
            for texel in mapped[at..at + width as usize * 4].as_chunks::<4>().0 {
                pixels.push(*texel);
            }
        }
        drop(mapped);
        readback.unmap();
        pixels
    }

    /// One pixel, `x` across and `y` down.
    pub fn pixel(&self, x: u32, y: u32) -> [u8; 4] {
        self.pixels()[(y * self.size[0] + x) as usize]
    }

    /// The shadow map's depths, row-major: what the lamp saw, its f16
    /// texels decoded.
    pub fn shadow_map(&self) -> Vec<f32> {
        let side = SHADOW_MAP;
        // R16Float: two bytes a texel, and the map's rows are already a
        // multiple of 256 bytes.
        let row_bytes = side * 2;
        let readback = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("capture_shadow"),
            size: u64::from(row_bytes) * u64::from(side),
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let mut encoder = self.device.create_command_encoder(&Default::default());
        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture: self.core.shadow_texture(),
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &readback,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(row_bytes),
                    rows_per_image: None,
                },
            },
            wgpu::Extent3d {
                width: side,
                height: side,
                depth_or_array_layers: 1,
            },
        );
        self.queue.submit([encoder.finish()]);
        let mapped = mapped(&self.device, &readback);
        let depths = mapped
            .as_chunks::<2>()
            .0
            .iter()
            .map(|texel| half_to_f32(u16::from_le_bytes(*texel)))
            .collect();
        drop(mapped);
        readback.unmap();
        depths
    }
}

/// Maps `buffer` whole, once its copy has landed.
fn mapped(device: &wgpu::Device, buffer: &wgpu::Buffer) -> wgpu::BufferView {
    let (sender, receiver) = std::sync::mpsc::sync_channel(1);
    buffer
        .slice(..)
        .map_async(wgpu::MapMode::Read, move |mapped| {
            let _ = sender.send(mapped);
        });
    let _ = device.poll(wgpu::PollType::wait_indefinitely());
    receiver
        .recv()
        .expect("no mapping came back")
        .expect("the mapping failed");
    buffer.get_mapped_range(..).expect("the map was taken")
}

/// An f16 texel's value.
fn half_to_f32(bits: u16) -> f32 {
    half::f16::from_bits(bits).to_f32()
}

/// A seated eye, rows by hand, as `Eye::camera` builds them: the eye 600
/// in front of the printing point looking down its depth, z up — a wall a
/// hand's width across fills a 64-point target.
pub(crate) fn camera() -> Camera {
    camera_at(Vec2::splat(64.0))
}

/// The same eye, over a target `points` big: only the ratio decides, and
/// `project` divides back out of the rows by it.
pub(crate) fn camera_at(points: Vec2) -> Camera {
    Camera {
        rows: [
            Vec4::new(15.0, 0.0, 0.0, 0.0),
            Vec4::new(0.0, 0.0, 15.0, 0.0),
            Vec4::new(0.0, -1.0, 0.0, 0.0),
            Vec4::new(0.0, -1.0, 0.0, 600.0),
        ],
        points,
    }
}

/// A wall standing across the eye's way: in the plane `y = centre.y`,
/// `half` millimetres each way of `centre` along x and z. The pipelines
/// cull nothing, so either face draws.
pub(crate) fn wall(centre: Vec3, half: f32, color: [u8; 4]) -> Mesh {
    let mut mesh = Mesh::new(Texture::WHITE);
    let vertex = |x: f32, z: f32| {
        Vertex::placed(
            Vec3::new(centre.x + x, centre.y, centre.z + z),
            [0.0; 2],
            color,
            Shade::Matte(Vec3::NEG_Y),
        )
    };
    mesh.quad(
        vertex(-half, -half),
        vertex(half, -half),
        vertex(half, half),
        vertex(-half, half),
    );
    mesh
}

/// A wall as [`wall`], its faces unlit: for the tests that read exact
/// vertex colours rather than the lamp's light.
pub(crate) fn unlit_wall(centre: Vec3, half: f32, color: [u8; 4]) -> Mesh {
    let mut mesh = wall(centre, half, color);
    for vertex in &mut mesh.vertices {
        *vertex = Vertex::placed(vertex.at, vertex.uv, vertex.color, Shade::Unlit);
    }
    mesh
}

/// A slab lying along the desk: in the plane `z = centre.z`, `half`
/// millimetres each way of `centre` along x and y.
pub(crate) fn slab(centre: Vec3, half: f32, color: [u8; 4]) -> Mesh {
    let mut mesh = Mesh::new(Texture::WHITE);
    let vertex = |x: f32, y: f32| {
        Vertex::placed(
            Vec3::new(centre.x + x, centre.y + y, centre.z),
            [0.0; 2],
            color,
            Shade::Matte(Vec3::Z),
        )
    };
    mesh.quad(
        vertex(-half, -half),
        vertex(half, -half),
        vertex(half, half),
        vertex(-half, half),
    );
    mesh
}

/// Shadows straight down from a lamp 300 above the floor: across maps x,
/// the map's up maps y, and depth runs 0 at the lamp to 1 at the floor.
pub(crate) fn downward() -> LightingUniform {
    LightingUniform {
        lamp: Vec4::new(0.0, -150.0, 300.0, 0.0),
        eye: Vec4::new(0.0, 1.0, 0.0, 0.0),
        bands: Default::default(),
        shadow: [
            Vec4::new(1.0 / 150.0, 0.0, 0.0, 0.0),
            Vec4::new(0.0, 1.0 / 150.0, 0.0, 0.0),
            Vec4::new(0.0, 0.0, -1.0 / 300.0, 1.0),
        ],
        params: Vec4::new(0.0, SHADOW_MAP as f32, 1.0, 300.0),
    }
}

/// A scene of one mesh placed where `placing` puts it.
pub(crate) fn scene_of<'a>(name: &str, mesh: &'a Mesh, placing: Placing) -> Scene<'a> {
    let mut scene = Scene::default();
    scene.opaque(name, mesh, placing);
    scene
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The lamp's pass records what casts: the caster's own depth stands in
    /// the map where it is, and the map stays clear where it is not.
    #[test]
    fn the_lamp_records_a_caster_in_the_shadow_map() {
        let mut rig = Rig::new([64, 64]);
        let caster = slab(Vec3::new(0.0, 0.0, 150.0), 20.0, [0xFF; 4]);
        let scene = scene_of("caster", &caster, Placing::at(Vec3::ZERO));
        rig.draw(&scene, &camera(), &downward(), wgpu::Color::BLACK);
        let map = rig.shadow_map();
        // `downward`: across maps x and y over ±150, depth runs 0 at the
        // lamp (z 300) to 1 at the floor (z 0).
        let texel = |x_mm: f32, y_mm: f32| {
            let side = SHADOW_MAP as usize;
            let column = ((1.0 + x_mm / 150.0) * 0.5 * SHADOW_MAP as f32) as usize;
            let row = ((1.0 - y_mm / 150.0) * 0.5 * SHADOW_MAP as f32) as usize;
            map[row * side + column]
        };
        assert!(
            (texel(0.0, 0.0) - 0.5).abs() < 1e-3,
            "the caster's own depth, {}",
            texel(0.0, 0.0)
        );
        assert!(
            (texel(100.0, 0.0) - 1.0).abs() < 1e-6,
            "clear where it does not stand, {}",
            texel(100.0, 0.0)
        );
    }

    /// A lit face darkens where a caster hides it and brightens toward the
    /// lamp's spot: the diffuse term follows the way to the head, and the
    /// shadow map takes the lamp's own light away where the caster stands.
    #[test]
    fn a_lit_face_darkens_in_shadow_and_brightens_toward_the_lamp() {
        let mut rig = Rig::new([64, 64]);
        let camera = camera();
        // The lit wall at the printing point's plane, casting nothing, so
        // the map holds only the caster above it.
        let wall = wall(Vec3::ZERO, 60.0, [0x80, 0x80, 0x80, 0xFF]);
        let caster = slab(Vec3::new(0.0, 0.0, 150.0), 20.0, [0xFF; 4]);
        let mut scene = Scene::default();
        scene.opaque("wall", &wall, Placing::at(Vec3::ZERO).casting(false));
        scene.opaque("caster", &caster, Placing::at(Vec3::ZERO));
        rig.draw(&scene, &camera, &downward(), wgpu::Color::BLACK);
        // The eye looks along −y: x runs across the target, z up. The
        // caster's footprint is x, y ∈ ±20; the wall is x, z ∈ ±60.
        let shadowed = rig.pixel(32, 32); // world (0, 0, 0): under the caster
        let lit = rig.pixel(52, 32); // world (25, 0, 0): clear of it
        assert!(
            shadowed[0] + 10 < lit[0],
            "shadowed {shadowed:?} against lit {lit:?}"
        );
        assert!(shadowed[0] < 110, "the ambient floor alone: {shadowed:?}");
        assert!(lit[0] > 110, "the lamp's own light: {lit:?}");
    }

    /// A slab lit by a head above it is brighter where it stands nearer the
    /// head: the diffuse term falls off across the face as the way to the
    /// lamp turns away.
    #[test]
    fn a_lit_slab_brightens_toward_the_lamp() {
        let mut rig = Rig::new([64, 64]);
        // A head 40 above the slab, straight down, no shadows: the look
        // depends on the way to the head alone.
        let mut lit = downward();
        lit.lamp = Vec4::new(0.0, 0.0, 40.0, 0.0);
        lit.eye = Vec4::new(0.0, 0.0, 1.0, 0.0);
        lit.params.y = 0.0;
        let slab = slab(Vec3::ZERO, 30.0, [0x80, 0x80, 0x80, 0xFF]);
        let scene = scene_of("slab", &slab, Placing::at(Vec3::ZERO));
        rig.draw(&scene, &top_down(), &lit, wgpu::Color::BLACK);
        // The eye looks straight down: x across, y up the target. The
        // slab's centre sits under the head; its edge 25 mm out turns away.
        let under = rig.pixel(32, 32); // world (0, 0, 0)
        let edge = rig.pixel(52, 32); // world (25, 0, 0)
        assert!(
            edge[0] + 4 < under[0],
            "edge {edge:?} against under {under:?}"
        );
    }

    /// A camera above the origin looking straight down: x across the
    /// target, y up it, the view 40 mm wide a 64-point side (0.8 points a
    /// millimetre), the eye 600 above the slab plane.
    fn top_down() -> Camera {
        Camera {
            rows: [
                Vec4::new(15.0, 0.0, 0.0, 0.0),
                Vec4::new(0.0, 15.0, 0.0, 0.0),
                Vec4::new(0.0, 0.0, -1.0, 0.0),
                Vec4::new(0.0, 0.0, -1.0, 600.0),
            ],
            points: Vec2::splat(64.0),
        }
    }
}
