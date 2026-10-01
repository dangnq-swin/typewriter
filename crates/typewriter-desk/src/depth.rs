//! What stands in front hides what is behind: triangles drawn with a depth
//! buffer, in any order, through a paint callback on eframe's wgpu renderer.
//! The machine and the sheets draw so, in one callback a frame: [`begin`]
//! sets its place, parts [`gather`] into it, [`end`] draws it.
//!
//! Each vertex is egui's, on screen in points, with a depth added: 0 at the
//! eye to 1 far off, where it stands in machine inches, and how it takes the
//! frame's light ([`Shade`]). The shader lights each fragment from those
//! inches toward the lamp, so a flat face is not lit evenly. Opaque triangles
//! hide what is behind them; decals lie on them (print, edges, glass), hide
//! nothing and are drawn after. Where no renderer is installed, as in the
//! snapshot tool, the triangles go out as data for [`rasterize`] to fill.

use std::collections::HashMap;
use std::sync::{Arc, Weak};

use eframe::egui::epaint::{PaintCallback, TessellationOptions, Tessellator, Vertex};
use eframe::egui::layers::ShapeIdx;
use eframe::egui::mutex::RwLock;
use eframe::egui::{
    Color32, Context, CornerRadius, Id, Mesh, Painter, Pos2, Shape, StrokeKind, TextureId, Vec2,
};
use eframe::egui_wgpu::{self, CallbackResources, CallbackTrait, Renderer, ScreenDescriptor};
use eframe::wgpu;

/// The window's depth buffer, in bits: eframe's `depth_buffer`. One sample
/// a pixel, as egui's: wgpu's OpenGL backend shows nothing with more.
pub const DEPTH_BITS: u8 = 32;
const SHADER: &str = include_str!("depth.wgsl");
/// egui's font atlas, which text and plain fills (`WHITE_UV`) draw from.
const FONTS: TextureId = TextureId::Managed(0);
/// Position (3 floats), uv (2), colour (4 bytes), the way it faces (3
/// floats), its material's parameter (1 float), material (4 bytes),
/// highlight (4 bytes) and the inches it stands at (3 floats).
const VERTEX_BYTES: u64 = 60;
/// The lamp and the eye as `vec4`s, then chrome's bands as `vec4`s: as many
/// as the shader's `r_lighting`.
const LIGHTING_BYTES: u64 = 7 * 16;
/// How many bands chrome's room has.
pub const CHROME_BANDS: usize = 5;

/// Whether triangles hide what is behind them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Layer {
    Opaque,
    /// Lying on an opaque surface, drawn after: hides nothing.
    Decal,
}

/// A frame's lighting: where its lamp stands, the directions its materials
/// need and the room chrome mirrors. Kept in step with `r_lighting` in
/// `depth.wgsl`.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Lighting {
    /// Where the lamp stands, in machine inches from the printing point:
    /// each fragment is lit from there toward its own inches.
    pub lamp: [f32; 3],
    /// Toward the eye, unit length: a highlight is where a surface turns them
    /// both.
    pub eye: [f32; 3],
    /// Chrome from its top (0) to its bottom (1), each band's colour and how
    /// far down it starts.
    pub chrome: [(f32, Color32); CHROME_BANDS],
}

impl Lighting {
    /// The uniform the shader reads: the lamp, the eye, then the bands.
    fn uniform(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(LIGHTING_BYTES as usize);
        let mut word = |value: [f32; 4]| {
            for part in value {
                out.extend_from_slice(&part.to_le_bytes());
            }
        };
        word([self.lamp[0], self.lamp[1], self.lamp[2], 0.0]);
        word([self.eye[0], self.eye[1], self.eye[2], 0.0]);
        for &(down, colour) in &self.chrome {
            let [r, g, b] = gamma_rgb(colour);
            word([down, r, g, b]);
        }
        out
    }
}

/// How a vertex takes the frame's light.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub enum Shade {
    /// Its colour as it is.
    #[default]
    Unlit,
    /// Plastic facing the unit `normal`: lit over an ambient floor (Lambert).
    Matte([f32; 3]),
    /// Metal polished to a `shine` facing the unit `normal`: lit over an
    /// ambient floor, then a highlight `sharpness` narrow (Blinn–Phong).
    Polished {
        normal: [f32; 3],
        shine: Color32,
        sharpness: f32,
    },
    /// Brushed metal running along the unit `tangent`: its colour turned to
    /// `shine` by the light streaked across the grain (Heidrich–Seidel),
    /// `sharpness` narrow.
    Streak {
        tangent: [f32; 3],
        shine: Color32,
        sharpness: f32,
    },
    /// Chrome, `t` of the way down: bright sky above, a band where it turns
    /// to the light, the dark room below. Its own colour is unused.
    Chrome(f32),
}

