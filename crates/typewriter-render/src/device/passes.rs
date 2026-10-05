//! The frame's passes, in order: the lamp's shadow map first, then the
//! target — opaque, then what lies on it, then text — with the standing
//! meshes' uploads and the frame's uniforms before them.

use std::collections::HashMap;
use std::num::NonZeroU64;

use bytemuck::bytes_of;

use super::Device;
use super::camera::{CAMERA_BYTES, Camera};
use super::mesh::{Mesh, Texture};
use super::pipelines::{LightingUniform, MODEL_BYTES};
use super::scene::{Layer, Scene};

/// The frame's depth buffer, in bits: what the window's surface owes the
/// passes that depth-test into it, and what the core's own [`Depth`] is.
/// One sample a pixel, as egui's: wgpu's OpenGL backend shows nothing with
/// more.
pub const DEPTH_BITS: u8 = 32;

/// The renderer core's own depth buffer: what its passes depth-test into,
/// sized to the target they draw on.
pub struct Depth {
    view: wgpu::TextureView,
    size: [u32; 2],
}

impl Depth {
    /// A `DEPTH_BITS`-deep buffer the size of `size`.
    pub fn new(device: &wgpu::Device, size: [u32; 2]) -> Self {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("scene_depth"),
            size: wgpu::Extent3d {
                width: size[0],
                height: size[1],
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Depth32Float,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });
        Self {
            view: texture.create_view(&Default::default()),
            size,
        }
    }

    /// Resizes to `size`, for a target that grew or shrank.
    pub fn resize(&mut self, device: &wgpu::Device, size: [u32; 2]) {
        if self.size != size {
            *self = Self::new(device, size);
        }
    }

    /// The view the passes depth-test into.
    pub fn view(&self) -> &wgpu::TextureView {
        &self.view
    }

    /// The target's size it is sized to, in pixels.
    pub fn size(&self) -> [u32; 2] {
        self.size
    }
}

/// A transform slot's stride, in bytes: dynamic offsets align to the
/// default `min_uniform_buffer_offset_alignment`, and a `Mat4` rides in
/// the first 64 of each.
pub(super) const MODEL_SLOT_BYTES: u32 = 256;

/// A named mesh's uploaded triangles: standing solids are written once
/// and stay until a frame replaces them.
pub(super) struct Standing {
    vertices: wgpu::Buffer,
    indices: wgpu::Buffer,
    /// What these hold: replaced where a frame's mesh differs.
    mesh: Mesh,
}

impl Standing {
    /// The indices' count, for the draw.
    fn indices_count(&self) -> u32 {
        // Safe cast: a mesh's indices fit a u32 draw.
        self.mesh.indices.len() as u32
    }
}
/// The frame's camera and its placements' transforms, refilled each frame
/// and grown when too small: one bind group both passes bind.
pub(super) struct Buffers {
    camera: wgpu::Buffer,
    models: wgpu::Buffer,
    group: wgpu::BindGroup,
    slots: u32,
}

impl Buffers {
    /// Room for `slots` transforms, to the next power of two: a growing
    /// frame grows them seldom. The camera is one fixed uniform.
    fn new(device: &wgpu::Device, layout: &wgpu::BindGroupLayout, slots: u32) -> Self {
        let slots = slots.max(1).next_power_of_two();
        let camera = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("scene_camera"),
            size: CAMERA_BYTES,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let models = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("scene_models"),
            size: u64::from(slots) * u64::from(MODEL_SLOT_BYTES),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("scene_frame"),
            layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: camera.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    // A slot's worth, not the whole buffer: each dynamic
                    // offset binds this much from its offset.
                    resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                        buffer: &models,
                        offset: 0,
                        size: NonZeroU64::new(MODEL_BYTES as u64),
                    }),
                },
            ],
        });
        Self {
            camera,
            models,
            group,
            slots,
        }
    }
}

