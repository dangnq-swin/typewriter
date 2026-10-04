//! The renderer core's pipelines: the passes the frame draws with, and
//! everything they bind once — the lamp's 2048² shadow map among them.
//! Decals get their lift from the pipeline's
//! [`DepthBiasState`](wgpu::DepthBiasState): the `decal_bias` twin is what
//! the CPU rasterizer adds instead.

use bytemuck::{Pod, Zeroable};
use glam::Vec4;

use super::camera::CAMERA_BYTES;
use super::vertex::VERTEX_BYTES;

/// The scene shader, beside this: the camera and the placements project,
/// the lamp's pass casts, and the lit fragments are T3's to land on the
/// layout declared here.
pub(super) const SHADER: &str = include_str!("scene.wgsl");

/// The shadow map's side in texels, as the lamp's eye and the map's own
/// agree it to be: the single source for the renderer core.
/// `typewriter`'s `depth` keeps its own copy until T8 retires it.
pub const SHADOW_MAP: u32 = 2048;

/// How many bands chrome's room has.
pub const CHROME_BANDS: usize = 5;

/// A placement's transform, as the shader's `r_model` reads it.
pub const MODEL_BYTES: usize = std::mem::size_of::<glam::Mat4>();

/// The uniform the shader reads: `r_lighting` in `scene.wgsl`.
pub const LIGHTING_BYTES: u64 = std::mem::size_of::<LightingUniform>() as u64;

/// The frame's light as the shader reads it, laid out as `r_lighting`:
/// where the lamp stands and how wide its head burns, the way to the eye,
/// the room chrome mirrors, and the lamp's own eye — the shadow map's
/// three rows and their parameters. T3's lighting builds it; until then
/// the shadow rows and the side that switches the pass on are what the
/// frame fills.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Pod, Zeroable)]
pub struct LightingUniform {
    /// The lamp's head, `w` its radius in millimetres.
    pub lamp: Vec4,
    /// Toward the eye, unit length, `w` the falloff's reference distance.
    pub eye: Vec4,
    /// Chrome's bands, `x` how far down, `yzw` its colour in gamma.
    pub bands: [Vec4; CHROME_BANDS],
    /// The shadow map's rows: across and up its texels in −1..=1, and its
    /// depth 0..=1, each row's `w` its constant.
    pub shadow: [Vec4; 3],
    /// `x` the map's depth bias, `y` the side in texels — 0 for no
    /// shadows — `z` the penumbra's scale, `w` the map's depth run in
    /// millimetres.
    pub params: Vec4,
}

/// The depth buffer's basic unit, which is what a `wgpu::DepthBiasState`
/// constant is counted in: what Vulkan and D3D take as the minimum
/// representable interval of a 32-bit float depth, as Mesa does for GL's
/// `glPolygonOffset` units.
#[cfg(test)]
const DEPTH_UNIT: f32 = 1.0 / (1u32 << 24) as f32;

/// The decal pass's constant bias, negative to lift: a fragment's depth is
/// biased by the sum *added* to it, so a negative one moves a decal toward
/// the eye. Worth the old 0.25 mm at the paper's distance: enough for a
/// decal's own mesh to show over the coarser mesh it lies on, where a
/// face turns to the eye.
const DECAL_BIAS_UNITS: i32 = -285;

/// The decal pass's slope-scaled bias: times the steepest depth a decal
/// triangle's runs per screen pixel. Where a face turns away, that is
/// where lying on it needs the most lift.
const DECAL_BIAS_SLOPE: f32 = -2.0;

/// The cap on their sum's magnitude, in absolute depth: a little over the
/// constant, so a steep face gets more lift than the paper does but not
/// near again what it would in millimetres. Short of the window the
/// guide's glass keeps behind the ribbon (`printing_point`'s test); it
/// also bounds the bias on any backend that takes the constant in absolute
/// depth too (Metal).
const DECAL_BIAS_CLAMP: f32 = 2.0e-5;