impl Shade {
    /// `rgba` (premultiplied sRGB gamma, 0..=1) standing at `at`, in machine
    /// inches, as `lighting`'s lamp lights it. Keep in step with `depth.wgsl`.
    pub fn apply(self, rgba: [f32; 4], lighting: &Lighting, at: [f32; 3]) -> [f32; 4] {
        let [r, g, b, a] = rgba;
        let gamma = [r, g, b];
        // From these inches toward the lamp: what a fragment sees, the
        // direction from it to the light.
        let light = unit(sub(lighting.lamp, at));
        let eye = lighting.eye;
        // Brightened `by`, then turned toward `shine` by `toward`: as the
        // shader mixes a lit colour with its highlight.
        let lit = |by: f32, shine: [f32; 3], toward: f32| {
            [0, 1, 2].map(|k| {
                let base = (gamma[k] * by).min(1.0);
                base + (shine[k] - base) * toward
            })
        };
        let shade = match self {
            Self::Unlit => return rgba,
            Self::Matte(normal) => {
                let facing = dot(unit(normal), light).max(0.0);
                gamma.map(|c| (c * (0.7 + 0.4 * facing)).min(1.0))
            }
            Self::Polished {
                normal,
                shine,
                sharpness,
            } => {
                let normal = unit(normal);
                let facing = dot(normal, light).max(0.0);
                let highlight = dot(normal, unit(add(light, eye))).max(0.0).powf(sharpness);
                lit(0.5 + 0.6 * facing, gamma_rgb(shine), highlight)
            }
            Self::Streak {
                tangent,
                shine,
                sharpness,
            } => {
                let tangent = unit(tangent);
                let (lt, vt) = (dot(tangent, light), dot(tangent, eye));
                let across = (1.0 - lt * lt).max(0.0).sqrt() * (1.0 - vt * vt).max(0.0).sqrt();
                let streak = (across - lt * vt).max(0.0).powf(sharpness);
                lit(1.0, gamma_rgb(shine), streak)
            }
            Self::Chrome(t) => chrome(&lighting.chrome, t),
        };
        let [r, g, b] = shade;
        [r, g, b, a]
    }

    /// `colour` as [`Shade::apply`] lights it at `at`, in machine inches.
    pub fn colour(self, colour: Color32, lighting: &Lighting, at: [f32; 3]) -> Color32 {
        if self == Self::Unlit {
            return colour;
        }
        let rgba = colour.to_array().map(|c| f32::from(c) / 255.0);
        // Safe cast: clamped to a byte.
        let [r, g, b, a] = self
            .apply(rgba, lighting, at)
            .map(|c| (c.clamp(0.0, 1.0) * 255.0).round() as u8);
        Color32::from_rgba_premultiplied(r, g, b, a)
    }

    /// As the shader takes it: its material, the way it faces or runs along,
    /// its parameter and its highlight. The parameter is how narrow a
    /// highlight is, or how far down a chrome plate lies.
    fn parts(self) -> (u32, [f32; 3], f32, Color32) {
        match self {
            Self::Unlit => (0, [0.0; 3], 0.0, Color32::WHITE),
            Self::Matte(normal) => (1, normal, 0.0, Color32::WHITE),
            Self::Polished {
                normal,
                shine,
                sharpness,
            } => (2, normal, sharpness, shine),
            Self::Streak {
                tangent,
                shine,
                sharpness,
            } => (3, tangent, sharpness, shine),
            Self::Chrome(t) => (4, [0.0; 3], t, Color32::WHITE),
        }
    }
}

/// Chrome `t` of the way down `bands`, in gamma 0..=1. Keep in step with
/// `chrome` in `depth.wgsl`.
fn chrome(bands: &[(f32, Color32); CHROME_BANDS], t: f32) -> [f32; 3] {
    let at = t.clamp(0.0, 1.0);
    let last = bands.len() - 1;
    for i in 0..last {
        let (from, to) = (bands[i], bands[i + 1]);
        if at <= to.0 {
            let across = (at - from.0) / (to.0 - from.0);
            let (upper, lower) = (gamma_rgb(from.1), gamma_rgb(to.1));
            return [0, 1, 2].map(|k| upper[k] + (lower[k] - upper[k]) * across);
        }
    }
    gamma_rgb(bands[last].1)
}

