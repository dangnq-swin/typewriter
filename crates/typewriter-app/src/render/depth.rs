//! What stands in front hides what is behind: triangles drawn with a depth
//! buffer, in any order, through a paint callback on eframe's wgpu renderer.
//! The desk edition draws its machine and sheets so.
//!
//! Each vertex is egui's, on screen in points, with a depth added: 0 at the
//! eye to 1 far off. Opaque triangles hide what is behind them; decals lie
//! on them (print, edges, glass), hide nothing and are drawn after. Where no
//! renderer is installed, as in the snapshot tool, the triangles go out as
//! data for it to draw.

use std::collections::HashMap;
use std::sync::{Arc, OnceLock, Weak};

use eframe::egui::epaint::{PaintCallback, TessellationOptions, Tessellator, Vertex};
use eframe::egui::mutex::RwLock;
use eframe::egui::{Color32, Context, Id, Mesh, Painter, Shape, TextureId};
use eframe::egui_wgpu::{self, CallbackResources, CallbackTrait, Renderer, ScreenDescriptor};
use eframe::wgpu::{self, util::DeviceExt};

/// The window's depth buffer, in bits: eframe's `depth_buffer`. One sample
/// a pixel, as egui's: wgpu's OpenGL backend shows nothing with more.
pub const DEPTH_BITS: u8 = 32;
const SHADER: &str = include_str!("depth.wgsl");
/// Position (3 floats), uv (2), colour (4 bytes).
const VERTEX_BYTES: u64 = 24;

/// Whether triangles hide what is behind them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Layer {
    Opaque,
    /// Lying on an opaque surface, drawn after: hides nothing.
    Decal,
}

/// Triangles in depth, in one texture: egui's mesh, and each vertex's depth.
#[derive(Debug, Clone, Default)]
pub struct Solid {
    pub mesh: Mesh,
    /// 0 at the eye to 1 far off, one for each of `mesh`'s vertices.
    pub depths: Vec<f32>,
}

/// What a frame draws in depth.
#[derive(Debug, Default)]
pub struct Solids {
    opaque: Vec<Solid>,
    decals: Vec<Solid>,
}

impl Solids {
    pub fn is_empty(&self) -> bool {
        self.opaque.is_empty() && self.decals.is_empty()
    }

    /// Opaque, then decals: the order they are drawn in.
    pub fn layers(&self) -> [(Layer, &[Solid]); 2] {
        [(Layer::Opaque, &self.opaque), (Layer::Decal, &self.decals)]
    }

    /// Adds `mesh`, `place` moving each vertex where it shows, maybe tinting
    /// it, and giving its depth. `None` hides a vertex, and the triangles
    /// touching it.
    pub fn add(
        &mut self,
        layer: Layer,
        mut mesh: Mesh,
        mut place: impl FnMut(&mut Vertex) -> Option<f32>,
    ) {
        let placed: Vec<_> = mesh.vertices.iter_mut().map(&mut place).collect();
        let mut solid = Solid {
            mesh: Mesh::with_texture(mesh.texture_id),
            depths: Vec::new(),
        };
        // Old index to new, once used.
        let mut kept = vec![None; placed.len()];
        for triangle in mesh.indices.as_chunks::<3>().0 {
            if triangle.iter().any(|&i| placed[i as usize].is_none()) {
                continue;
            }
            for &i in triangle {
                let i = i as usize;
                let new = *kept[i].get_or_insert_with(|| {
                    solid.mesh.vertices.push(mesh.vertices[i]);
                    solid.depths.push(placed[i].unwrap_or_default());
                    // Safe cast: a frame's vertices fit egui's own u32 indices.
                    solid.mesh.vertices.len() as u32 - 1
                });
                solid.mesh.indices.push(new);
            }
        }
        if solid.mesh.indices.is_empty() {
            return;
        }
        match layer {
            Layer::Opaque => self.opaque.push(solid),
            Layer::Decal => self.decals.push(solid),
        }
    }

    /// `shapes` drawn flat, their triangles cut to no side `longest` long
    /// to bend with what they are placed on, then each vertex placed as by
    /// [`Solids::add`]. An opaque shape's edges aren't feathered: a
    /// feather's clear fringe would hide what is behind it.
    pub fn add_shapes(
        &mut self,
        painter: &Painter,
        layer: Layer,
        shapes: Vec<Shape>,
        longest: f32,
        mut place: impl FnMut(&mut Vertex) -> Option<f32>,
    ) {
        let options = TessellationOptions {
            feathering: layer == Layer::Decal,
            ..TessellationOptions::default()
        };
        let font_image = painter.fonts(|fonts| fonts.font_image_size());
        let mut tessellator =
            Tessellator::new(painter.pixels_per_point(), options, font_image, Vec::new());
        let mut mesh = Mesh::default();
        for shape in shapes {
            let mut one = Mesh::default();
            tessellator.tessellate_shape(shape, &mut one);
            if one.texture_id != mesh.texture_id && !mesh.is_empty() {
                let flat = subdivided(std::mem::take(&mut mesh), longest);
                self.add(layer, flat, &mut place);
            }
            mesh.texture_id = one.texture_id;
            mesh.append(one);
        }
        self.add(layer, subdivided(mesh, longest), place);
    }