/// How the decal pipeline lifts a triangle whose depth runs `slope` per
/// screen pixel: the magnitude of the negative bias the GPU adds, toward
/// the eye, so what lies on a face shows over it. The GPU applies this
/// through the pipeline's `DepthBiasState`; `rasterize` applies the same
/// through its `decal_slope`.
#[cfg(test)]
pub(crate) fn decal_bias(slope: f32) -> f32 {
    (DECAL_BIAS_UNITS as f32 * DEPTH_UNIT + DECAL_BIAS_SLOPE * slope)
        .clamp(-DECAL_BIAS_CLAMP, 0.0)
        .abs()
}

/// The decal pipeline's bias, `clamp`ed where the backend caps the slope
/// term at all.
fn decal_bias_state(clamp: bool) -> wgpu::DepthBiasState {
    if clamp {
        wgpu::DepthBiasState {
            constant: DECAL_BIAS_UNITS,
            slope_scale: DECAL_BIAS_SLOPE,
            clamp: DECAL_BIAS_CLAMP,
        }
    } else {
        // Devices without depth-bias clamping (some software renderers)
        // cannot cap the slope term, and an uncapped one would float a
        // decal far off its face: there the pass lifts by the constant
        // alone.
        wgpu::DepthBiasState {
            constant: DECAL_BIAS_UNITS,
            slope_scale: 0.0,
            clamp: 0.0,
        }
    }
}

/// Group 0's layout: a texture and a sampler, as egui's are, so what the
/// app uploads reads as its own does today.
pub(super) fn texture_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("scene_textures"),
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
/// float map, its texels blended by the sampler — which the lit shaders
/// read when they land.
fn shadow_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("scene_shadow"),
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

/// The renderer core's pipelines and everything they bind once: the lamp's
/// shadow map among them, built for a target's format and kept.
pub(super) struct Pipelines {
    pub(super) opaque: wgpu::RenderPipeline,
    pub(super) decal: wgpu::RenderPipeline,
    /// Text on a surface: the placeholder T4's glyphs grow into.
    pub(super) text: wgpu::RenderPipeline,
    /// The opaque solids again, from the lamp, into the shadow map.
    pub(super) lamp: wgpu::RenderPipeline,
    /// The frame's lighting, written each frame.
    pub(super) lighting: wgpu::Buffer,
    pub(super) lighting_group: wgpu::BindGroup,
    /// The frame's camera and placements' transforms: one layout, the
    /// transforms by dynamic offset, a slot a draw.
    pub(super) frame_layout: wgpu::BindGroupLayout,
    /// The lamp's map, kept whole: what the lamp saw last, for the lit
    /// shaders to read, and what the bare-device tests read back.
    #[cfg(test)]
    pub(super) shadow: wgpu::Texture,
    /// What the lamp saw last, for the lit shaders to read.
    pub(super) shadow_view: wgpu::TextureView,
    /// The depth test the lamp's own pass runs on, discarded after.
    pub(super) lamp_depth_view: wgpu::TextureView,
    pub(super) shadow_group: wgpu::BindGroup,
}

