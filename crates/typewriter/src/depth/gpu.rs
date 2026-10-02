//! The GPU's half of the depth pass: [`install`]s the two [`Pipelines`] on
//! the window's renderer, [`pack`] lays a frame's solids into vertex and
//! index bytes exactly as `depth.wgsl` reads them, and [`Gpu`] — the paint
//! callback [`super::end`] installs — draws them. Decals get their lift
//! from the pipeline's [`DepthBiasState`](wgpu::DepthBiasState): the
//! `decal_bias` twins are what the CPU rasterizer adds instead.

use std::sync::{Arc, Weak};

use bytemuck::{Pod, Zeroable};
use eframe::egui::mutex::RwLock;
use eframe::egui::{Context, TextureId, Vec2};
use eframe::egui_wgpu::{self, CallbackResources, CallbackTrait, Renderer, ScreenDescriptor};
use eframe::wgpu;
use glam::Vec3;

use super::lighting::LIGHTING_BYTES;
use super::solids::{Layer, Solids};
use super::{CAMERA_BYTES, Pass};

pub(super) const SHADER: &str = include_str!("depth.wgsl");
/// egui's font atlas, which text and plain fills (`WHITE_UV`) draw from.
pub(super) const FONTS: TextureId = TextureId::Managed(0);

/// One vertex, exactly as `vs_main` in `depth.wgsl` reads it: the
/// millimetres it stands at, uv, colour, the way it faces, its material's
/// parameter, material and highlight. The compiler keeps its size and order
/// true to `vertex_attr_array` and the shader; little-endian bytes, like
/// both backends read.
#[repr(C)]
#[derive(Debug, Copy, Clone, PartialEq, Pod, Zeroable)]
struct DepthVertex {
    at: Vec3,
    uv: [f32; 2],
    color: [u8; 4],
    vector: Vec3,
    spec: f32,
    material: u32,
    shine: [u8; 4],
}

/// What the GPU indexes vertices by, in bytes.
const VERTEX_BYTES: u64 = std::mem::size_of::<DepthVertex>() as u64;

/// The depth buffer's basic unit for [`super::DEPTH_BITS`], which is what a
/// `wgpu::DepthBiasState` constant is counted in: what Vulkan and D3D take
/// as the minimum representable interval of a 32-bit float depth, as Mesa
/// does for GL's `glPolygonOffset` units.
#[cfg(test)]
const DEPTH_UNIT: f32 = 1.0 / (1u32 << 24) as f32;
/// The decal pass's constant bias, negative to lift: a
/// fragment's depth is biased by the sum *added* to it, so a negative one
/// moves a decal toward the eye. Worth the old 0.25 mm at the paper's
/// distance: enough for a decal's own mesh to show over the coarser mesh it
/// lies on, where a face turns to the eye.
const DECAL_BIAS_UNITS: i32 = -285;
/// The decal pass's slope-scaled bias: times the steepest depth a decal
/// triangle's runs per screen pixel. Where a face turns away, that is where
/// lying on it needs the most lift.
const DECAL_BIAS_SLOPE: f32 = -2.0;
/// The cap on their sum's magnitude, in absolute depth: a little over the
/// constant, so a steep face gets more lift than the paper does but not
/// near again what it would in millimetres. Short of the window the guide's
/// glass keeps behind the ribbon (`printing_point`'s test); it also bounds
/// the bias on any backend that takes the constant in absolute depth too
/// (Metal).
const DECAL_BIAS_CLAMP: f32 = 2.0e-5;

/// How the decal pipeline lifts a triangle whose depth runs `slope` per
/// screen pixel: the magnitude of the negative bias the GPU adds, toward
/// the eye, so what lies on a face shows over it. The GPU applies this
/// through the pipeline's `DepthBiasState`; `rasterize` applies the same
/// through its `decal_slope`.
#[cfg(test)]
pub fn decal_bias(slope: f32) -> f32 {
    (DECAL_BIAS_UNITS as f32 * DEPTH_UNIT + DECAL_BIAS_SLOPE * slope)
        .clamp(-DECAL_BIAS_CLAMP, 0.0)
        .abs()
}