/// `colour`'s rgb in gamma 0..=1.
fn gamma_rgb(colour: Color32) -> [f32; 3] {
    let [r, g, b, _] = colour.to_array();
    [r, g, b].map(|c| f32::from(c) / 255.0)
}

fn dot(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn add(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

fn sub(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn unit(v: [f32; 3]) -> [f32; 3] {
    let length = dot(v, v).sqrt().max(1e-6);
    v.map(|c| c / length)
}

/// Triangles in depth, in one texture: egui's mesh, and each vertex's depth,
/// the inches it stands at, and shade.
#[derive(Debug, Clone, Default)]
pub struct Solid {
    /// In egui's font atlas ([`FONTS`]), uvs are in texels: the atlas may
    /// grow until the frame ends, so they are normalised as it is drawn.
    pub mesh: Mesh,
    /// 0 at the eye to 1 far off, one for each of `mesh`'s vertices.
    pub depths: Vec<f32>,
    /// Where each of `mesh`'s vertices stands, in machine inches from the
    /// printing point: the lamp lights a fragment from these. What takes no
    /// shade says `[0.0; 3]`.
    pub places: Vec<[f32; 3]>,
    /// One for each of `mesh`'s vertices.
    pub shades: Vec<Shade>,
}

impl Solid {
    /// `mesh` at `depths`, each vertex its colour as it is.
    pub fn unlit(mesh: Mesh, depths: Vec<f32>) -> Self {
        let shades = vec![Shade::Unlit; depths.len()];
        let places = vec![[0.0; 3]; depths.len()];
        Self {
            mesh,
            depths,
            places,
            shades,
        }
    }
}

/// Where a vertex shows in a [`Solid`]: its depth, the inches it stands at,
/// and its take of the light.
#[derive(Debug, Clone, Copy, Default)]
pub struct Placing {
    pub depth: f32,
    pub at: [f32; 3],
    pub shade: Shade,
}

impl From<f32> for Placing {
    /// Unlit, needing no inches.
    fn from(depth: f32) -> Self {
        Self {
            depth,
            at: [0.0; 3],
            shade: Shade::Unlit,
        }
    }
}

/// What a frame draws in depth.
#[derive(Debug, Clone, Default)]
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
    /// it, and saying its [`Placing`]: depth, inches and shade. `None` hides
    /// a vertex, and the triangles touching it.
    pub fn add(
        &mut self,
        layer: Layer,
        mut mesh: Mesh,
        mut place: impl FnMut(&mut Vertex) -> Option<Placing>,
    ) {
        let placed: Vec<_> = mesh.vertices.iter_mut().map(&mut place).collect();
        let mut solid = Solid::unlit(Mesh::with_texture(mesh.texture_id), Vec::new());
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
                    let Placing { depth, at, shade } = placed[i].unwrap_or_default();
                    solid.depths.push(depth);
                    solid.places.push(at);
                    solid.shades.push(shade);
                    // Safe cast: a frame's vertices fit egui's own u32 indices.
                    solid.mesh.vertices.len() as u32 - 1
                });
                solid.mesh.indices.push(new);
            }
        }
        self.join(layer, solid);
    }

    /// Adds `solid` as it is.
    pub fn push(&mut self, layer: Layer, solid: Solid) {
        self.join(layer, solid);
    }

    /// Adds `other`'s solids after these.
    pub fn append(&mut self, other: Self) {
        for (layer, solids) in [(Layer::Opaque, other.opaque), (Layer::Decal, other.decals)] {
            for solid in solids {
                self.join(layer, solid);
            }
        }
    }

    /// Adds `solid`, joined to one of the same texture where it can: each
    /// solid is a draw call.
    fn join(&mut self, layer: Layer, solid: Solid) {
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
                into.places.extend(solid.places);
                into.shades.extend(solid.shades);
            }
            None => solids.push(solid),
        }
    }

    /// `shapes` drawn flat, cut to no side `longest` long to bend with what
    /// they are placed on, then each placed as by [`Solids::add`].
    /// An opaque shape's edges aren't feathered: a feather's clear fringe
    /// would hide what is behind it.
    pub fn add_shapes(
        &mut self,
        painter: &Painter,
        layer: Layer,
        shapes: Vec<Shape>,
        longest: f32,
        mut place: impl FnMut(&mut Vertex) -> Option<Placing>,
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

/// The frame's depth pass, between [`begin`] and [`end`].
#[derive(Clone, Default)]
struct Frame {
    /// Where it draws in the painter's order. `None`: not begun.
    slot: Option<ShapeIdx>,
    solids: Solids,
    /// How its solids take the light.
    lighting: Lighting,
}

/// A frame's depth pass as drawn: its solids, lit by `lighting`.
#[derive(Debug, Default)]
pub struct Pass {
    pub solids: Solids,
    /// How its solids take the light.
    pub lighting: Lighting,
}

fn frame_id() -> Id {
    Id::new("depth-pass-frame")
}

/// Starts the frame's depth pass, lit by `lighting`: what is [`gather`]ed
/// until [`end`] draws over what `painter` has drawn so far, under what it
/// draws after.
pub fn begin(painter: &Painter, lighting: Lighting) {
    let slot = painter.add(Shape::Noop);
    let frame = Frame {
        slot: Some(slot),
        solids: Solids::default(),
        lighting,
    };
    painter
        .ctx()
        .data_mut(|data| data.insert_temp(frame_id(), frame));
}

/// Adds `solids` to the frame's depth pass. Their depth decides only among
/// what is drawn in depth.
pub fn gather(ctx: &Context, solids: Solids) {
    ctx.data_mut(|data| {
        data.get_temp_mut_or_default::<Frame>(frame_id())
            .solids
            .append(solids);
    });
}

/// Draws the frame's depth pass where it [`begin`]s, or else now, with
/// `painter`'s clip.
pub fn end(painter: &Painter) {
    let Frame {
        slot,
        solids,
        lighting,
    } = painter
        .ctx()
        .data_mut(|data| data.remove_temp::<Frame>(frame_id()))
        .unwrap_or_default();
    if solids.is_empty() {
        return;
    }
    // The whole window: positions stay the window's.
    let rect = painter.ctx().viewport_rect();
    let pass = Arc::new(Pass { solids, lighting });
    let callback = if is_installed(painter.ctx()) {
        egui_wgpu::Callback::new_paint_callback(
            rect,
            Gpu {
                pass,
                ctx: painter.ctx().clone(),
            },
        )
    } else {
        PaintCallback {
            rect,
            callback: pass,
        }
    };
    match slot {
        Some(slot) => painter.set(slot, Shape::Callback(callback)),
        None => {
            painter.add(Shape::Callback(callback));
        }
    }
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
    let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("depth"),
        bind_group_layouts: &[Some(&textures), Some(&lighting_layout)],
        immediate_size: 0,
    });
    let format = render_state.target_format;
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
                        7 => Float32x3,
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
        lighting,
        lighting_group,
        renderer: Arc::downgrade(&render_state.renderer),
        buffers: None,
        draws: Vec::new(),
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
    /// The frame's lighting, written each frame.
    lighting: wgpu::Buffer,
    lighting_group: wgpu::BindGroup,
    /// Weak: the renderer keeps these.
    renderer: Weak<RwLock<Renderer>>,
    /// Refilled each frame, grown when too small: one pass a frame.
    buffers: Option<Buffers>,
    /// This frame's, into `buffers`.
    draws: Vec<Draw>,
}

