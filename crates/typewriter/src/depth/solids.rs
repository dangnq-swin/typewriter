//! What a frame draws in depth: a [`Solid`] is one texture's triangles with
//! the millimetres each vertex stands at and how it takes the light;
//! [`Solids`] gathers them, joined by texture into as few draws as the
//! layers allow. [`Layer`] says whether a solid hides what is behind it.
//! The millimetres are the truth and the screen positions follow them —
//! [`Solids::show`] redraws them at the end of a frame — so a solid built
//! once can be kept across frames and [`Solids::shifted`] into a moving
//! part's place without being re-tessellated.

use std::collections::HashMap;

use eframe::egui::epaint::{TessellationOptions, Tessellator, Vertex};
use eframe::egui::{Color32, CornerRadius, Mesh, Painter, Pos2, Shape, StrokeKind};
use glam::Vec3;

use super::Camera;
use super::lighting::Shade;

/// Whether triangles hide what is behind them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Layer {
    Opaque,
    /// Lying on an opaque surface, drawn after: hides nothing, and lifted
    /// toward the eye by the pass's depth bias (`decal_bias`) so it shows
    /// over the face it lies on.
    Decal,
}

/// Triangles in depth, in one texture: egui's mesh, and each vertex's
/// millimetres and shade. The pass's `Camera` projects the millimetres:
/// depth is not carried, it is what they come to.
#[derive(Debug, Clone, Default)]
pub struct Solid {
    /// In egui's font atlas (the pass's `FONTS`), uvs are in texels: the
    /// atlas may grow until the frame ends, so they are normalised as it is
    /// drawn.
    pub mesh: Mesh,
    /// Where each of `mesh`'s vertices stands, in absolute machine
    /// millimetres: the pass projects these into the window, and the lamp
    /// lights a fragment from them.
    pub places: Vec<Vec3>,
    /// One for each of `mesh`'s vertices.
    pub shades: Vec<Shade>,
}

impl Solid {
    /// `mesh` standing at `places`, each vertex its colour as it is.
    pub fn unlit(mesh: Mesh, places: Vec<Vec3>) -> Self {
        let shades = vec![Shade::Unlit; places.len()];
        Self {
            mesh,
            places,
            shades,
        }
    }
}

/// Where a vertex shows in a [`Solid`]: the absolute millimetres it stands
/// at, and its take of the light.
#[derive(Debug, Clone, Copy, Default)]
pub struct Placing {
    pub at: Vec3,
    pub shade: Shade,
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

    /// `self` with every vertex standing `by` millimetres further off:
    /// a moving part gathered from solids kept at rest. What a kept mesh's
    /// vertices show on screen is stale by then; [`Solids::show`] redraws
    /// them, and the shade a face takes rides in vectors that a shift
    /// cannot change — a turning part wants its producer to rebuild rather
    /// than rotate.
    pub fn shifted(&self, by: Vec3) -> Self {
        let mut out = self.clone();
        for (_, solids) in out.layers_mut() {
            for solid in solids {
                for place in &mut solid.places {
                    *place += by;
                }
            }
        }
        out
    }

    /// Redraw each vertex where its own millimetres show through `camera`:
    /// what the GPU projects from, and now what the CPU twin fills and
    /// [`Solids::shifted`] leave stale. The producer's screen positions are
    /// only the frame they were built in: a kept solid owes the pass its
    /// millimetres, not those. Skipped for a default camera, as the
    /// hand-built test solids carry: there the producers' positions stand.
    pub(super) fn show(&mut self, camera: &Camera) {
        if camera.points.x <= 0.0 || camera.points.y <= 0.0 {
            return;
        }
        for (_, solids) in self.layers_mut() {
            for solid in solids {
                for (vertex, &mm) in solid.mesh.vertices.iter_mut().zip(&solid.places) {
                    vertex.pos = camera.on_screen(mm);
                }
            }
        }
    }

    /// Opaque, then decals: the order they are drawn in.
    pub fn layers(&self) -> [(Layer, &[Solid]); 2] {
        [(Layer::Opaque, &self.opaque), (Layer::Decal, &self.decals)]
    }

    fn layers_mut(&mut self) -> [(Layer, &mut Vec<Solid>); 2] {
        [
            (Layer::Opaque, &mut self.opaque),
            (Layer::Decal, &mut self.decals),
        ]
    }

    /// Adds `mesh`, `place` moving each vertex where it shows, maybe tinting
    /// it, and saying its [`Placing`]: millimetres and shade. `None` hides
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
                    let Placing { at, shade } = placed[i].unwrap_or_default();
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

#[cfg(test)]
mod tests {
    use eframe::egui::{Rect, Stroke, TextureId, pos2};

    use super::*;

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
            solids.add(layer, quad(texture), |v| {
                Some(Placing {
                    at: Vec3::new(v.pos.x, 0.0, 0.0),
                    shade: Shade::Unlit,
                })
            });
        }
        let [(_, opaque), (_, decals)] = solids.layers();
        let textures = |solids: &[Solid]| -> Vec<TextureId> {
            solids.iter().map(|s| s.mesh.texture_id).collect()
        };
        assert_eq!(textures(opaque), [TextureId::User(1), TextureId::User(2)]);
        let order = [1, 2, 1].map(TextureId::User);
        assert_eq!(textures(decals), order);
        // The joined mesh indexes its own vertices, each with its place.
        let joined = &opaque[0];
        assert_eq!(joined.mesh.indices, [0, 1, 2, 3, 4, 5]);
        assert_eq!(joined.places.len(), joined.mesh.vertices.len());
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
    fn a_shifted_solid_travels_whole_and_keeps_its_colour() {
        let mut mesh = Mesh::default();
        for (x, y) in [(0.0, 0.0), (10.0, 0.0), (0.0, 10.0)] {
            mesh.colored_vertex(pos2(x, y), Color32::WHITE);
        }
        mesh.add_triangle(0, 1, 2);
        let mut solids = Solids::default();
        solids.add(Layer::Opaque, mesh, |v| {
            Some(Placing {
                at: Vec3::new(v.pos.x, 0.0, v.pos.y),
                shade: Shade::Matte(Vec3::Z),
            })
        });
        let shifted = solids.shifted(Vec3::new(0.0, 12.7, 0.0));
        let [(_, opaque), _] = shifted.layers();
        assert_eq!(
            opaque[0].places,
            [
                Vec3::new(0.0, 12.7, 0.0),
                Vec3::new(10.0, 12.7, 0.0),
                Vec3::new(0.0, 12.7, 10.0)
            ]
        );
        // The shade vectors, and everything else in the mesh, ride along untouched.
        assert_eq!(opaque[0].shades, solids.layers()[0].1[0].shades);
        assert_eq!(
            opaque[0].mesh.vertices,
            solids.layers()[0].1[0].mesh.vertices
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
            (v.pos.x < 2.5).then(|| Placing {
                at: Vec3::new(v.pos.x / 10.0, 0.0, 0.0),
                shade: Shade::Unlit,
            })
        });
        let [(_, opaque), (_, decals)] = solids.layers();
        assert!(decals.is_empty());
        let solid = &opaque[0];
        assert_eq!(solid.mesh.indices, [0, 1, 2]);
        assert_eq!(
            solid.places,
            [
                Vec3::ZERO,
                Vec3::new(0.1, 0.0, 0.0),
                Vec3::new(0.2, 0.0, 0.0)
            ]
        );
        assert_eq!(solid.mesh.vertices[1].pos, pos2(1.0, 1.0));
    }
}