/// The frame as received: its draws in placement order.
pub(super) struct Plan {
    pub(super) draws: Vec<Draw>,
}

/// A placement as drawn: its layer's pipeline, texture, mesh, and the
/// transform slot it projects by.
pub(super) struct Draw {
    pub(super) layer: Layer,
    pub(super) texture: Texture,
    pub(super) mesh: String,
    pub(super) slot: u32,
    pub(super) casts: bool,
}

impl Device {
    /// Takes the frame's meshes in — uploading only where a named mesh
    /// differs from what stands — writes the frame's camera, lighting and
    /// placements, and lays the draw list out in placement order.
    pub(super) fn receive(
        &mut self,
        scene: &Scene,
        camera: &Camera,
        lighting: &LightingUniform,
    ) -> Plan {
        // The last mesh registered under a name is what the frame holds.
        let mut named: HashMap<&str, &Mesh> = HashMap::new();
        for (name, mesh) in scene.meshes() {
            named.insert(name.as_str(), *mesh);
        }
        let mut draws = Vec::new();
        let mut models = Vec::new();
        for placed in scene.placed() {
            let Some(mesh) = named.get(placed.mesh.as_str()).copied() else {
                // Placing an unregistered name draws nothing: loud in
                // debug, for a typo'd name would hide a solid silently
                // otherwise.
                debug_assert!(
                    false,
                    "placing {:?}, which the scene does not name",
                    placed.mesh
                );
                continue;
            };
            if mesh.is_empty() {
                continue;
            }
            self.upload(placed.mesh.clone(), mesh);
            // Safe cast: a frame's draws fit a u32 slot.
            let slot = draws.len() as u32;
            draws.push(Draw {
                layer: placed.layer,
                texture: mesh.texture,
                mesh: placed.mesh.clone(),
                slot,
                casts: placed.placing.casts,
            });
            let at = models.len();
            models.resize(at + MODEL_SLOT_BYTES as usize, 0);
            models[at..at + MODEL_BYTES].copy_from_slice(bytes_of(&placed.placing.model));
        }
        self.frame_for(draws.len() as u32);
        if let Some(frame) = &self.frame {
            self.queue
                .write_buffer(&frame.camera, 0, bytes_of(&camera.uniform()));
            if !models.is_empty() {
                self.queue.write_buffer(&frame.models, 0, &models);
            }
        }
        self.queue
            .write_buffer(&self.pipelines.lighting, 0, bytes_of(lighting));
        Plan { draws }
    }

    /// Writes `mesh` over `name`'s standing triangles where they differ:
    /// an unchanged mesh is not re-uploaded, so a standing solid uploads
    /// once and moves by its placement's transform alone.
    fn upload(&mut self, name: String, mesh: &Mesh) {
        if self
            .meshes
            .get(&name)
            .is_some_and(|standing| standing.mesh == *mesh)
        {
            return;
        }
        // Safe casts: byte counts.
        let (vertex_bytes, index_bytes) = (
            std::mem::size_of_val(&mesh.vertices[..]) as u64,
            std::mem::size_of_val(&mesh.indices[..]) as u64,
        );
        // What stands already keeps its buffers where they are big enough;
        // a grown mesh gets new ones, the old ending with the replacement.
        let held = self.meshes.get(&name);
        let vertices = match held {
            Some(standing) if standing.vertices.size() >= vertex_bytes => standing.vertices.clone(),
            _ => self.buffer(
                "scene_mesh_vertices",
                vertex_bytes,
                wgpu::BufferUsages::VERTEX,
            ),
        };
        let indices = match held {
            Some(standing) if standing.indices.size() >= index_bytes => standing.indices.clone(),
            _ => self.buffer("scene_mesh_indices", index_bytes, wgpu::BufferUsages::INDEX),
        };
        self.queue
            .write_buffer(&vertices, 0, bytemuck::cast_slice(&mesh.vertices));
        self.queue
            .write_buffer(&indices, 0, bytemuck::cast_slice(&mesh.indices));
        self.meshes.insert(
            name,
            Standing {
                vertices,
                indices,
                mesh: mesh.clone(),
            },
        );
        #[cfg(test)]
        self.uploads.set(self.uploads.get() + 1);
    }