    /// Everything in `other`, drawn with these.
    pub fn extend(&mut self, other: Self) {
        self.opaque.extend(other.opaque);
        self.decals.extend(other.decals);
    }
}

/// `mesh` with no triangle's side longer than `longest`: each long side cut
/// in half, its neighbour sharing the cut, until none is.
pub fn subdivided(mesh: Mesh, longest: f32) -> Mesh {
    let Mesh {
        indices,
        mut vertices,
        texture_id,
    } = mesh;
    let mut halves = HashMap::new();
    let mut out = Vec::with_capacity(indices.len());
    let mut todo: Vec<[u32; 3]> = indices.as_chunks::<3>().0.to_vec();
    let length = |vertices: &[Vertex], a: u32, b: u32| {
        vertices[a as usize].pos.distance(vertices[b as usize].pos)
    };
    while let Some([a, b, c]) = todo.pop() {
        let sides = [(a, b, c), (b, c, a), (c, a, b)];
        let (a, b, c) = sides
            .into_iter()
            .max_by(|x, y| length(&vertices, x.0, x.1).total_cmp(&length(&vertices, y.0, y.1)))
            .unwrap_or((a, b, c));
        if length(&vertices, a, b) <= longest {
            out.extend([a, b, c]);
            continue;
        }
        let half = *halves.entry((a.min(b), a.max(b))).or_insert_with(|| {
            let (p, q) = (vertices[a as usize], vertices[b as usize]);
            let colour = |i: usize| p.color[i].midpoint(q.color[i]);
            vertices.push(Vertex {
                pos: p.pos.lerp(q.pos, 0.5),
                uv: p.uv.lerp(q.uv, 0.5),
                color: Color32::from_rgba_premultiplied(colour(0), colour(1), colour(2), colour(3)),
            });
            // Safe cast: a frame's vertices fit egui's own u32 indices.
            vertices.len() as u32 - 1
        });
        todo.push([a, half, c]);
        todo.push([half, b, c]);
    }
    Mesh {
        indices: out,
        vertices,
        texture_id,
    }
}

/// Draws `solids` over what `painter` has drawn so far, under what it draws
/// after: their depth decides only among what is drawn in depth.
pub fn paint(painter: &Painter, solids: Solids) {
    if solids.is_empty() {
        return;
    }
    // The whole window: positions stay the window's.
    let rect = painter.ctx().viewport_rect();
    let solids = Arc::new(solids);
    let callback = if is_installed(painter.ctx()) {
        egui_wgpu::Callback::new_paint_callback(
            rect,
            Gpu {
                solids,
                buffers: OnceLock::new(),
            },
        )
    } else {
        PaintCallback {
            rect,
            callback: solids,
        }
    };
    painter.add(Shape::Callback(callback));
}

fn installed_id() -> Id {
    Id::new("depth-pass-installed")
}

fn is_installed(ctx: &Context) -> bool {
    ctx.data(|data| data.get_temp::<bool>(installed_id()))
        .unwrap_or(false)
}