/// A frame's pass, for egui's renderer.
struct Gpu {
    pass: Arc<Pass>,
    /// For the font atlas's size once the frame is over.
    ctx: Context,
}

struct Buffers {
    vertices: wgpu::Buffer,
    indices: wgpu::Buffer,
}

impl Buffers {
    /// Room for at least `vertex_bytes` and `index_bytes`, to the next
    /// power of two: a growing page grows them seldom.
    fn new(device: &wgpu::Device, vertex_bytes: u64, index_bytes: u64) -> Self {
        let buffer = |label, bytes: u64, usage| {
            device.create_buffer(&wgpu::BufferDescriptor {
                label: Some(label),
                size: bytes.next_power_of_two(),
                usage: usage | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            })
        };
        Self {
            vertices: buffer("depth_vertices", vertex_bytes, wgpu::BufferUsages::VERTEX),
            indices: buffer("depth_indices", index_bytes, wgpu::BufferUsages::INDEX),
        }
    }

    fn holds(&self, vertex_bytes: u64, index_bytes: u64) -> bool {
        self.vertices.size() >= vertex_bytes && self.indices.size() >= index_bytes
    }
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
    // Safe cast: a vertex's bytes.
    let mut vertices = Vec::with_capacity(vertex_count * VERTEX_BYTES as usize);
    let mut indices = Vec::with_capacity(index_count * 4);
    let mut draws = Vec::new();
    let (mut first, mut base) = (0, 0);
    for (layer, solid) in all() {
        let texels = texel_scale(solid.mesh.texture_id, fonts);
        let placed = solid.depths.iter().zip(&solid.places).zip(&solid.shades);
        for (vertex, ((depth, &at), shade)) in solid.mesh.vertices.iter().zip(placed) {
            let x = 2.0 * vertex.pos.x / width - 1.0;
            let y = 1.0 - 2.0 * vertex.pos.y / height;
            let uv = vertex.uv.to_vec2() * texels;
            let (material, vector, spec, shine) = shade.parts();
            for value in [x, y, *depth, uv.x, uv.y] {
                vertices.extend_from_slice(&value.to_le_bytes());
            }
            vertices.extend_from_slice(&vertex.color.to_array());
            for value in vector.into_iter().chain([spec]) {
                vertices.extend_from_slice(&value.to_le_bytes());
            }
            vertices.extend_from_slice(&material.to_le_bytes());
            vertices.extend_from_slice(&shine.to_array());
            for value in at {
                vertices.extend_from_slice(&value.to_le_bytes());
            }
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
        queue: &wgpu::Queue,
        screen: &ScreenDescriptor,
        _encoder: &mut wgpu::CommandEncoder,
        resources: &mut CallbackResources,
    ) -> Vec<wgpu::CommandBuffer> {
        let Some(pipelines) = resources.get_mut::<Pipelines>() else {
            return Vec::new();
        };
        let size = screen
            .size_in_pixels
            .map(|pixels| pixels as f32 / screen.pixels_per_point);
        // The frame is over: the atlas is as large as it gets this frame.
        let fonts = self.ctx.fonts(|fonts| fonts.font_image_size());
        let (vertices, indices, draws) = pack(&self.pass.solids, size, fonts);
        queue.write_buffer(&pipelines.lighting, 0, &self.pass.lighting.uniform());
        // Safe casts: byte counts.
        let (vertex_bytes, index_bytes) = (vertices.len() as u64, indices.len() as u64);
        let buffers = match pipelines.buffers.take() {
            Some(buffers) if buffers.holds(vertex_bytes, index_bytes) => buffers,
            _ => Buffers::new(device, vertex_bytes, index_bytes),
        };
        queue.write_buffer(&buffers.vertices, 0, &vertices);
        queue.write_buffer(&buffers.indices, 0, &indices);
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

/// Fills `callback`'s pass inside `clip` in a snapshot, with a depth buffer
/// kept through the frame, each pixel lit as the shader lights it.
#[cfg(test)]
pub fn rasterize(
    callback: &PaintCallback,
    clip: eframe::egui::Rect,
    raster: &mut typewriter_app::snapshot::Raster,
) {
    let Some(Pass { solids, lighting }) = callback.callback.downcast_ref::<Pass>() else {
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
            let keep = |pixel: usize, triangle: [u32; 3], weights: [f32; 3]| {
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
            };
            let shade = |triangle: [u32; 3], weights: [f32; 3], rgba| {
                // The shader blends each vertex's inches across the triangle
                // affine, in screen space: this blends them the same way, to
                // find the fragment's place on the lamp.
                let at = [0, 1, 2].map(|k| {
                    (0..3)
                        .map(|c| weights[c] * solid.places[triangle[c] as usize][k])
                        .sum()
                });
                pixel_shade(&solid.shades, triangle, weights).apply(rgba, lighting, at)
            };
            raster.fill(&mesh, clip, keep, shade);
        }
    }
    raster.frame = Some(Box::new(buffer));
}

/// The shade at a pixel `weights` across `triangle`, as the shader makes it:
/// the way the surface faces or runs along and the material's parameters
/// blended across it, its material and its highlight taken from the first
/// corner, which is what the shader's flat attributes give.
#[cfg(test)]
fn pixel_shade(shades: &[Shade], triangle: [u32; 3], weights: [f32; 3]) -> Shade {
    let corners = triangle.map(|i| shades[i as usize]);
    let first = corners[0];
    if first == Shade::Unlit {
        return first;
    }
    let mut vector = [0.0; 3];
    let mut spec = 0.0;
    for (shade, weight) in corners.into_iter().zip(weights) {
        let (_, faced, param, _) = shade.parts();
        for k in 0..3 {
            vector[k] += faced[k] * weight;
        }
        spec += param * weight;
    }
    vector = unit(vector);
    let (sharpness, down) = (spec, spec);
    match first {
        Shade::Matte(_) => Shade::Matte(vector),
        Shade::Polished { shine, .. } => Shade::Polished {
            normal: vector,
            shine,
            sharpness,
        },
        Shade::Streak { shine, .. } => Shade::Streak {
            tangent: vector,
            shine,
            sharpness,
        },
        Shade::Chrome(_) => Shade::Chrome(down),
        Shade::Unlit => first,
    }
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
        let mut lit = Solid::unlit(triangle(Color32::RED), vec![0.25; 3]);
        lit.shades = vec![Shade::Matte([0.0, 0.0, 1.0]); 3];
        lit.places = vec![[1.0, 2.0, 3.5]; 3];
        solids.push(Layer::Opaque, lit);
        solids.add(Layer::Decal, triangle(Color32::BLUE), |_| Some(0.5.into()));
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
        assert_eq!(
            [
                float(VERTEX_BYTES as usize),
                float(VERTEX_BYTES as usize + 4)
            ],
            [1.0, 1.0]
        );
        assert_eq!(vertices[20..24], Color32::RED.to_array());
        // Its normal, parameters, material and shine, then the unlit decal's.
        assert_eq!([float(24), float(28), float(32)], [0.0, 0.0, 1.0]);
        assert_eq!(float(36), 0.0);
        assert_eq!(vertices[40..44], 1u32.to_le_bytes());
        // And where it stands, in inches, for the lamp.
        assert_eq!([float(48), float(52), float(56)], [1.0, 2.0, 3.5]);
        let next = 3 * VERTEX_BYTES as usize;
        assert_eq!(vertices[next + 40..next + 44], 0u32.to_le_bytes());
        assert_eq!(
            [float(next + 48), float(next + 52), float(next + 56)],
            [0.0, 0.0, 0.0]
        );
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
            normal: [0.0, 0.0, 1.0],
            shine,
            sharpness: 24.0,
        };
        let mut solids = Solids::default();
        let mut lit = Solid::unlit(mesh.clone(), vec![0.25; 3]);
        lit.shades = vec![polished; 3];
        solids.push(Layer::Opaque, lit);
        let mut down = Solid::unlit(mesh, vec![0.5; 3]);
        down.shades = vec![Shade::Chrome(0.75); 3];
        solids.push(Layer::Opaque, down);
        let (vertices, _, _) = pack(&solids, [1600.0, 1000.0], [64, 32]);
        let float = |at: usize| f32::from_le_bytes(vertices[at..at + 4].try_into().unwrap());
        // A polished vertex: its normal, its sharpness, its material, its shine.
        assert_eq!([float(24), float(28), float(32)], [0.0, 0.0, 1.0]);
        assert_eq!(float(36), 24.0);
        assert_eq!(vertices[40..44], 2u32.to_le_bytes());
        assert_eq!(vertices[44..48], shine.to_array());
        // A chrome one: how far down it lies, and no shine of its own.
        let next = VERTEX_BYTES as usize * 3;
        assert_eq!(float(next + 36), 0.75);
        assert_eq!(vertices[next + 40..next + 44], 4u32.to_le_bytes());
        assert_eq!(vertices[next + 44..next + 48], Color32::WHITE.to_array());
    }