    fn buffer(&self, label: &str, bytes: u64, usage: wgpu::BufferUsages) -> wgpu::Buffer {
        self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some(label),
            size: bytes.max(1),
            usage: usage | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        })
    }

    /// The frame's buffers, grown to hold `slots` transforms: rebuilt only
    /// when a frame wants more than they hold.
    fn frame_for(&mut self, slots: u32) {
        if self
            .frame
            .as_ref()
            .is_some_and(|frame| frame.slots >= slots)
        {
            return;
        }
        self.frame = Some(Buffers::new(
            &self.device,
            &self.pipelines.frame_layout,
            slots,
        ));
    }

    /// Draws the casters into the shadow map, before anything reads it:
    /// their faces again through `vs_lamp`, the survivor's depth written
    /// by `fs_lamp`. Decals lie flat and hide nothing; they cast nothing
    /// either.
    pub(super) fn lamp_pass(
        &self,
        lighting: &LightingUniform,
        plan: &Plan,
        encoder: &mut wgpu::CommandEncoder,
    ) {
        if lighting.params.y <= 0.0 || !plan.draws.iter().any(|draw| draw.casts) {
            return;
        }
        let Some(frame) = &self.frame else {
            return;
        };
        let mut lamp = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("scene_lamp"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &self.pipelines.shadow_view,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::WHITE),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: &self.pipelines.lamp_depth_view,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Clear(1.0),
                    store: wgpu::StoreOp::Discard,
                }),
                stencil_ops: None,
            }),
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        lamp.set_pipeline(&self.pipelines.lamp);
        lamp.set_bind_group(1, &self.pipelines.lighting_group, &[]);
        for draw in &plan.draws {
            if !draw.casts {
                continue;
            }
            let Some(standing) = self.meshes.get(&draw.mesh) else {
                continue;
            };
            lamp.set_bind_group(2, &frame.group, &[draw.slot * MODEL_SLOT_BYTES]);
            lamp.set_vertex_buffer(0, standing.vertices.slice(..));
            lamp.set_index_buffer(standing.indices.slice(..), wgpu::IndexFormat::Uint32);
            lamp.draw_indexed(0..standing.indices_count(), 0, 0..1);
        }
    }

    /// Draws the frame onto `target`: opaque faces hiding what is behind
    /// them, then what lies on them, then text — each draw its placement's
    /// transform.
    pub(super) fn main_pass(
        &self,
        target: super::Target<'_>,
        plan: &Plan,
        encoder: &mut wgpu::CommandEncoder,
    ) {
        let Some(frame) = &self.frame else {
            return;
        };
        let mut render = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("scene"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: target.view,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(target.clear),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: target.depth,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Clear(1.0),
                    store: wgpu::StoreOp::Store,
                }),
                stencil_ops: None,
            }),
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        render.set_bind_group(1, &self.pipelines.lighting_group, &[]);
        // The lamp's map, bound and waiting for the lit fragments T3
        // lands.
        render.set_bind_group(3, &self.pipelines.shadow_group, &[]);
        for draw in &plan.draws {
            let Some(standing) = self.meshes.get(&draw.mesh) else {
                continue;
            };
            let Some(texture) = self.textures.group(draw.texture) else {
                continue;
            };
            render.set_pipeline(match draw.layer {
                Layer::Opaque => &self.pipelines.opaque,
                Layer::Decal => &self.pipelines.decal,
                Layer::Text => &self.pipelines.text,
            });
            render.set_bind_group(0, texture, &[]);
            render.set_bind_group(2, &frame.group, &[draw.slot * MODEL_SLOT_BYTES]);
            render.set_vertex_buffer(0, standing.vertices.slice(..));
            render.set_index_buffer(standing.indices.slice(..), wgpu::IndexFormat::Uint32);
            render.draw_indexed(0..standing.indices_count(), 0, 0..1);
        }
    }
}