/// Group 0's layout: a texture and a sampler, as egui's are, so its
/// textures' bind groups serve the pass — and a stand-in's in a test.
fn texture_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("depth_textures"),
        entries: &[
            wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    multisampled: false,
                    sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    view_dimension: wgpu::TextureViewDimension::D2,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 1,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                count: None,
            },
        ],
    })
}

impl Pipelines {
    /// The depth pass's two pipelines for `format`, needing only a device:
    /// [`install`]s on the window's, and the GPU snapshot's on its own.
    /// `clamp` is whether the backend caps the decal bias.
    fn build(device: &wgpu::Device, format: wgpu::TextureFormat, clamp: bool) -> Self {
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("depth"),
            source: wgpu::ShaderSource::Wgsl(SHADER.into()),
        });
        let textures = texture_layout(device);
        let lighting_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("depth_lighting"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: wgpu::BufferSize::new(LIGHTING_BYTES),
                },
                count: None,
            }],
        });
        let lighting = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("depth_lighting"),
            size: LIGHTING_BYTES,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let lighting_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("depth_lighting"),
            layout: &lighting_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: lighting.as_entire_binding(),
            }],
        });
        let camera_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("depth_camera"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: wgpu::BufferSize::new(CAMERA_BYTES),
                },
                count: None,
            }],
        });
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("depth"),
            bind_group_layouts: &[
                Some(&textures),
                Some(&lighting_layout),
                Some(&camera_layout),
            ],
            immediate_size: 0,
        });
        let decal_bias = if clamp {
            wgpu::DepthBiasState {
                constant: DECAL_BIAS_UNITS,
                slope_scale: DECAL_BIAS_SLOPE,
                clamp: DECAL_BIAS_CLAMP,
            }
        } else {
            wgpu::DepthBiasState {
                constant: DECAL_BIAS_UNITS,
                slope_scale: 0.0,
                clamp: 0.0,
            }
        };
        let pipeline = |layer: Layer| {
            let opaque = layer == Layer::Opaque;
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some(if opaque {
                    "depth_opaque"
                } else {
                    "depth_decal"
                }),
                layout: Some(&layout),
                vertex: wgpu::VertexState {
                    module: &module,
                    entry_point: Some("vs_main"),
                    buffers: &[Some(wgpu::VertexBufferLayout {
                        array_stride: VERTEX_BYTES,
                        step_mode: wgpu::VertexStepMode::Vertex,
                        attributes: &wgpu::vertex_attr_array![
                            0 => Float32x3,
                            1 => Float32x2,
                            2 => Uint32,
                            3 => Float32x3,
                            4 => Float32,
                            5 => Uint32,
                            6 => Uint32,
                        ],
                    })],
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                },
                primitive: wgpu::PrimitiveState::default(),
                depth_stencil: Some(wgpu::DepthStencilState {
                    format: wgpu::TextureFormat::Depth32Float,
                    depth_write_enabled: Some(opaque),
                    depth_compare: Some(if opaque {
                        wgpu::CompareFunction::Less
                    } else {
                        wgpu::CompareFunction::LessEqual
                    }),
                    stencil: wgpu::StencilState::default(),
                    // Decals lie on faces they match exactly: the pass lifts
                    // them, so they show without standing nearer in millimetres.
                    bias: if opaque {
                        wgpu::DepthBiasState::default()
                    } else {
                        decal_bias
                    },
                }),
                multisample: wgpu::MultisampleState::default(),
                fragment: Some(wgpu::FragmentState {
                    module: &module,
                    entry_point: Some(if format.is_srgb() {
                        "fs_main_linear_framebuffer"
                    } else {
                        "fs_main_gamma_framebuffer"
                    }),
                    // Premultiplied, as egui blends.
                    targets: &[Some(wgpu::ColorTargetState {
                        format,
                        blend: Some(wgpu::BlendState {
                            color: wgpu::BlendComponent {
                                src_factor: wgpu::BlendFactor::One,
                                dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                                operation: wgpu::BlendOperation::Add,
                            },
                            alpha: wgpu::BlendComponent {
                                src_factor: wgpu::BlendFactor::OneMinusDstAlpha,
                                dst_factor: wgpu::BlendFactor::One,
                                operation: wgpu::BlendOperation::Add,
                            },
                        }),
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                }),
                multiview_mask: None,
                cache: None,
            })
        };
        Self {
            opaque: pipeline(Layer::Opaque),
            decal: pipeline(Layer::Decal),
            lighting,
            lighting_group,
            camera_layout,
            // The window's renderer keeps these; a test's device has none.
            renderer: Weak::new(),
            buffers: None,
            draws: Vec::new(),
        }
    }
}