    /// A frame with its lamp straight above, the eye above too, chrome from
    /// white at its top to black at its foot.
    fn above() -> Lighting {
        let band = |grey: u8| Color32::from_rgb(grey, grey, grey);
        Lighting {
            lamp: [0.0, 0.0, 10.0],
            eye: [0.0, 0.0, 1.0],
            chrome: [
                (0.0, band(0xFF)),
                (0.25, band(0x80)),
                (0.5, band(0xFF)),
                (0.75, band(0x40)),
                (1.0, band(0x00)),
            ],
        }
    }

    #[test]
    fn polished_metal_shines_where_it_turns_the_light_to_the_eye() {
        let grey = [0.5, 0.5, 0.5, 1.0];
        let shine = Color32::from_rgb(0xFF, 0xFF, 0xFF);
        let polished = |normal, sharpness| {
            Shade::Polished {
                normal,
                shine,
                sharpness,
            }
            .apply(grey, &above(), [0.0; 3])
        };
        // Side-on to both: the ambient floor alone, half as bright.
        assert_eq!(polished([1.0, 0.0, 0.0], 4.0), [0.25, 0.25, 0.25, 1.0]);
        // Facing them both: wholly the shine.
        assert!(polished([0.0, 0.0, 1.0], 4.0)[0] > 0.999);
        // A narrower highlight catches less of what turns aside.
        let aside = [0.0, 0.2, 1.0];
        assert!(polished(aside, 24.0)[0] < polished(aside, 4.0)[0]);
    }