#[cfg(test)]
mod tests {
    use glam::{Vec2, Vec3};

    use super::*;
    use crate::device::capture::{self, Rig};
    use crate::device::pipelines::SHADOW_MAP;
    use crate::device::scene::Placing;

    /// The eye's own red, green, blue and yellow, premultiplied.
    const RED: [u8; 4] = [0xFF, 0, 0, 0xFF];
    const GREEN: [u8; 4] = [0, 0xFF, 0, 0xFF];
    const BLUE: [u8; 4] = [0, 0, 0xFF, 0xFF];
    const YELLOW: [u8; 4] = [0xFF, 0xFF, 0, 0xFF];
    const BLACK: [u8; 4] = [0, 0, 0, 0xFF];

    /// Opaque faces hide what is behind them; what lies on them shows
    /// over them, decals first and text after, each lifted by its
    /// pipeline's bias.
    #[test]
    fn opaque_hides_what_is_behind_and_what_lies_on_shows_over() {
        let mut rig = Rig::new([64, 64]);
        let camera = capture::camera();
        let room = capture::unlit_wall(Vec3::new(0.0, -300.0, 0.0), 30.0, GREEN);
        let near = capture::unlit_wall(Vec3::ZERO, 10.0, RED);
        let print = capture::unlit_wall(Vec3::ZERO, 6.0, BLUE);
        let glyph = capture::unlit_wall(Vec3::ZERO, 6.0, YELLOW);
        let mut scene = Scene::default();
        // The room behind, then the near wall: the nearer hides it.
        scene.opaque("room", &room, Placing::at(Vec3::ZERO));
        scene.opaque("wall", &near, Placing::at(Vec3::ZERO));
        // A decal lying on the near wall's plane, and text beside it: both
        // show over what they lie on.
        scene.decal("print", &print, Placing::flat(Vec3::ZERO));
        scene.text("glyph", &glyph, Placing::flat(Vec3::new(8.0, 0.0, 0.0)));
        rig.draw(
            &scene,
            &camera,
            &LightingUniform::default(),
            wgpu::Color::BLACK,
        );
        assert_eq!(rig.pixel(26, 32), RED, "the nearer wall");
        assert_eq!(rig.pixel(29, 32), BLUE, "the decal over it");
        assert_eq!(rig.pixel(40, 32), YELLOW, "the text over it");
        assert_eq!(rig.pixel(18, 32), GREEN, "the room, past the wall's edge");
        assert_eq!(rig.pixel(32, 8), BLACK, "past both, the clear");
    }