/// Kept by egui's renderer. Its textures are looked up while drawing, so
/// each frame's are the ones drawn: the font atlas grows as glyphs come.
struct Pipelines {
    opaque: wgpu::RenderPipeline,
    decal: wgpu::RenderPipeline,
    /// The frame's lighting, written each frame.
    lighting: wgpu::Buffer,
    lighting_group: wgpu::BindGroup,
    /// The frame's camera, for the buffers' bind group: the vertex shader
    /// projects with it.
    camera_layout: wgpu::BindGroupLayout,
    /// Weak: the renderer keeps these.
    renderer: Weak<RwLock<Renderer>>,
    /// Refilled each frame, grown when too small: one pass a frame.
    buffers: Option<Buffers>,
    /// This frame's, into `buffers`.
    draws: Vec<Draw>,
}

/// A frame's pass, for egui's renderer.
pub(super) struct Gpu {
    pub(super) pass: Arc<Pass>,
    /// For the font atlas's size once the frame is over.
    pub(super) ctx: Context,
}

struct Buffers {
    vertices: wgpu::Buffer,
    indices: wgpu::Buffer,
    /// The frame's camera, refilled each frame, and its bind group.
    camera: wgpu::Buffer,
    camera_group: wgpu::BindGroup,
}

impl Buffers {
    /// Room for at least `vertex_bytes` and `index_bytes`, to the next
    /// power of two: a growing page grows them seldom. The camera is one
    /// fixed uniform, bound once a pass.
    fn new(
        device: &wgpu::Device,
        camera_layout: &wgpu::BindGroupLayout,
        vertex_bytes: u64,
        index_bytes: u64,
    ) -> Self {
        let buffer = |label, bytes: u64, usage| {
            device.create_buffer(&wgpu::BufferDescriptor {
                label: Some(label),
                size: bytes.next_power_of_two(),
                usage: usage | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            })
        };
        let camera = buffer("depth_camera", CAMERA_BYTES, wgpu::BufferUsages::UNIFORM);
        let camera_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("depth_camera"),
            layout: camera_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: camera.as_entire_binding(),
            }],
        });
        Self {
            vertices: buffer("depth_vertices", vertex_bytes, wgpu::BufferUsages::VERTEX),
            indices: buffer("depth_indices", index_bytes, wgpu::BufferUsages::INDEX),
            camera,
            camera_group,
        }
    }

    fn holds(&self, vertex_bytes: u64, index_bytes: u64) -> bool {
        self.vertices.size() >= vertex_bytes && self.indices.size() >= index_bytes
    }
}

/// `solids` as the GPU takes them, for the font atlas `fonts` texels as it
/// is once the frame is over: vertices and indices, and a draw for each
/// solid, opaque first. Positions go out in absolute machine millimetres;
/// the `Camera` uniform projects them.
fn pack(solids: &Solids, fonts: [usize; 2]) -> (Vec<DepthVertex>, Vec<u32>, Vec<Draw>) {
    let all = || {
        let layers = solids.layers().into_iter();
        layers.flat_map(|(layer, solids)| solids.iter().map(move |solid| (layer, solid)))
    };
    let vertex_count: usize = all().map(|(_, solid)| solid.mesh.vertices.len()).sum();
    let index_count: usize = all().map(|(_, solid)| solid.mesh.indices.len()).sum();
    let mut vertices = Vec::with_capacity(vertex_count);
    let mut indices = Vec::with_capacity(index_count);
    let mut draws = Vec::new();
    let (mut first, mut base) = (0, 0);
    for (layer, solid) in all() {
        let texels = texel_scale(solid.mesh.texture_id, fonts);
        let placed = solid.places.iter().zip(&solid.shades);
        for (vertex, (&at, shade)) in solid.mesh.vertices.iter().zip(placed) {
            let uv = vertex.uv.to_vec2() * texels;
            let (material, vector, spec, shine) = shade.parts();
            vertices.push(DepthVertex {
                at,
                uv: [uv.x, uv.y],
                color: vertex.color.to_array(),
                vector,
                spec,
                material,
                shine: shine.to_array(),
            });
        }
        indices.extend_from_slice(&solid.mesh.indices);
        // Safe casts: a frame's vertices fit egui's own u32 indices, and
        // far fewer than i32 counts.
        let count = solid.mesh.indices.len() as u32;
        draws.push(Draw {
            layer,
            texture: solid.mesh.texture_id,
            indices: first..first + count,
            base,
        });
        first += count;
        base += solid.mesh.vertices.len() as i32;
    }
    (vertices, indices, draws)
}

