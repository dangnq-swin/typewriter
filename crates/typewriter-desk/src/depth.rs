//! What stands in front hides what is behind: triangles drawn with a depth
//! buffer, in any order, through a paint callback on eframe's wgpu renderer.
//! The machine and the sheets draw so.
//!
//! Each vertex is egui's, on screen in points, with a depth added: 0 at the
//! eye to 1 far off. Opaque triangles hide what is behind them; decals lie
//! on them (print, edges, glass), hide nothing and are drawn after. Where no
//! renderer is installed, as in the snapshot tool, the triangles go out as
//! data for [`rasterize`] to fill.

use std::collections::HashMap;
use std::sync::{Arc, OnceLock, Weak};

use eframe::egui::epaint::{PaintCallback, TessellationOptions, Tessellator, Vertex};
use eframe::egui::mutex::RwLock;
use eframe::egui::{
    Color32, Context, CornerRadius, Id, Mesh, Painter, Pos2, Shape, StrokeKind, TextureId, Vec2,
};
use eframe::egui_wgpu::{self, CallbackResources, CallbackTrait, Renderer, ScreenDescriptor};
use eframe::wgpu::{self, util::DeviceExt};

/// The window's depth buffer, in bits: eframe's `depth_buffer`. One sample
/// a pixel, as egui's: wgpu's OpenGL backend shows nothing with more.
pub const DEPTH_BITS: u8 = 32;
const SHADER: &str = include_str!("depth.wgsl");
/// egui's font atlas, which text and plain fills (`WHITE_UV`) draw from.
const FONTS: TextureId = TextureId::Managed(0);
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
    /// In egui's font atlas ([`FONTS`]), uvs are in texels: the atlas may
    /// grow until the frame ends, so they are normalised as it is drawn.
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
    /// touching it. Joins a solid of the same texture where it can: each
    /// solid is a draw call.
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
        let texture = solid.mesh.texture_id;
        // Opaque faces hide by depth, in any order; decals blend in order,
        // so join only the last.
        let (solids, into) = match layer {
            Layer::Opaque => {
                let at = self
                    .opaque
                    .iter()
                    .position(|s| s.mesh.texture_id == texture);
                (&mut self.opaque, at)
            }
            Layer::Decal => {
                let last = self.decals.last().filter(|s| s.mesh.texture_id == texture);
                let at = last.map(|_| self.decals.len() - 1);
                (&mut self.decals, at)
            }
        };
        match into {
            Some(at) => {
                let into = &mut solids[at];
                into.mesh.append(solid.mesh);
                into.depths.extend(solid.depths);
            }
            None => solids.push(solid),
        }
    }

    /// `shapes` drawn flat, cut to no side `longest` long to bend with what
    /// they are placed on, then each vertex placed as by [`Solids::add`].
    /// An opaque shape's edges aren't feathered: a feather's clear fringe
    /// would hide what is behind it.
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
        // An atlas a texel across: text's uvs stay in texels (see `Solid`).
        let mut tessellator =
            Tessellator::new(painter.pixels_per_point(), options, [1, 1], Vec::new());
        let mut mesh = Mesh::default();
        for shape in shapes {
            let mut one = Mesh::default();
            tessellator.tessellate_shape(cut(shape, longest), &mut one);
            if one.texture_id != mesh.texture_id && !mesh.is_empty() {
                let flat = subdivided(std::mem::take(&mut mesh), longest);
                self.add(layer, flat, &mut place);
            }
            mesh.texture_id = one.texture_id;
            mesh.append(one);
        }
        self.add(layer, subdivided(mesh, longest), place);
    }
}