    /// The lamp's pass casts what the hook registers: the map holds the
    /// caster's depth where it stands, the floor's where it does not, and
    /// a placement that did not register casts nothing.
    #[test]
    fn the_lamp_casts_what_the_hook_registers() {
        let mut rig = Rig::new([64, 64]);
        let camera = capture::camera();
        let lit = capture::downward();
        let floor = capture::slab(Vec3::ZERO, 60.0, [0x80, 0x80, 0x80, 0xFF]);
        let shelf = capture::slab(Vec3::new(0.0, 0.0, 150.0), 40.0, [0xFF; 4]);
        let leaf = capture::slab(Vec3::new(45.0, 0.0, 150.0), 10.0, [0xFF; 4]);
        let mut scene = Scene::default();
        scene.opaque("floor", &floor, Placing::at(Vec3::ZERO));
        scene.opaque("shelf", &shelf, Placing::at(Vec3::ZERO));
        // A second shelf over the floor, not registered a caster: the map
        // holds the floor's depth in its footprint, not its own.
        scene.opaque("leaf", &leaf, Placing::flat(Vec3::ZERO));
        rig.draw(&scene, &camera, &lit, wgpu::Color::BLACK);
        let map = rig.shadow_map();
        let texel = |x_mm: f32, y_mm: f32| {
            let column = ((1.0 + x_mm / 150.0) * 0.5 * 2048.0) as u32;
            let row = ((1.0 - y_mm / 150.0) * 0.5 * 2048.0) as u32;
            map[(row * 2048 + column) as usize]
        };
        assert!((texel(0.0, 0.0) - 0.5).abs() < 1e-6, "under the shelf");
        assert!(
            (texel(45.0, 0.0) - 1.0).abs() < 1e-6,
            "the leaf cast nothing: the floor's own depth"
        );
        assert!(
            (texel(0.0, 45.0) - 1.0).abs() < 1e-6,
            "past the shelf's edge"
        );
        // Registered a caster, its own depth is what the map holds.
        let leaf = capture::slab(Vec3::new(45.0, 0.0, 150.0), 10.0, [0xFF; 4]);
        let mut scene = Scene::default();
        scene.opaque("floor", &floor, Placing::at(Vec3::ZERO));
        scene.opaque("leaf", &leaf, Placing::flat(Vec3::ZERO).casting(true));
        rig.draw(&scene, &camera, &lit, wgpu::Color::BLACK);
        let map = rig.shadow_map();
        let column = ((1.0 + 45.0 / 150.0) * 0.5 * 2048.0) as u32;
        assert!((map[(1024 * 2048 + column) as usize] - 0.5).abs() < 1e-6);
    }

    /// A standing solid uploads once and moves by its placement alone;
    /// a replaced mesh re-uploads under the same name.
    #[test]
    fn standing_solids_upload_once_and_move_by_transform() {
        let mut rig = Rig::new([64, 64]);
        let camera = capture::camera();
        let lit = LightingUniform::default();
        let clear = wgpu::Color::BLACK;
        let wall = capture::unlit_wall(Vec3::ZERO, 10.0, [0xFF; 4]);
        let scene = capture::scene_of("wall", &wall, Placing::at(Vec3::new(-25.0, 0.0, 0.0)));
        rig.draw(&scene, &camera, &lit, clear);
        assert_eq!(rig.core.uploads.get(), 1, "the standing solid, uploaded");
        // The same mesh, moved: nothing re-uploaded.
        let moved = capture::scene_of("wall", &wall, Placing::at(Vec3::new(25.0, 0.0, 0.0)));
        rig.draw(&moved, &camera, &lit, clear);
        assert_eq!(rig.core.uploads.get(), 1, "moved by transform alone");
        assert_eq!(rig.pixel(52, 32), [0xFF; 4]);
        // Replaced under the same name: one re-upload, and what draws.
        let replaced_mesh = capture::unlit_wall(Vec3::ZERO, 5.0, RED);
        let replaced = capture::scene_of(
            "wall",
            &replaced_mesh,
            Placing::at(Vec3::new(25.0, 0.0, 0.0)),
        );
        rig.draw(&replaced, &camera, &lit, clear);
        assert_eq!(rig.core.uploads.get(), 2, "the replacement, uploaded");
        assert_eq!(rig.pixel(54, 32), RED);
        assert_eq!(rig.pixel(12, 32), BLACK, "moved off of");
    }