/// What a solid's uvs in `texture` are scaled by to normalise them, the
/// font atlas `fonts` texels.
pub(super) fn texel_scale(texture: TextureId, fonts: [usize; 2]) -> Vec2 {
    if texture != FONTS {
        return Vec2::splat(1.0);
    }
    // Safe casts: a texture's side in texels.
    let [width, height] = fonts.map(|side| side.max(1) as f32);
    Vec2::new(1.0 / width, 1.0 / height)
}

/// A solid's run of `indices`, from `base` in the vertices.
struct Draw {
    layer: Layer,
    texture: TextureId,
    indices: std::ops::Range<u32>,
    base: i32,
}

impl CallbackTrait for Gpu {
    fn prepare(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        _screen: &ScreenDescriptor,
        _encoder: &mut wgpu::CommandEncoder,
        resources: &mut CallbackResources,
    ) -> Vec<wgpu::CommandBuffer> {
        let Some(pipelines) = resources.get_mut::<Pipelines>() else {
            return Vec::new();
        };
        // The frame is over: the atlas is as large as it gets this frame.
        let fonts = self.ctx.fonts(|fonts| fonts.font_image_size());
        let (vertices, indices, draws) = pack(&self.pass.solids, fonts);
        queue.write_buffer(
            &pipelines.lighting,
            0,
            bytemuck::bytes_of(&self.pass.lighting.uniform()),
        );
        // Safe casts: byte counts.
        let (vertex_bytes, index_bytes) = (
            vertices.len() as u64 * VERTEX_BYTES,
            indices.len() as u64 * 4,
        );
        let buffers = match pipelines.buffers.take() {
            Some(buffers) if buffers.holds(vertex_bytes, index_bytes) => buffers,
            _ => Buffers::new(device, &pipelines.camera_layout, vertex_bytes, index_bytes),
        };
        queue.write_buffer(
            &buffers.camera,
            0,
            bytemuck::bytes_of(&self.pass.camera.uniform()),
        );
        queue.write_buffer(&buffers.vertices, 0, bytemuck::cast_slice(&vertices));
        queue.write_buffer(&buffers.indices, 0, bytemuck::cast_slice(&indices));
        pipelines.buffers = Some(buffers);
        pipelines.draws = draws;
        Vec::new()
    }

    fn paint(
        &self,
        _info: eframe::egui::PaintCallbackInfo,
        render_pass: &mut wgpu::RenderPass<'static>,
        resources: &CallbackResources,
    ) {
        let Some(pipelines) = resources.get::<Pipelines>() else {
            return;
        };
        let Some(buffers) = &pipelines.buffers else {
            return;
        };
        let Some(renderer) = pipelines.renderer.upgrade() else {
            return;
        };
        // egui's renderer is reading too, on this thread; nothing writes to
        // it while it draws.
        let renderer = renderer.read();
        render_pass.set_vertex_buffer(0, buffers.vertices.slice(..));
        render_pass.set_index_buffer(buffers.indices.slice(..), wgpu::IndexFormat::Uint32);
        render_pass.set_bind_group(1, &pipelines.lighting_group, &[]);
        render_pass.set_bind_group(2, &buffers.camera_group, &[]);
        for draw in &pipelines.draws {
            let Some(texture) = renderer.texture(&draw.texture) else {
                continue;
            };
            render_pass.set_pipeline(match draw.layer {
                Layer::Opaque => &pipelines.opaque,
                Layer::Decal => &pipelines.decal,
            });
            render_pass.set_bind_group(0, &texture.bind_group, &[]);
            render_pass.draw_indexed(draw.indices.clone(), draw.base, 0..1);
        }
    }
}