impl Pipelines {
    /// The frame's passes' pipelines, for a target in `format`, needing
    /// only a device: [`super::Device::new`] builds them on the window's,
    /// and the bare-device tests on their own. `clamp` is whether the
    /// backend caps the decal bias.
    pub(super) fn build(device: &wgpu::Device, format: wgpu::TextureFormat, clamp: bool) -> Self {
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("scene"),
            source: wgpu::ShaderSource::Wgsl(SHADER.into()),
        });
        let textures = texture_layout(device);
        let lighting_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("scene_lighting"),
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
            label: Some("scene_lighting"),
            size: LIGHTING_BYTES,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let lighting_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("scene_lighting"),
            layout: &lighting_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: lighting.as_entire_binding(),
            }],
        });
        // The frame's camera and its placements: one group, the camera a
        // fixed uniform, the placements a slot a draw by dynamic offset.
        let frame_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("scene_frame"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: wgpu::BufferSize::new(CAMERA_BYTES),
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: true,
                        min_binding_size: wgpu::BufferSize::new(MODEL_BYTES as u64),
                    },
                    count: None,
                },
            ],
        });
        // The lamp's map, once: the same side every frame's lighting reads.
        // A 16-bit float map, since GL cannot load a depth texture's
        // texels: the lamp's fragment writes the depth into its single
        // channel, and the sampler blends the texels. `R16Float` is core
        // WebGPU — filterable and renderable wherever wgpu runs, no
        // feature asked — while its fixed-point cousins need
        // `TEXTURE_FORMAT_16BIT_NORM` and are not renderable at all in
        // wgpu 30. A float's step is at worst a texel's pitch over the
        // map's depth run: the map's own resolution. `COPY_SRC` for the
        // bare-device tests' readback, nothing the frame pays for. Kept
        // whole, for the tests read it back.
        let shadow = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("scene_shadow"),
            size: wgpu::Extent3d {
                width: SHADOW_MAP,
                height: SHADOW_MAP,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::R16Float,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let shadow_view = shadow.create_view(&Default::default());
        let lamp_depth = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("scene_shadow_depth"),
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
            label: Some("scene_shadow"),
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
            label: Some("scene_shadow"),
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
            label: Some("scene"),
            bind_group_layouts: &[
                Some(&textures),
                Some(&lighting_layout),
                Some(&frame_layout),
                Some(&shadows),
            ],
            immediate_size: 0,
        });
        // The lamp's own pass: the casters again, projected by the shadow
        // rows the frame's lighting carries, into the map. No faces are
        // culled: a plate casts by whatever side the light meets. The
        // depth test runs on a throwaway depth texture; what survives it
        // the fragment writes into the map.
        let lamp_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("scene_lamp"),
            bind_group_layouts: &[None, Some(&lighting_layout), Some(&frame_layout), None],
            immediate_size: 0,
        });
        let scene_pipeline =
            |label: &'static str, fragment: &'static str, depth: wgpu::DepthStencilState| {
                device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                    label: Some(label),
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
                    depth_stencil: Some(depth),
                    multisample: wgpu::MultisampleState::default(),
                    fragment: Some(wgpu::FragmentState {
                        module: &module,
                        entry_point: Some(fragment),
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
        let opaque_depth = wgpu::DepthStencilState {
            format: wgpu::TextureFormat::Depth32Float,
            depth_write_enabled: Some(true),
            depth_compare: Some(wgpu::CompareFunction::Less),
            stencil: wgpu::StencilState::default(),
            bias: wgpu::DepthBiasState::default(),
        };
        // Decals lie on faces they match exactly: the pipeline lifts them,
        // so they show without standing nearer in millimetres.
        let lying_depth = wgpu::DepthStencilState {
            format: wgpu::TextureFormat::Depth32Float,
            depth_write_enabled: Some(false),
            depth_compare: Some(wgpu::CompareFunction::LessEqual),
            stencil: wgpu::StencilState::default(),
            bias: decal_bias_state(clamp),
        };
        let main_fragment = if format.is_srgb() {
            "fs_main_linear_framebuffer"
        } else {
            "fs_main_gamma_framebuffer"
        };
        let text_fragment = if format.is_srgb() {
            "fs_text_linear_framebuffer"
        } else {
            "fs_text_gamma_framebuffer"
        };
        let lamp = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("scene_lamp"),
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
            opaque: scene_pipeline("scene_opaque", main_fragment, opaque_depth),
            decal: scene_pipeline("scene_decal", main_fragment, lying_depth.clone()),
            text: scene_pipeline("scene_text", text_fragment, lying_depth),
            lamp,
            lighting,
            lighting_group,
            frame_layout,
            #[cfg(test)]
            shadow,
            shadow_view,
            lamp_depth_view,
            shadow_group,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The scene shader, parsed and validated: the camera and the lamp
    /// project through it, so the frame means nothing while it does not
    /// compile.
    #[test]
    fn the_shader_is_valid() {
        use wgpu::naga;
        let module = naga::front::wgsl::parse_str(SHADER).unwrap();
        let mut validator = naga::valid::Validator::new(
            naga::valid::ValidationFlags::all(),
            naga::valid::Capabilities::empty(),
        );
        validator.validate(&module).unwrap();
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
}