    /// A mesh draws the texture its mesh names, and a texture replaced
    /// under its name keeps its meshes' handle: they draw the new texels.
    #[test]
    fn a_mesh_draws_the_texture_its_mesh_names() {
        let mut rig = Rig::new([64, 64]);
        let camera = capture::camera();
        let lit = LightingUniform::default();
        let clear = wgpu::Color::BLACK;
        let red = rig.core.texture("red", [1, 1], &RED);
        let white = capture::unlit_wall(Vec3::ZERO, 10.0, [0xFF; 4]);
        let mut red_wall = capture::unlit_wall(Vec3::ZERO, 10.0, [0xFF; 4]);
        red_wall.texture = red;
        let mut scene = Scene::default();
        scene.opaque("white", &white, Placing::at(Vec3::new(-25.0, 0.0, 0.0)));
        scene.opaque("red", &red_wall, Placing::at(Vec3::new(25.0, 0.0, 0.0)));
        rig.draw(&scene, &camera, &lit, clear);
        assert_eq!(rig.pixel(12, 32), [0xFF; 4], "the white texel");
        assert_eq!(rig.pixel(52, 32), RED, "the red texel, over a white vertex");
        // A name replaced: the same handle, new texels.
        rig.core.texture("red", [1, 1], &GREEN);
        rig.draw(&scene, &camera, &lit, clear);
        assert_eq!(rig.pixel(52, 32), GREEN, "the replaced texels");
    }

    /// What a frame of the core looks like, headless: `device-<backend>.ppm`
    /// for each backend that answers, and the lamp's map beside it. Not
    /// for CI: it wants a device.
    ///
    /// ```sh
    /// TYPEWRITER_RENDER_SNAPSHOT=<folder> cargo test -p typewriter-render -- \
    ///     --ignored --nocapture scene_snapshot
    /// ```
    #[test]
    #[ignore]
    fn scene_snapshot() {
        let Some(folder) = std::env::var_os("TYPEWRITER_RENDER_SNAPSHOT") else {
            return;
        };
        let folder = std::path::PathBuf::from(folder);
        let wood = [0x9C, 0x6C, 0x43, 0xFF];
        let mut rig = Rig::new([128, 128]);
        let camera = capture::camera_at(Vec2::splat(128.0));
        let room = capture::wall(Vec3::new(0.0, -300.0, 0.0), 55.0, wood);
        let near = capture::wall(Vec3::ZERO, 25.0, [0xB4, 0xB8, 0xB4, 0xFF]);
        let shelf = capture::slab(Vec3::new(0.0, 0.0, 40.0), 30.0, [0x80, 0x40, 0x30, 0xFF]);
        let print = capture::wall(Vec3::ZERO, 12.0, [0xF4, 0xF4, 0xEC, 0xFF]);
        let glyph = capture::wall(Vec3::ZERO, 8.0, [0x20, 0x20, 0x20, 0xFF]);
        let mut scene = Scene::default();
        scene.opaque("room", &room, Placing::at(Vec3::ZERO));
        scene.opaque("wall", &near, Placing::at(Vec3::ZERO));
        // The one caster with area under the straight-down lamp: the map's
        // shot shows its depth where the walls' own is edge-on.
        scene.opaque("shelf", &shelf, Placing::at(Vec3::ZERO));
        scene.decal("print", &print, Placing::flat(Vec3::new(-4.0, 0.0, 6.0)));
        scene.text("glyph", &glyph, Placing::flat(Vec3::new(12.0, 0.0, 6.0)));
        rig.draw(&scene, &camera, &capture::downward(), wgpu::Color::BLACK);
        let (width, height) = (rig.size[0], rig.size[1]);
        let mut ppm = format!("P6\n{width} {height}\n255\n").into_bytes();
        for texel in rig.pixels() {
            ppm.extend_from_slice(&texel[..3]);
        }
        let name = format!("device-{:?}.ppm", rig.backend).to_lowercase();
        std::fs::write(folder.join(name), ppm).unwrap();
        // The lamp's map, dark where its casters stand.
        let side = SHADOW_MAP as usize;
        let mut ppm = format!("P6\n{side} {side}\n255\n").into_bytes();
        for depth in rig.shadow_map() {
            let v = (depth.clamp(0.0, 1.0) * 255.0) as u8;
            ppm.extend_from_slice(&[v, v, v]);
        }
        std::fs::write(folder.join("map.ppm"), ppm).unwrap();
        eprintln!("captured {:?}", rig.backend);
    }
}
