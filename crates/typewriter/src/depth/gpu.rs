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
use super::{CAMERA_BYTES, Pass, SHADOW_MAP};

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

/// Group 3's layout: the shadow map the lamp cast — a filterable 16-bit
/// float map, its texels blended by the sampler — which the lit shaders read.
fn shadow_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("depth_shadow"),
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
    /// The depth pass's two lighting pipelines and the lamp's own depth-only
    /// one, for `format`, needing only a device: [`install`]s on the window's,
    /// and the GPU snapshot's on its own. `clamp` is whether the backend caps
    /// the decal bias.
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
                // The vertex shader reads it too: the lamp's pass projects
                // its vertices by `r_lighting`'s shadow rows.
                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
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
        // The lamp's map, once: the same side every frame's `Lighting` reads.
        // A 16-bit float map, since GL cannot load a depth texture's texels:
        // the lamp's fragment writes the depth into its single channel, and
        // the sampler blends the texels. `R16Float` is core WebGPU —
        // filterable and renderable wherever wgpu runs, no feature asked —
        // while its fixed-point cousins need `TEXTURE_FORMAT_16BIT_NORM` and
        // are not renderable at all in wgpu 30. A float's step is at worst a
        // texel's pitch over the map's depth run: the map's own resolution.
        // The view and bind group hold the textures, so the local handles
        // end here.
        let shadow = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("depth_shadow"),
            size: wgpu::Extent3d {
                width: SHADOW_MAP,
                height: SHADOW_MAP,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::R16Float,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let shadow_view = shadow.create_view(&Default::default());
        let lamp_depth = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("depth_shadow_test"),
            size: wgpu::Extent3d {
                width: SHADOW_MAP,
                height: SHADOW_MAP,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Depth32Float,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });
        let lamp_depth_view = lamp_depth.create_view(&Default::default());
        let shadows = shadow_layout(device);
        let shadow_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("depth_shadow"),
            // Tap coordinates fall outside the map by design: an edge the
            // lamp found clear holds its shadow off, a caster reaching the
            // edge keeps hiding what lies beyond.
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        let shadow_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("depth_shadow"),
            layout: &shadows,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&shadow_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&shadow_sampler),
                },
            ],
        });
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("depth"),
            bind_group_layouts: &[
                Some(&textures),
                Some(&lighting_layout),
                Some(&camera_layout),
                Some(&shadows),
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
        // The lamp's own pass: the opaque solids again, projected by the
        // shadow rows the frame's `Lighting` carries, into the map. No
        // faces are culled: a plate casts by whatever side the light meets.
        // The depth test runs on a throwaway depth texture; what survives it
        // the fragment writes into the map.
        let lamp_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("depth_lamp"),
            bind_group_layouts: &[None, Some(&lighting_layout), None, None],
            immediate_size: 0,
        });
        let lamp = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("depth_lamp"),
            layout: Some(&lamp_layout),
            vertex: wgpu::VertexState {
                module: &module,
                entry_point: Some("vs_lamp"),
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
            primitive: wgpu::PrimitiveState {
                cull_mode: None,
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Depth32Float,
                depth_write_enabled: Some(true),
                depth_compare: Some(wgpu::CompareFunction::Less),
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            fragment: Some(wgpu::FragmentState {
                module: &module,
                entry_point: Some("fs_lamp"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: wgpu::TextureFormat::R16Float,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            }),
            multiview_mask: None,
            cache: None,
        });
        Self {
            opaque: pipeline(Layer::Opaque),
            decal: pipeline(Layer::Decal),
            lamp,
            lighting,
            lighting_group,
            camera_layout,
            shadow_view,
            lamp_depth_view,
            shadow_group,
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
    /// The opaque solids again, from the lamp, into the shadow map.
    lamp: wgpu::RenderPipeline,
    /// The frame's lighting, written each frame.
    lighting: wgpu::Buffer,
    lighting_group: wgpu::BindGroup,
    /// The frame's camera, for the buffers' bind group: the vertex shader
    /// projects with it.
    camera_layout: wgpu::BindGroupLayout,
    /// What the lamp saw last frame, for the lit shaders to read.
    shadow_view: wgpu::TextureView,
    /// The depth test the lamp's own pass runs on, discarded after.
    lamp_depth_view: wgpu::TextureView,
    shadow_group: wgpu::BindGroup,
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
    let mut vertices = Vec::new();
    let mut indices = Vec::new();
    let mut draws = Vec::new();
    let (mut first, mut base) = (0, 0);
    for (layer, layer_solids) in solids.layers() {
        for solid in layer_solids {
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
        encoder: &mut wgpu::CommandEncoder,
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
        // What the lamp hides, before anything reads it: the opaque solids
        // again through `vs_lamp`, the survivor's depth written by `fs_lamp`
        // into the map. Decals lay flat and hide nothing; they cast
        // nothing either.
        if self.pass.lighting.shadow.is_on() {
            let mut lamp = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("depth_lamp"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &pipelines.shadow_view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::WHITE),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &pipelines.lamp_depth_view,
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
            lamp.set_pipeline(&pipelines.lamp);
            lamp.set_vertex_buffer(0, buffers.vertices.slice(..));
            lamp.set_index_buffer(buffers.indices.slice(..), wgpu::IndexFormat::Uint32);
            lamp.set_bind_group(1, &pipelines.lighting_group, &[]);
            for draw in &draws {
                if draw.layer == Layer::Opaque {
                    lamp.draw_indexed(draw.indices.clone(), draw.base, 0..1);
                }
            }
        }
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
        render_pass.set_bind_group(3, &pipelines.shadow_group, &[]);
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

    /// The sm9's frame at `zoom_percent` over `view`: `paint` draws it as the
    /// window's does, and the pass it ends with comes back with the font
    /// atlas's size, as the draw reads it.
    fn framed(
        view: Rect,
        zoom_percent: u16,
        paint: impl Fn(&eframe::egui::Painter, Rect, &typewriter_ui::draw::Metrics, f32, f32),
    ) -> (Pass, [usize; 2]) {
        let ctx = Context::default();
        let sm9 = include_str!("../../../../profiles/olympia-sm9.toml");
        let profile = typewriter_core::Profile::from_toml_str(sm9).unwrap();
        let metrics = typewriter_ui::draw::Metrics::new(
            &profile,
            typewriter_ui::draw::points_per_inch(zoom_percent),
        );
        let typing_y =
            view.height() * crate::machine::typing_line_height(view, &metrics, zoom_percent);
        let middle = view.center().x;
        let mut output = ctx.run_ui(
            eframe::egui::RawInput {
                screen_rect: Some(view),
                ..Default::default()
            },
            |ui| {
                let painter = ui.painter();
                paint(painter, view, &metrics, typing_y, middle);
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

    /// The whole machine painted as a frame, in the tall snapshot's view.
    fn machine_pass() -> (Pass, [usize; 2]) {
        framed(
            Rect::from_min_size(Pos2::ZERO, vec2(1400.0, 2400.0)),
            100,
            |painter, view, metrics, typing_y, middle| {
                crate::machine::paint_behind(painter, view, metrics, typing_y, middle, 0.0);
                crate::machine::paint_knobs(
                    painter,
                    view,
                    metrics,
                    typing_y,
                    (middle, 0.0),
                    |_| false,
                );
                crate::machine::paint_front(painter, view, metrics, typing_y);
            },
        )
    }

    /// The desk seen from the chair, the carriage `shift` points left of
    /// centre: the room's flat wood behind, the machine's depth — and the
    /// lamp's shadows on it — over that. `zoom_percent` sets the camera:
    /// the bench's scenes sit nearer and further than the snapshot's.
    fn desk_pass(size: Vec2, zoom_percent: u16, shift: f32) -> (Pass, [usize; 2]) {
        framed(
            Rect::from_min_size(Pos2::ZERO, size),
            zoom_percent,
            move |painter, view, metrics, typing_y, middle| {
                crate::room::paint(painter, view, metrics, zoom_percent);
                crate::machine::paint_behind(painter, view, metrics, typing_y, middle - shift, 0.0);
                crate::machine::paint_knobs(
                    painter,
                    view,
                    metrics,
                    typing_y,
                    (middle - shift, 0.0),
                    |_| false,
                );
                crate::machine::paint_front(painter, view, metrics, typing_y);
            },
        )
    }

    /// A headless device with the depth pass's pipelines, a one-texel white
    /// texture standing for the font atlas, and `pass`'s packed draws on
    /// their buffers, as `Gpu::prepare` leaves the window's: what the
    /// snapshot's shot and the bench's timing both draw. `extra` carries
    /// what a caller wants beyond the default device's; whether it landed
    /// reads off the `device`.
    struct Rig {
        device: wgpu::Device,
        queue: wgpu::Queue,
        backend: wgpu::Backend,
        pipelines: Pipelines,
        white_group: wgpu::BindGroup,
        vertices: Vec<DepthVertex>,
        indices: Vec<u32>,
        draws: Vec<Draw>,
        buffers: Buffers,
    }

    fn rig(
        backends: wgpu::Backends,
        pass: &Pass,
        fonts: [usize; 2],
        extra: wgpu::Features,
    ) -> Result<Rig, String> {
        let mut desc = wgpu::InstanceDescriptor::new_without_display_handle();
        desc.backends = backends;
        let instance = wgpu::Instance::new(desc);
        let adapter =
            wait(instance.request_adapter(&Default::default())).map_err(|why| why.to_string())?;
        let info = adapter.get_info();
        eprintln!("adapter: {:?} {}", info.backend, info.name);
        let clamp = adapter
            .get_downlevel_capabilities()
            .flags
            .contains(wgpu::DownlevelFlags::DEPTH_BIAS_CLAMP);
        let (device, queue) = wait(adapter.request_device(&wgpu::DeviceDescriptor {
            required_features: extra & adapter.features(),
            ..Default::default()
        }))
        .map_err(|why| why.to_string())?;
        let pipelines = Pipelines::build(&device, wgpu::TextureFormat::Rgba8Unorm, clamp);
        let textures = texture_layout(&device);
        // A white texel: colour reaches the shader through vertices, not
        // the texture — but the glyph regions, which the twin shares.
        let white = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("gpu_white"),
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
        let white_view = white.create_view(&Default::default());
        let white_sampler = device.create_sampler(&wgpu::SamplerDescriptor::default());
        // The group holds its texture, view and sampler, so these handles
        // need not outlive creation.
        let white_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("gpu_white"),
            layout: &textures,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&white_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&white_sampler),
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
        Ok(Rig {
            device,
            queue,
            backend: info.backend,
            pipelines,
            white_group,
            vertices,
            indices,
            draws,
            buffers,
        })
    }

    /// Draws the opaque draws through the lamp's pipeline into the shadow
    /// map on an already-begun pass, as `Gpu::prepare` does: the pass is
    /// the caller's, so a bench can time it.
    fn lamp_draws(rig: &Rig, lamp: &mut wgpu::RenderPass<'_>) {
        lamp.set_pipeline(&rig.pipelines.lamp);
        lamp.set_vertex_buffer(0, rig.buffers.vertices.slice(..));
        lamp.set_index_buffer(rig.buffers.indices.slice(..), wgpu::IndexFormat::Uint32);
        lamp.set_bind_group(1, &rig.pipelines.lighting_group, &[]);
        for draw in &rig.draws {
            if draw.layer == Layer::Opaque {
                lamp.draw_indexed(draw.indices.clone(), draw.base, 0..1);
            }
        }
    }

    /// Draws every draw on an already-begun pass, as `Gpu::paint` does, the
    /// white group standing for the font atlas.
    fn paint_draws(rig: &Rig, render: &mut wgpu::RenderPass<'_>) {
        render.set_vertex_buffer(0, rig.buffers.vertices.slice(..));
        render.set_index_buffer(rig.buffers.indices.slice(..), wgpu::IndexFormat::Uint32);
        render.set_bind_group(1, &rig.pipelines.lighting_group, &[]);
        render.set_bind_group(2, &rig.buffers.camera_group, &[]);
        render.set_bind_group(3, &rig.pipelines.shadow_group, &[]);
        for draw in &rig.draws {
            render.set_pipeline(match draw.layer {
                Layer::Opaque => &rig.pipelines.opaque,
                Layer::Decal => &rig.pipelines.decal,
            });
            render.set_bind_group(0, &rig.white_group, &[]);
            render.draw_indexed(draw.indices.clone(), draw.base, 0..1);
        }
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
        clear: wgpu::Color,
    ) -> Result<(wgpu::Backend, Vec<u8>), String> {
        let (width, height) = (pass.camera.points.x as u32, pass.camera.points.y as u32);
        let rig = rig(backends, pass, fonts, wgpu::Features::empty())?;
        let backend = rig.backend;
        // The lamp's own pass, first: what it hides, into the shadow map.
        {
            let mut encoder = rig.device.create_command_encoder(&Default::default());
            {
                let mut lamp = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("gpu_snapshot_lamp"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: &rig.pipelines.shadow_view,
                        depth_slice: None,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(wgpu::Color::WHITE),
                            store: wgpu::StoreOp::Store,
                        },
                    })],
                    depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                        view: &rig.pipelines.lamp_depth_view,
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
                lamp_draws(&rig, &mut lamp);
            }
            rig.queue.submit([encoder.finish()]);
        }
        let target = rig.device.create_texture(&wgpu::TextureDescriptor {
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
        let depth = rig.device.create_texture(&wgpu::TextureDescriptor {
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
        let mut encoder = rig.device.create_command_encoder(&Default::default());
        {
            let mut render = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("gpu_snapshot"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &target_view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(clear),
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
            paint_draws(&rig, &mut render);
        }
        rig.queue.submit([encoder.finish()]);
        // The copy wants rows a multiple of 256 bytes.
        let row_bytes = (width * 4).next_multiple_of(256);
        let readback = rig.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("gpu_snapshot_readback"),
            size: u64::from(row_bytes) * u64::from(height),
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let mut encoder = rig.device.create_command_encoder(&Default::default());
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
        rig.queue.submit([encoder.finish()]);
        let (sender, receiver) = std::sync::mpsc::sync_channel(1);
        readback
            .slice(..)
            .map_async(wgpu::MapMode::Read, move |mapped| {
                let _ = sender.send(mapped);
            });
        let _ = rig.device.poll(wgpu::PollType::wait_indefinitely());
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
            match gpu_capture(backends, &pass, fonts, wgpu::Color::BLACK) {
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

    /// The lamp's shadows where they fall on the desk, headless:
    /// `desk-<backend>.ppm` with the carriage at rest and
    /// `desk-left-<backend>.ppm` thrown left — the views its moving
    /// shadow crosses the map's edges in. The flat room's mesh is not
    /// drawn here, so the target clears to the wood's own near colour:
    /// the desk's catcher decal reads over it as it does in the window.
    /// As [`gpu_snapshot`]: wants a device, not for CI.
    #[test]
    #[ignore]
    fn gpu_desk_snapshot() {
        let Some(folder) = std::env::var_os("TYPEWRITER_GPU_SNAPSHOT") else {
            return;
        };
        let folder = std::path::PathBuf::from(folder);
        // The wood's near colour, gamma as the target stores it.
        let wood = wgpu::Color {
            r: 0x9C as f64 / 255.0,
            g: 0x6C as f64 / 255.0,
            b: 0x43 as f64 / 255.0,
            a: 1.0,
        };
        let mut captured = 0;
        for (name, shift) in [("desk", 0.0), ("desk-left", 400.0)] {
            let (pass, fonts) = desk_pass(vec2(1600.0, 1000.0), 100, shift);
            if shift == 0.0 {
                // The CPU twin's map beside the GPU's reads: the lamp's
                // cast with cleared texels white, so a bright hole in the
                // machine's shadow can be told a seam in the cast from a
                // GPU quirk.
                let map = crate::depth::raster::cast(&pass.solids, &pass.lighting);
                let side = pass.lighting.shadow.side as usize;
                let mut ppm = format!("P6\n{side} {side}\n255\n").into_bytes();
                for depth in map {
                    let v = (depth.clamp(0.0, 1.0) * 255.0) as u8;
                    ppm.extend_from_slice(&[v, v, v]);
                }
                std::fs::write(folder.join("map.ppm"), ppm).unwrap();
            }
            for backends in [wgpu::Backends::VULKAN, wgpu::Backends::GL] {
                match gpu_capture(backends, &pass, fonts, wood) {
                    Ok((backend, ppm)) => {
                        let file = format!("{name}-{:?}.ppm", backend).to_lowercase();
                        std::fs::write(folder.join(file), ppm).unwrap();
                        eprintln!("captured {name} {backend:?}");
                        captured += 1;
                    }
                    Err(why) => eprintln!("{backends:?}: no device: {why}"),
                }
            }
        }
        assert!(captured > 0, "no headless device answered");
    }

    /// What one frame of the depth pass cost, from [`bench_frames`].
    struct GpuFrameTimes {
        backend: wgpu::Backend,
        /// Whether the device read the pass timings below: a backend whose
        /// adapter lacks `Features::TIMESTAMP_QUERY` leaves them empty and
        /// only the wall-side samples answer. This machine answers on both
        /// Vulkan and GL.
        timestamps: bool,
        /// The lamp's shadow pass, GPU milliseconds a frame.
        lamp_ms: Vec<f64>,
        /// The main full-window pass, GPU milliseconds a frame.
        main_ms: Vec<f64>,
        /// Lamp's begin to main's end: the depth pass's GPU span, the gap
        /// between the two included.
        span_ms: Vec<f64>,
        /// Writing the frame's buffers, encoding, and submitting: the
        /// CPU's own share of a frame, egui's pass aside (that is
        /// `stage::tests::bench`'s).
        cpu_ms: Vec<f64>,
        /// The whole batch's wall over the frames: throughput once pipelined,
        /// without the window system's pacing.
        wall_ms: f64,
    }

    /// Draws `pass` `frames` times on a headless device of `backends`, each
    /// frame as `Gpu` does — the lamp's pass into the shadow map, then the
    /// lit full-window pass, one pair of command buffers a submit — and times
    /// every pass with real GPU timestamp queries when the adapter's
    /// `Features::TIMESTAMP_QUERY` answers (requested on the device; wgpu
    /// only lets pass begin/end writes, none inside a pass). Buffers are
    /// written afresh each frame, as the window's frames are.
    fn bench_frames(
        backends: wgpu::Backends,
        pass: &Pass,
        fonts: [usize; 2],
        clear: wgpu::Color,
        frames: usize,
    ) -> Result<GpuFrameTimes, String> {
        let (width, height) = (pass.camera.points.x as u32, pass.camera.points.y as u32);
        let wanted = wgpu::Features::TIMESTAMP_QUERY;
        let rig = rig(backends, pass, fonts, wanted)?;
        let timestamps = rig.device.features().contains(wanted);
        let target = rig.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("bench_target"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });
        let depth = rig.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("bench_depth"),
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
        // Four slots a frame, into one set of wgpu's 4096; the bench keeps
        // `frames` well under. One frame more than timed: the first draws.
        let timed = frames + 1;
        let queries = timestamps.then(|| {
            rig.device.create_query_set(&wgpu::QuerySetDescriptor {
                label: Some("bench_timestamps"),
                ty: wgpu::QueryType::Timestamp,
                count: 4 * timed as u32,
            })
        });
        let writes = |first: u32| {
            queries.as_ref().map(|set| wgpu::RenderPassTimestampWrites {
                query_set: set,
                beginning_of_pass_write_index: Some(first),
                end_of_pass_write_index: Some(first + 1),
            })
        };
        let mut times = GpuFrameTimes {
            backend: rig.backend,
            timestamps,
            lamp_ms: Vec::new(),
            main_ms: Vec::new(),
            span_ms: Vec::new(),
            cpu_ms: Vec::with_capacity(frames),
            wall_ms: 0.0,
        };
        let mut batch = std::time::Instant::now();
        for frame in 0..=frames as u32 {
            let started = std::time::Instant::now();
            rig.queue.write_buffer(
                &rig.pipelines.lighting,
                0,
                bytemuck::bytes_of(&pass.lighting.uniform()),
            );
            rig.queue.write_buffer(
                &rig.buffers.camera,
                0,
                bytemuck::bytes_of(&pass.camera.uniform()),
            );
            rig.queue.write_buffer(
                &rig.buffers.vertices,
                0,
                bytemuck::cast_slice(&rig.vertices),
            );
            rig.queue
                .write_buffer(&rig.buffers.indices, 0, bytemuck::cast_slice(&rig.indices));
            let mut lamp_encoder = rig.device.create_command_encoder(&Default::default());
            {
                let mut lamp = lamp_encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("bench_lamp"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: &rig.pipelines.shadow_view,
                        depth_slice: None,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(wgpu::Color::WHITE),
                            store: wgpu::StoreOp::Store,
                        },
                    })],
                    depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                        view: &rig.pipelines.lamp_depth_view,
                        depth_ops: Some(wgpu::Operations {
                            load: wgpu::LoadOp::Clear(1.0),
                            store: wgpu::StoreOp::Discard,
                        }),
                        stencil_ops: None,
                    }),
                    timestamp_writes: writes(4 * frame),
                    occlusion_query_set: None,
                    multiview_mask: None,
                });
                lamp_draws(&rig, &mut lamp);
            }
            let mut main_encoder = rig.device.create_command_encoder(&Default::default());
            {
                let mut render = main_encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("bench_main"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: &target_view,
                        depth_slice: None,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(clear),
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
                    timestamp_writes: writes(4 * frame + 2),
                    occlusion_query_set: None,
                    multiview_mask: None,
                });
                paint_draws(&rig, &mut render);
            }
            rig.queue
                .submit([lamp_encoder.finish(), main_encoder.finish()]);
            if frame == 0 {
                // The first frame is a warm-up: it builds what the rest
                // draw. Drain it, and the wall covers only the timed ones.
                let _ = rig.device.poll(wgpu::PollType::wait_indefinitely());
                batch = std::time::Instant::now();
            } else {
                times.cpu_ms.push(started.elapsed().as_secs_f64() * 1e3);
            }
        }
        // The submits never block: drain the queue, so the wall is the
        // device's pace through the timed frames, not the CPU's enqueue.
        let _ = rig.device.poll(wgpu::PollType::wait_indefinitely());
        times.wall_ms = batch.elapsed().as_secs_f64() * 1e3 / (frames.max(1) as f64);
        if let Some(queries) = queries {
            let count = 4 * timed as u32;
            // A resolve destination cannot also be map-read: the ticks
            // resolve into one buffer, copied out to a mapped one.
            let resolved = rig.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("bench_resolved"),
                size: u64::from(count) * 8,
                usage: wgpu::BufferUsages::QUERY_RESOLVE
                    | wgpu::BufferUsages::COPY_DST
                    | wgpu::BufferUsages::COPY_SRC,
                mapped_at_creation: false,
            });
            let readback = rig.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("bench_readback"),
                size: u64::from(count) * 8,
                usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            let mut encoder = rig.device.create_command_encoder(&Default::default());
            encoder.resolve_query_set(&queries, 0..count, &resolved, 0);
            encoder.copy_buffer_to_buffer(&resolved, 0, &readback, 0, u64::from(count) * 8);
            rig.queue.submit([encoder.finish()]);
            let (sender, receiver) = std::sync::mpsc::sync_channel(1);
            readback
                .slice(..)
                .map_async(wgpu::MapMode::Read, move |mapped| {
                    let _ = sender.send(mapped);
                });
            let _ = rig.device.poll(wgpu::PollType::wait_indefinitely());
            receiver
                .recv()
                .map_err(|_| "no mapping")?
                .map_err(|why| why.to_string())?;
            let mapped = readback
                .get_mapped_range(..)
                .map_err(|why| why.to_string())?;
            let stamp = |i: usize| u64::from_le_bytes(mapped[i * 8..i * 8 + 8].try_into().unwrap());
            // Ticks to milliseconds, and the queue's clock to the unit.
            let period = f64::from(rig.queue.get_timestamp_period()) * 1e-6;
            for frame in 0..timed {
                let first = 4 * frame;
                let diff = |a: usize, b: usize| stamp(b).wrapping_sub(stamp(a)) as f64 * period;
                // The warm-up frame's slot is read too, and dropped: every
                // list has one sample more than the bench reports.
                times.lamp_ms.push(diff(first, first + 1));
                times.main_ms.push(diff(first + 2, first + 3));
                times.span_ms.push(diff(first, first + 3));
            }
            for list in [&mut times.lamp_ms, &mut times.main_ms, &mut times.span_ms] {
                list.remove(0);
            }
            // Uncomment for a raw look at the clock: the ticks of the first
            // three frames, and what one tick is worth.
            // eprintln!(
            //     "period {period:?} ns/texel-ms ticks {:?}",
            //     (0..12).map(stamp).collect::<Vec<_>>()
            // );
            drop(mapped);
            readback.unmap();
        }
        Ok(times)
    }

    /// The shadow bench, on the same `TYPEWRITER_BENCH` frames gate as
    /// `stage::tests::bench` — the `--ignored bench` filter
    /// runs both: draws the desk scene `frames` times on every headless
    /// device that answers and prints, per frame, the lamp's shadow pass and
    /// the lit full-window pass in real GPU milliseconds from timestamp
    /// queries, the pass's GPU span, and the CPU's buffer-and-submit wall.
    /// The scenes are the snapshot shots' window sizes: the desk at 100 %, a
    /// full-HD window, the wide view at 200 %, and the far-back one at 25 %.
    /// Not for CI: it wants a device; without one it only reports it.
    /// One warm-up frame a scene is drained before timing, but an
    /// integrated GPU still ramps its clocks: small `frames` reads slow —
    /// 200 or more shows the steady state.
    ///
    /// ```sh
    /// TYPEWRITER_BENCH=50 xvfb-run -a cargo test -p typewriter --release -- \
    ///     --ignored bench --nocapture
    /// ```
    #[test]
    #[ignore]
    fn gpu_bench() {
        let Some(frames) = std::env::var("TYPEWRITER_BENCH")
            .ok()
            .and_then(|said| said.parse::<usize>().ok())
        else {
            return;
        };
        // Four queries a frame, into one set of wgpu's 4096.
        let frames = frames.clamp(1, 1000);
        let min = |v: &[f64]| v.iter().copied().fold(f64::MAX, f64::min);
        let median = |v: &[f64]| {
            let mut sorted = v.to_vec();
            sorted.sort_unstable_by(|a, b| a.total_cmp(b));
            sorted[sorted.len() / 2]
        };
        let pair = |v: &[f64]| {
            if v.is_empty() {
                format!("{:^15}", "-")
            } else {
                format!("{:>7.3}/{:<7.3}", min(v), median(v))
            }
        };
        // The wood's near colour, as the desk snapshot clears to.
        let wood = wgpu::Color {
            r: 0x9C as f64 / 255.0,
            g: 0x6C as f64 / 255.0,
            b: 0x43 as f64 / 255.0,
            a: 1.0,
        };
        println!(
            "{frames} frames of the depth pass per scene, per-frame GPU milliseconds min/median from timestamp queries."
        );
        println!(
            "{:<22}{:<8}{:>15}{:>15}{:>15}{:>10}{:>9}",
            "scene", "backend", "lamp", "main", "span", "cpu ms", "wall ms"
        );
        let scenes = [
            ("desk 1600×1000", vec2(1600.0, 1000.0), 100),
            ("full 1920×1080", vec2(1920.0, 1080.0), 100),
            ("wide 3000×1400", vec2(3000.0, 1400.0), 200),
            ("far-back 1536×960@25", vec2(1536.0, 960.0), 25),
        ];
        for (name, size, zoom_percent) in scenes {
            let (pass, fonts) = desk_pass(size, zoom_percent, 0.0);
            for backends in [wgpu::Backends::VULKAN, wgpu::Backends::GL] {
                match bench_frames(backends, &pass, fonts, wood, frames) {
                    Ok(t) => println!(
                        "{:<22}{:<8}{}{}{}{:>10.3}{:>9.3}{}",
                        name,
                        format!("{:?}", t.backend).to_lowercase(),
                        pair(&t.lamp_ms),
                        pair(&t.main_ms),
                        pair(&t.span_ms),
                        median(&t.cpu_ms),
                        t.wall_ms,
                        if t.timestamps {
                            ""
                        } else {
                            " (no timestamp queries)"
                        },
                    ),
                    Err(why) => eprintln!("{name}: {backends:?}: no device: {why}"),
                }
            }
        }
    }
}