/// Readies `render_state`'s window, with a `DEPTH_BITS` depth buffer, to
/// draw solids.
pub fn install(ctx: &Context, render_state: &egui_wgpu::RenderState) {
    // Devices without depth-bias clamping (some software renderers) cannot
    // cap the slope term, and an uncapped one would float a decal far off
    // its face: there the pass lifts by the constant alone.
    let clamp = render_state
        .adapter
        .get_downlevel_capabilities()
        .flags
        .contains(wgpu::DownlevelFlags::DEPTH_BIAS_CLAMP);
    let mut pipelines = Pipelines::build(&render_state.device, render_state.target_format, clamp);
    pipelines.renderer = Arc::downgrade(&render_state.renderer);
    render_state
        .renderer
        .write()
        .callback_resources
        .insert(pipelines);
    ctx.data_mut(|data| data.insert_temp(super::installed_id(), true));
}

#[cfg(test)]
mod tests {
    use eframe::egui::epaint::{TessellationOptions, Tessellator};
    use eframe::egui::{Color32, Mesh, Pos2, Rect, Shape, pos2, vec2};

    use super::*;
    use crate::depth::lighting::Shade;
    use crate::depth::solids::{Placing, Solid};

    #[test]
    fn packing_lays_each_solid_after_the_last() {
        let mut solids = Solids::default();
        let triangle = |colour: Color32| {
            let mut mesh = Mesh::default();
            for (x, y) in [(800.0, 500.0), (1600.0, 0.0), (0.0, 1000.0)] {
                mesh.colored_vertex(pos2(x, y), colour);
            }
            mesh.add_triangle(0, 1, 2);
            mesh
        };
        let mut lit = Solid::unlit(triangle(Color32::RED), vec![Vec3::new(1.0, 2.0, 3.5); 3]);
        lit.shades = vec![Shade::Matte(Vec3::Z); 3];
        solids.push(Layer::Opaque, lit);
        solids.add(Layer::Decal, triangle(Color32::BLUE), |_| {
            Some(Placing::default())
        });
        let (vertices, indices, draws) = pack(&solids, [64, 32]);
        assert_eq!(vertices.len(), 6);
        assert_eq!(indices.len(), 6);
        let runs: Vec<_> = draws
            .iter()
            .map(|d| (d.layer, d.indices.clone(), d.base))
            .collect();
        assert_eq!(runs, [(Layer::Opaque, 0..3, 0), (Layer::Decal, 3..6, 3)]);
        // Where each lit vertex stands, in machine millimetres: the shader
        // projects these, and the camera's matrix decides the depth.
        assert_eq!(vertices[0].at, Vec3::new(1.0, 2.0, 3.5));
        assert_eq!(vertices[2].at, Vec3::new(1.0, 2.0, 3.5));
        assert_eq!(vertices[0].color, Color32::RED.to_array());
        // Its normal, parameter, material and shine.
        assert_eq!(vertices[0].vector, Vec3::Z);
        assert_eq!(vertices[0].spec, 0.0);
        assert_eq!(vertices[0].material, 1);
        assert_eq!(vertices[0].shine, Color32::WHITE.to_array());
        // The unlit decal, standing at the printing point.
        assert_eq!(vertices[3].at, Vec3::ZERO);
        assert_eq!(vertices[3].material, 0);
    }

    #[test]
    fn the_decal_bias_lifts_toward_the_eye_and_caps_its_lift() {
        // On a face turned to the eye: the constant alone.
        let constant = decal_bias(0.0);
        assert!((constant - 285.0 / (1u32 << 24) as f32).abs() < 1e-9);
        // The steeper a decal's triangle, the more it is lifted. Until the
        // cap, which keeps it off what stands behind.
        assert!(decal_bias(1e-5) > constant);
        assert_eq!(decal_bias(1e-3), DECAL_BIAS_CLAMP);
    }