/// Readies `render_state`'s window, with a [`DEPTH_BITS`] depth buffer, to
/// draw solids.
pub fn install(ctx: &Context, render_state: &egui_wgpu::RenderState) {
    let device = &render_state.device;
    let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("depth"),
        source: wgpu::ShaderSource::Wgsl(SHADER.into()),
    });
    // As egui's: its textures' bind groups serve.
    let textures = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
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
    });
    let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("depth"),
        bind_group_layouts: &[Some(&textures)],
        immediate_size: 0,
    });
    let format = render_state.target_format;
    let pipeline = |layer: Layer| {
        let opaque = layer == Layer::Opaque;
        device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some(if opaque { "depth_opaque" } else { "depth_decal" }),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module: &module,
                entry_point: Some("vs_main"),
                buffers: &[Some(wgpu::VertexBufferLayout {
                    array_stride: VERTEX_BYTES,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x2, 2 => Uint32],
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
                bias: wgpu::DepthBiasState::default(),
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
    let pipelines = Pipelines {
        opaque: pipeline(Layer::Opaque),
        decal: pipeline(Layer::Decal),
        renderer: Arc::downgrade(&render_state.renderer),
    };
    render_state
        .renderer
        .write()
        .callback_resources
        .insert(pipelines);
    ctx.data_mut(|data| data.insert_temp(installed_id(), true));
}

/// Kept by egui's renderer. Its textures are looked up while drawing, so
/// each frame's are the ones drawn: the font atlas grows as glyphs come.
struct Pipelines {
    opaque: wgpu::RenderPipeline,
    decal: wgpu::RenderPipeline,
    /// Weak: the renderer keeps these.
    renderer: Weak<RwLock<Renderer>>,
}

/// A frame's solids, for egui's renderer.
struct Gpu {
    solids: Arc<Solids>,
    /// Made while preparing, drawn from while painting.
    buffers: OnceLock<Buffers>,
}

struct Buffers {
    vertices: wgpu::Buffer,
    indices: wgpu::Buffer,
    draws: Vec<Draw>,
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
        _queue: &wgpu::Queue,
        screen: &ScreenDescriptor,
        _encoder: &mut wgpu::CommandEncoder,
        _resources: &mut CallbackResources,
    ) -> Vec<wgpu::CommandBuffer> {
        let [width, height] = screen
            .size_in_pixels
            .map(|pixels| pixels as f32 / screen.pixels_per_point);
        let (mut vertices, mut indices, mut draws) = (Vec::new(), Vec::new(), Vec::new());
        let mut count = 0u32;
        for (layer, solids) in self.solids.layers() {
            for solid in solids {
                let first = indices.len() as u32 / 4;
                for (vertex, depth) in solid.mesh.vertices.iter().zip(&solid.depths) {
                    let x = 2.0 * vertex.pos.x / width - 1.0;
                    let y = 1.0 - 2.0 * vertex.pos.y / height;
                    for value in [x, y, *depth, vertex.uv.x, vertex.uv.y] {
                        vertices.extend_from_slice(&value.to_le_bytes());
                    }
                    vertices.extend_from_slice(&vertex.color.to_array());
                }
                for index in &solid.mesh.indices {
                    indices.extend_from_slice(&index.to_le_bytes());
                }
                draws.push(Draw {
                    layer,
                    texture: solid.mesh.texture_id,
                    indices: first..first + solid.mesh.indices.len() as u32,
                    // Safe cast: far fewer vertices than i32 counts.
                    base: count as i32,
                });
                count += solid.mesh.vertices.len() as u32;
            }
        }
        let buffer = |label, contents: &[u8], usage| {
            device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some(label),
                contents,
                usage,
            })
        };
        let buffers = Buffers {
            vertices: buffer("depth_vertices", &vertices, wgpu::BufferUsages::VERTEX),
            indices: buffer("depth_indices", &indices, wgpu::BufferUsages::INDEX),
            draws,
        };
        // Prepared once a frame: a second would be the same.
        let _ = self.buffers.set(buffers);
        Vec::new()
    }

    fn paint(
        &self,
        _info: eframe::egui::PaintCallbackInfo,
        render_pass: &mut wgpu::RenderPass<'static>,
        resources: &CallbackResources,
    ) {
        let (Some(buffers), Some(pipelines)) = (self.buffers.get(), resources.get::<Pipelines>())
        else {
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
        for draw in &buffers.draws {
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

#[cfg(test)]
mod tests {
    use eframe::egui::pos2;

    use super::*;

    #[test]
    fn the_shader_is_valid() {
        use eframe::wgpu::naga;
        let module = naga::front::wgsl::parse_str(SHADER).unwrap();
        let mut validator = naga::valid::Validator::new(
            naga::valid::ValidationFlags::all(),
            naga::valid::Capabilities::empty(),
        );
        validator.validate(&module).unwrap();
    }

    #[test]
    fn subdividing_leaves_no_long_side_and_shares_its_cuts() {
        let mut mesh = Mesh::default();
        for (x, y) in [(0.0, 0.0), (10.0, 0.0), (10.0, 1.0), (0.0, 1.0)] {
            mesh.colored_vertex(pos2(x, y), Color32::WHITE);
        }
        mesh.add_triangle(0, 1, 2);
        mesh.add_triangle(0, 2, 3);
        let mesh = subdivided(mesh, 1.5);
        for triangle in mesh.indices.as_chunks::<3>().0 {
            let [a, b, c] = triangle.map(|i| mesh.vertices[i as usize].pos);
            for (p, q) in [(a, b), (b, c), (c, a)] {
                assert!(p.distance(q) <= 1.5, "{p:?} {q:?}");
            }
        }
        // Each cut made once, whichever triangle made it.
        let mut positions: Vec<_> = mesh.vertices.iter().map(|v| (v.pos.x, v.pos.y)).collect();
        positions.sort_by(|a, b| a.partial_cmp(b).unwrap());
        positions.dedup();
        assert_eq!(positions.len(), mesh.vertices.len());
    }

    #[test]
    fn a_hidden_vertex_drops_its_triangles_only() {
        let mut mesh = Mesh::default();
        for x in [0.0, 1.0, 2.0, 3.0] {
            mesh.colored_vertex(pos2(x, 0.0), Color32::WHITE);
        }
        mesh.add_triangle(0, 1, 2);
        mesh.add_triangle(1, 2, 3);
        let mut solids = Solids::default();
        solids.add(Layer::Opaque, mesh, |v| {
            v.pos.y += 1.0;
            (v.pos.x < 2.5).then_some(v.pos.x / 10.0)
        });
        let [(_, opaque), (_, decals)] = solids.layers();
        assert!(decals.is_empty());
        let solid = &opaque[0];
        assert_eq!(solid.mesh.indices, [0, 1, 2]);
        assert_eq!(solid.depths, [0.0, 0.1, 0.2]);
        assert_eq!(solid.mesh.vertices[1].pos, pos2(1.0, 1.0));
    }
}