    #[test]
    fn brushed_metal_streaks_across_its_grain() {
        let grey = [0.5, 0.5, 0.5, 1.0];
        let shine = Color32::from_rgb(0xFF, 0xFF, 0xFF);
        let streak = |tangent| {
            Shade::Streak {
                tangent,
                shine,
                sharpness: 6.0,
            }
            .apply(grey, &above(), [0.0; 3])[0]
        };
        // Across the light: the whole highlight. Along it: none at all.
        assert!(streak([1.0, 0.0, 0.0]) > 0.99);
        assert!((streak([0.0, 0.0, 1.0]) - 0.5).abs() < 1e-3);
    }

    #[test]
    fn chrome_reads_the_room_down_its_plate() {
        let light = above();
        let chrome = |t| Shade::Chrome(t).apply([0.0, 0.0, 0.0, 1.0], &light, [0.0; 3])[0] * 255.0;
        assert_eq!(chrome(0.0), 255.0);
        assert_eq!(chrome(0.5), 255.0);
        assert_eq!(chrome(1.0), 0.0);
        // Halfway between two bands.
        assert!((chrome(0.875) - 32.0).abs() < 0.01, "{:?}", chrome(0.875));
        // Past the bottom it stays the room's dark.
        assert_eq!(chrome(1.5), 0.0);
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
            solids.add(layer, quad(texture), |v| Some(v.pos.x.into()));
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
    fn one_pass_a_frame_where_it_began() {
        let ctx = Context::default();
        let triangle = |depth: f32| {
            let mut mesh = Mesh::default();
            for (x, y) in [(0.0, 0.0), (10.0, 0.0), (0.0, 10.0)] {
                mesh.colored_vertex(pos2(x, y), Color32::WHITE);
            }
            mesh.add_triangle(0, 1, 2);
            let mut solids = Solids::default();
            solids.add(Layer::Opaque, mesh, |_| Some(depth.into()));
            solids
        };
        let mut output = ctx.run_ui(Default::default(), |ui| {
            let painter = ui.painter();
            begin(painter, Lighting::default());
            gather(&ctx, triangle(0.5));
            painter.rect_filled(
                Rect::from_min_size(pos2(0.0, 0.0), Vec2::splat(5.0)),
                0,
                Color32::RED,
            );
            gather(&ctx, triangle(0.25));
            end(painter);
        });
        output.textures_delta.clear();
        let shapes: Vec<_> = output.shapes.iter().map(|s| &s.shape).collect();
        let callbacks: Vec<_> = shapes
            .iter()
            .enumerate()
            .filter_map(|(i, s)| match s {
                Shape::Callback(c) => Some((i, &c.callback.downcast_ref::<Pass>()?.solids)),
                _ => None,
            })
            .collect();
        let [(at, solids)] = callbacks[..] else {
            panic!("{shapes:?}");
        };
        // Under the flat shape drawn after it began, both triangles joined.
        assert!(matches!(shapes[at + 1], Shape::Rect(_)), "{shapes:?}");
        let [(_, opaque), _] = solids.layers();
        assert_eq!(opaque.len(), 1);
        assert_eq!(
            opaque[0].depths,
            [0.5; 3].into_iter().chain([0.25; 3]).collect::<Vec<_>>()
        );
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
            (v.pos.x < 2.5).then(|| (v.pos.x / 10.0).into())
        });
        let [(_, opaque), (_, decals)] = solids.layers();
        assert!(decals.is_empty());
        let solid = &opaque[0];
        assert_eq!(solid.mesh.indices, [0, 1, 2]);
        assert_eq!(solid.depths, [0.0, 0.1, 0.2]);
        assert_eq!(solid.mesh.vertices[1].pos, pos2(1.0, 1.0));
    }
}