    #[test]
    fn polished_and_chrome_carry_their_shine_and_their_place_down() {
        let mut mesh = Mesh::default();
        for (x, y) in [(800.0, 500.0), (1600.0, 0.0), (0.0, 1000.0)] {
            mesh.colored_vertex(pos2(x, y), Color32::RED);
        }
        mesh.add_triangle(0, 1, 2);
        let shine = Color32::from_rgb(0xDC, 0xDE, 0xDA);
        let polished = Shade::Polished {
            normal: Vec3::Z,
            shine,
            sharpness: 24.0,
        };
        let mut solids = Solids::default();
        let mut lit = Solid::unlit(mesh.clone(), vec![Vec3::ZERO; 3]);
        lit.shades = vec![polished; 3];
        solids.push(Layer::Opaque, lit);
        let mut down = Solid::unlit(mesh, vec![Vec3::ZERO; 3]);
        down.shades = vec![Shade::Chrome(0.75); 3];
        solids.push(Layer::Opaque, down);
        let (vertices, _, _) = pack(&solids, [64, 32]);
        // A polished vertex: its normal, its sharpness, its material, its shine.
        assert_eq!(vertices[0].vector, Vec3::Z);
        assert_eq!(vertices[0].spec, 24.0);
        assert_eq!(vertices[0].material, 2);
        assert_eq!(vertices[0].shine, shine.to_array());
        // A chrome one: how far down it lies, and no shine of its own.
        assert_eq!(vertices[3].spec, 0.75);
        assert_eq!(vertices[3].material, 4);
        assert_eq!(vertices[3].shine, Color32::WHITE.to_array());
    }

    #[test]
    fn text_in_depth_keeps_texel_uvs_and_shapes_read_the_same() {
        // What `add_shapes` and `pack` lean on in epaint: a one-texel atlas
        // leaves glyphs' uvs in texels; plain fills use (0, 0) either way.
        let ctx = Context::default();
        let mut galley = None;
        let mut output = ctx.run_ui(Default::default(), |ui| {
            let font = eframe::egui::FontId::monospace(40.0);
            galley = Some(
                ui.painter()
                    .layout_no_wrap("Wg".into(), font, Color32::BLACK),
            );
        });
        output.textures_delta.clear();
        let (galley, fonts) = (galley.unwrap(), ctx.fonts(|f| f.font_image_size()));
        let tessellate = |atlas: [usize; 2]| {
            let options = TessellationOptions::default();
            let mut tessellator = Tessellator::new(1.0, options, atlas, Vec::new());
            let mut mesh = Mesh::default();
            let text = Shape::galley(pos2(0.0, 0.0), galley.clone(), Color32::BLACK);
            tessellator.tessellate_shape(text, &mut mesh);
            mesh
        };
        let (texels, normalised) = (tessellate([1, 1]), tessellate(fonts));
        assert!(!texels.is_empty());
        assert_eq!(texels.texture_id, FONTS);
        let scale = texel_scale(FONTS, fonts);
        for (t, n) in texels.vertices.iter().zip(&normalised.vertices) {
            let uv = t.uv.to_vec2() * scale;
            assert!(
                (uv - n.uv.to_vec2()).length() < 1e-6,
                "{:?} {:?}",
                t.uv,
                n.uv
            );
        }
        assert_eq!(eframe::egui::epaint::WHITE_UV, pos2(0.0, 0.0));
    }

    /// Polls `future` out: natively wgpu's futures do their work when
    /// polled, so no executor is wanted.
    fn wait<T>(future: impl std::future::Future<Output = T>) -> T {
        use std::task::{Context, Poll};
        let mut cx = Context::from_waker(std::task::Waker::noop());
        let mut future = std::pin::pin!(future);
        loop {
            match future.as_mut().poll(&mut cx) {
                Poll::Ready(value) => return value,
                Poll::Pending => std::thread::yield_now(),
            }
        }
    }