/// `shape` with its lines cut into pieces short enough that their strokes
/// need no [`subdivided`]: halving a stroke's long, thin triangles leaves
/// thin ones again, and their count explodes. Other shapes unchanged.
fn cut(shape: Shape, longest: f32) -> Shape {
    if !longest.is_finite() {
        return shape;
    }
    // Half: a piece's quad, diagonal and all, is then short enough.
    let step = longest / 2.0;
    match shape {
        Shape::Vec(shapes) => Shape::Vec(shapes.into_iter().map(|s| cut(s, longest)).collect()),
        Shape::LineSegment { points, stroke } => {
            Shape::line(cut_points(&points, false, step), stroke)
        }
        Shape::Path(mut path) if path.fill == Color32::TRANSPARENT => {
            path.points = cut_points(&path.points, path.closed, step);
            Shape::Path(path)
        }
        Shape::Rect(rect)
            if rect.fill == Color32::TRANSPARENT
                && rect.corner_radius == CornerRadius::ZERO
                && rect.blur_width == 0.0
                && rect.brush.is_none() =>
        {
            let half = rect.stroke.width / 2.0;
            let outline = match rect.stroke_kind {
                StrokeKind::Inside => rect.rect.shrink(half),
                StrokeKind::Middle => rect.rect,
                StrokeKind::Outside => rect.rect.expand(half),
            };
            let corners = [
                outline.left_top(),
                outline.right_top(),
                outline.right_bottom(),
                outline.left_bottom(),
            ];
            Shape::closed_line(cut_points(&corners, true, step), rect.stroke)
        }
        shape => shape,
    }
}

/// `points` with more between them, none more than `step` apart; `closed`
/// cuts the way back to the first too.
fn cut_points(points: &[Pos2], closed: bool, step: f32) -> Vec<Pos2> {
    let ends = points.iter().zip(points.iter().skip(1));
    let back = closed.then(|| points.last().zip(points.first())).flatten();
    let mut out = Vec::with_capacity(points.len());
    for (&a, &b) in ends.chain(back) {
        // Safe cast: a line on screen in steps.
        let pieces = (a.distance(b) / step).ceil().max(1.0) as u32;
        out.extend((0..pieces).map(|i| a.lerp(b, i as f32 / pieces as f32)));
    }
    if !closed {
        out.extend(points.last());
    }
    out
}

/// `mesh` with no triangle's side longer than `longest`: each long side cut
/// in half, its neighbour sharing the cut, until none is. For triangles
/// near even-sided: thin ones multiply. Keeps the triangles' order, which
/// decals blend in.
fn subdivided(mesh: Mesh, longest: f32) -> Mesh {
    // Laid flat where it lies: nothing to cut, so spare the walk.
    if !longest.is_finite() {
        return mesh;
    }
    let Mesh {
        indices,
        mut vertices,
        texture_id,
    } = mesh;
    let mut halves = HashMap::new();
    let mut out = Vec::with_capacity(indices.len());
    let length = |vertices: &[Vertex], a: u32, b: u32| {
        vertices[a as usize].pos.distance(vertices[b as usize].pos)
    };
    // Each triangle's pieces before the next triangle's, first half first.
    let mut todo = Vec::new();
    for &triangle in indices.as_chunks::<3>().0 {
        todo.push(triangle);
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
                    color: Color32::from_rgba_premultiplied(
                        colour(0),
                        colour(1),
                        colour(2),
                        colour(3),
                    ),
                });
                // Safe cast: a frame's vertices fit egui's own u32 indices.
                vertices.len() as u32 - 1
            });
            todo.push([half, b, c]);
            todo.push([a, half, c]);
        }
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
                ctx: painter.ctx().clone(),
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
    /// For the font atlas's size once the frame is over.
    ctx: Context,
    /// Made while preparing, drawn from while painting.
    buffers: OnceLock<Buffers>,
}

struct Buffers {
    vertices: wgpu::Buffer,
    indices: wgpu::Buffer,
    draws: Vec<Draw>,
}