    /// The whole machine painted as a frame, in the tall snapshot's view,
    /// zoom and typing line, its pass taken as the CPU twin's `rasterize`
    /// does, with the font atlas's size as the draw reads it.
    fn machine_pass() -> (Pass, [usize; 2]) {
        let ctx = Context::default();
        let view = Rect::from_min_size(Pos2::ZERO, vec2(1400.0, 2400.0));
        let sm9 = include_str!("../../../../profiles/olympia-sm9.toml");
        let profile = typewriter_core::Profile::from_toml_str(sm9).unwrap();
        let metrics =
            typewriter_ui::draw::Metrics::new(&profile, typewriter_ui::draw::points_per_inch(100));
        let typing_y = view.height() * crate::machine::typing_line_height(view, &metrics, 100);
        let middle = view.center().x;
        let mut output = ctx.run_ui(
            eframe::egui::RawInput {
                screen_rect: Some(view),
                ..Default::default()
            },
            |ui| {
                let painter = ui.painter();
                crate::machine::paint_behind(painter, view, &metrics, typing_y, middle, 0.0);
                crate::machine::paint_knobs(
                    painter,
                    view,
                    &metrics,
                    typing_y,
                    (middle, 0.0),
                    |_| false,
                );
                crate::machine::paint_front(painter, view, &metrics, typing_y);
            },
        );
        output.textures_delta.clear();
        let fonts = ctx.fonts(|fonts| fonts.font_image_size());
        let pass = output
            .shapes
            .iter()
            .find_map(|shape| match &shape.shape {
                Shape::Callback(callback) => {
                    callback.callback.downcast_ref::<Pass>().map(|pass| Pass {
                        solids: pass.solids.clone(),
                        lighting: pass.lighting,
                        camera: pass.camera,
                    })
                }
                _ => None,
            })
            .expect("the machine ends its frame with a pass");
        (pass, fonts)
    }

    /// Draws `pass` on a headless device through [`Pipelines::build`] and
    /// the packed draws, as `Gpu::paint` does, into a gamma `Rgba8Unorm`
    /// the size of the view with a `Depth32Float` beside it, cleared black
    /// like the twin's raster; `white` stands for the font atlas, so plain
    /// fills and shadows read true and glyphs not. Returns the pixels as a
    /// PPM: the snapshot tool's twin goes by `rasterize` instead.
    fn gpu_capture(
        backends: wgpu::Backends,
        pass: &Pass,
        fonts: [usize; 2],
    ) -> Result<(wgpu::Backend, Vec<u8>), String> {
        let (width, height) = (pass.camera.points.x as u32, pass.camera.points.y as u32);
        let mut desc = wgpu::InstanceDescriptor::new_without_display_handle();
        desc.backends = backends;
        let instance = wgpu::Instance::new(desc);
        let adapter =
            wait(instance.request_adapter(&Default::default())).map_err(|why| why.to_string())?;
        let info = adapter.get_info();
        eprintln!("adapter: {:?} {}", info.backend, info.name);
        let backend = info.backend;
        let clamp = adapter
            .get_downlevel_capabilities()
            .flags
            .contains(wgpu::DownlevelFlags::DEPTH_BIAS_CLAMP);
        let (device, queue) = wait(adapter.request_device(&wgpu::DeviceDescriptor::default()))
            .map_err(|why| why.to_string())?;
        let pipelines = Pipelines::build(&device, wgpu::TextureFormat::Rgba8Unorm, clamp);
        let textures = texture_layout(&device);
        // A white texel: colour reaches the shader through vertices, not
        // the texture — but the glyph regions, which the twin shares.
        let white = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("gpu_snapshot_white"),
            size: wgpu::Extent3d {
                width: 1,
                height: 1,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        queue.write_texture(
            white.as_image_copy(),
            &[0xFF; 4],
            Default::default(),
            wgpu::Extent3d {
                width: 1,
                height: 1,
                depth_or_array_layers: 1,
            },
        );
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor::default());
        let white_view = white.create_view(&Default::default());
        let white_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("gpu_snapshot_white"),
            layout: &textures,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&white_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
            ],
        });
        let (vertices, indices, draws) = pack(&pass.solids, fonts);
        let buffers = Buffers::new(
            &device,
            &pipelines.camera_layout,
            vertices.len() as u64 * VERTEX_BYTES,
            indices.len() as u64 * 4,
        );
        queue.write_buffer(
            &pipelines.lighting,
            0,
            bytemuck::bytes_of(&pass.lighting.uniform()),
        );
        queue.write_buffer(
            &buffers.camera,
            0,
            bytemuck::bytes_of(&pass.camera.uniform()),
        );
        queue.write_buffer(&buffers.vertices, 0, bytemuck::cast_slice(&vertices));
        queue.write_buffer(&buffers.indices, 0, bytemuck::cast_slice(&indices));
        let target = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("gpu_snapshot_target"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let depth = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("gpu_snapshot_depth"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Depth32Float,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });
        let target_view = target.create_view(&Default::default());
        let depth_view = depth.create_view(&Default::default());
        let mut encoder = device.create_command_encoder(&Default::default());
        {
            let mut render = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("gpu_snapshot"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &target_view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &depth_view,
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
            render.set_vertex_buffer(0, buffers.vertices.slice(..));
            render.set_index_buffer(buffers.indices.slice(..), wgpu::IndexFormat::Uint32);
            render.set_bind_group(1, &pipelines.lighting_group, &[]);
            render.set_bind_group(2, &buffers.camera_group, &[]);
            for draw in &draws {
                render.set_pipeline(match draw.layer {
                    Layer::Opaque => &pipelines.opaque,
                    Layer::Decal => &pipelines.decal,
                });
                render.set_bind_group(0, &white_group, &[]);
                render.draw_indexed(draw.indices.clone(), draw.base, 0..1);
            }
        }
        queue.submit([encoder.finish()]);
        // The copy wants rows a multiple of 256 bytes.
        let row_bytes = (width * 4).next_multiple_of(256);
        let readback = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("gpu_snapshot_readback"),
            size: u64::from(row_bytes) * u64::from(height),
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let mut encoder = device.create_command_encoder(&Default::default());
        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture: &target,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &readback,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(row_bytes),
                    rows_per_image: Some(height),
                },
            },
            wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );
        queue.submit([encoder.finish()]);
        let (sender, receiver) = std::sync::mpsc::sync_channel(1);
        readback
            .slice(..)
            .map_async(wgpu::MapMode::Read, move |mapped| {
                let _ = sender.send(mapped);
            });
        let _ = device.poll(wgpu::PollType::wait_indefinitely());
        receiver
            .recv()
            .map_err(|_| "no mapping")?
            .map_err(|why| why.to_string())?;
        let mapped = readback
            .get_mapped_range(..)
            .map_err(|why| why.to_string())?;
        let mut ppm = format!("P6\n{width} {height}\n255\n").into_bytes();
        for row in 0..height {
            let start = usize::try_from(row)
                .unwrap()
                .checked_mul(row_bytes as usize)
                .unwrap();
            let row = &mapped[start..start + width as usize * 4];
            for texel in row.as_chunks::<4>().0 {
                ppm.extend_from_slice(&texel[..3]);
            }
        }
        drop(mapped);
        readback.unmap();
        Ok((backend, ppm))
    }

    /// The real GPU path, headless: paints the whole machine into one pass
    /// and draws it on every backend's device that answers, writing
    /// `tall-<backend>.ppm` for each into `$TYPEWRITER_GPU_SNAPSHOT`, to
    /// look at what a screen shows where the twin's `rasterize` does
    /// not. Not for CI: it wants a device.
    ///
    /// ```sh
    /// TYPEWRITER_GPU_SNAPSHOT=<folder> xvfb-run -a cargo test -p typewriter -- \
    ///     --ignored --nocapture gpu_snapshot
    /// ```
    #[test]
    #[ignore]
    fn gpu_snapshot() {
        let Some(folder) = std::env::var_os("TYPEWRITER_GPU_SNAPSHOT") else {
            return;
        };
        let folder = std::path::PathBuf::from(folder);
        let (pass, fonts) = machine_pass();
        let mut captured = 0;
        for backends in [wgpu::Backends::VULKAN, wgpu::Backends::GL] {
            match gpu_capture(backends, &pass, fonts) {
                Ok((backend, ppm)) => {
                    let name = format!("tall-{:?}.ppm", backend).to_lowercase();
                    std::fs::write(folder.join(name), ppm).unwrap();
                    eprintln!("captured {backend:?}");
                    captured += 1;
                }
                Err(why) => eprintln!("{backends:?}: no device: {why}"),
            }
        }
        assert!(captured > 0, "no headless device answered");
    }
}