/// `solids` as the GPU takes them, for a window `size` points and the font
/// atlas `fonts` texels, as it is once the frame is over: vertex and index
/// bytes, and a draw for each solid, opaque first.
fn pack(
    solids: &Solids,
    [width, height]: [f32; 2],
    fonts: [usize; 2],
) -> (Vec<u8>, Vec<u8>, Vec<Draw>) {
    let all = || {
        let layers = solids.layers().into_iter();
        layers.flat_map(|(layer, solids)| solids.iter().map(move |solid| (layer, solid)))
    };
    let vertex_count: usize = all().map(|(_, solid)| solid.mesh.vertices.len()).sum();
    let index_count: usize = all().map(|(_, solid)| solid.mesh.indices.len()).sum();
    // Safe cast: a vertex's 24 bytes.
    let mut vertices = Vec::with_capacity(vertex_count * VERTEX_BYTES as usize);
    let mut indices = Vec::with_capacity(index_count * 4);
    let mut draws = Vec::new();
    let (mut first, mut base) = (0, 0);
    for (layer, solid) in all() {
        let texels = texel_scale(solid.mesh.texture_id, fonts);
        for (vertex, depth) in solid.mesh.vertices.iter().zip(&solid.depths) {
            let x = 2.0 * vertex.pos.x / width - 1.0;
            let y = 1.0 - 2.0 * vertex.pos.y / height;
            let uv = vertex.uv.to_vec2() * texels;
            for value in [x, y, *depth, uv.x, uv.y] {
                vertices.extend_from_slice(&value.to_le_bytes());
            }
            vertices.extend_from_slice(&vertex.color.to_array());
        }
        for index in &solid.mesh.indices {
            indices.extend_from_slice(&index.to_le_bytes());
        }
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
fn texel_scale(texture: TextureId, fonts: [usize; 2]) -> Vec2 {
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
        _queue: &wgpu::Queue,
        screen: &ScreenDescriptor,
        _encoder: &mut wgpu::CommandEncoder,
        _resources: &mut CallbackResources,
    ) -> Vec<wgpu::CommandBuffer> {
        let size = screen
            .size_in_pixels
            .map(|pixels| pixels as f32 / screen.pixels_per_point);
        // The frame is over: the atlas is as large as it gets this frame.
        let fonts = self.ctx.fonts(|fonts| fonts.font_image_size());
        let (vertices, indices, draws) = pack(&self.solids, size, fonts);
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

/// Fills `callback`'s solids inside `clip` in a snapshot, with a depth
/// buffer kept through the frame.
#[cfg(test)]
pub fn rasterize(
    callback: &PaintCallback,
    clip: eframe::egui::Rect,
    raster: &mut typewriter_app::snapshot::Raster,
) {
    let Some(solids) = callback.callback.downcast_ref::<Solids>() else {
        return;
    };
    let [width, height] = raster.size();
    // Cleared once a frame, as the window's.
    let mut buffer = raster
        .frame
        .take()
        .and_then(|frame| frame.downcast::<Vec<f32>>().ok())
        .map_or_else(|| vec![1.0; width * height], |buffer| *buffer);
    for (layer, solids) in solids.layers() {
        for solid in solids {
            let fonts = raster.texture_size(FONTS).unwrap_or([1, 1]);
            let texels = texel_scale(solid.mesh.texture_id, fonts);
            let mut mesh = solid.mesh.clone();
            for vertex in &mut mesh.vertices {
                vertex.uv = (vertex.uv.to_vec2() * texels).to_pos2();
            }
            raster.fill(&mesh, clip, |pixel, triangle, weights| {
                let depth: f32 = (0..3)
                    .map(|k| solid.depths[triangle[k] as usize] * weights[k])
                    .sum();
                let there = &mut buffer[pixel];
                match layer {
                    Layer::Opaque if depth < *there => {
                        *there = depth;
                        true
                    }
                    Layer::Decal => depth <= *there,
                    Layer::Opaque => false,
                }
            });
        }
    }
    raster.frame = Some(Box::new(buffer));
}

#[cfg(test)]
mod tests {
    use eframe::egui::{Rect, Stroke, pos2};

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
    fn cut_lines_need_no_subdividing() {
        let (longest, stroke) = (20.0, Stroke::new(1.0, Color32::BLACK));
        let rect = Rect::from_min_max(pos2(0.0, 0.0), pos2(600.0, 1000.0));
        // Each with its length.
        let shapes = [
            (
                Shape::line_segment([pos2(0.0, 0.0), pos2(1000.0, 0.0)], stroke),
                1000.0,
            ),
            (
                Shape::rect_stroke(rect, CornerRadius::ZERO, stroke, StrokeKind::Middle),
                3200.0,
            ),
        ];
        for (shape, length) in shapes {
            let options = TessellationOptions::default();
            let mut tessellator = Tessellator::new(1.0, options, [1, 1], Vec::new());
            let mut mesh = Mesh::default();
            tessellator.tessellate_shape(cut(shape, longest), &mut mesh);
            let count = mesh.vertices.len();
            // A feathered stroke: four vertices a piece, give or take its ends.
            let pieces = length / (longest / 2.0);
            assert!((count as f32) < 4.0 * (pieces + 2.0), "{count}");
            assert_eq!(subdivided(mesh, longest).vertices.len(), count);
        }
    }

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
        solids.add(Layer::Opaque, triangle(Color32::RED), |_| Some(0.25));
        solids.add(Layer::Decal, triangle(Color32::BLUE), |_| Some(0.5));
        let (vertices, indices, draws) = pack(&solids, [1600.0, 1000.0], [64, 32]);
        assert_eq!(vertices.len(), 6 * VERTEX_BYTES as usize);
        assert_eq!(indices.len(), 6 * 4);
        let runs: Vec<_> = draws
            .iter()
            .map(|d| (d.layer, d.indices.clone(), d.base))
            .collect();
        assert_eq!(runs, [(Layer::Opaque, 0..3, 0), (Layer::Decal, 3..6, 3)]);
        let float = |at: usize| f32::from_le_bytes(vertices[at..at + 4].try_into().unwrap());
        // The window's middle, then its top right, in device coordinates.
        assert_eq!([float(0), float(4), float(8)], [0.0, 0.0, 0.25]);
        assert_eq!([float(24), float(28)], [1.0, 1.0]);
        assert_eq!(vertices[20..24], Color32::RED.to_array());
    }

    #[test]
    fn solids_join_by_texture_decals_only_in_order() {
        let quad = |texture: u64| {
            let mut mesh = Mesh::with_texture(TextureId::User(texture));
            for x in [0.0, 1.0, 1.0] {
                mesh.vertices.push(Vertex {
                    pos: pos2(x, x),
                    uv: pos2(0.0, 0.0),
                    color: Color32::WHITE,
                });
            }
            mesh.add_triangle(0, 1, 2);
            mesh
        };
        let mut solids = Solids::default();
        for (layer, texture) in [
            (Layer::Opaque, 1),
            (Layer::Opaque, 2),
            (Layer::Opaque, 1),
            (Layer::Decal, 1),
            (Layer::Decal, 1),
            (Layer::Decal, 2),
            (Layer::Decal, 1),
        ] {
            solids.add(layer, quad(texture), |v| Some(v.pos.x));
        }
        let [(_, opaque), (_, decals)] = solids.layers();
        let textures = |solids: &[Solid]| -> Vec<TextureId> {
            solids.iter().map(|s| s.mesh.texture_id).collect()
        };
        assert_eq!(textures(opaque), [TextureId::User(1), TextureId::User(2)]);
        let order = [1, 2, 1].map(TextureId::User);
        assert_eq!(textures(decals), order);
        // The joined mesh indexes its own vertices, each with its depth.
        let joined = &opaque[0];
        assert_eq!(joined.mesh.indices, [0, 1, 2, 3, 4, 5]);
        assert_eq!(joined.depths.len(), joined.mesh.vertices.len());
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

    #[test]
    fn subdividing_keeps_the_triangles_order() {
        // Three marks over one place, in the order struck: the last on top.
        let mut mesh = Mesh::default();
        for colour in [Color32::RED, Color32::GREEN, Color32::BLUE] {
            let first = mesh.vertices.len() as u32;
            for (x, y) in [(0.0, 0.0), (10.0, 0.0), (0.0, 10.0)] {
                mesh.colored_vertex(pos2(x, y), colour);
            }
            mesh.add_triangle(first, first + 1, first + 2);
        }
        let mesh = subdivided(mesh, 3.0);
        let colours: Vec<Color32> = mesh
            .indices
            .as_chunks::<3>()
            .0
            .iter()
            .map(|t| mesh.vertices[t[0] as usize].color)
            .collect();
        let mut runs = colours.clone();
        runs.dedup();
        assert_eq!(runs, [Color32::RED, Color32::GREEN, Color32::BLUE]);
        assert!(colours.len() > 3, "cut");
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
