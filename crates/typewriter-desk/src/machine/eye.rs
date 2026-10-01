//! The seated eye: the machine's inches on screen, how deep they are, and
//! text laid flat on it.

use eframe::egui::epaint::{Vertex, WHITE_UV};
use eframe::egui::{Align2, Color32, FontId, Mesh, Pos2, Rect, Shape, Stroke, pos2, vec2};

use super::EDGE;
use super::canvas::Canvas;
use super::geometry::{add, convex_grid, dot, sub};
use crate::depth::{Layer, Solid};
use typewriter_app::draw::{Metrics, convex_mesh};

/// The eye from the printing point, and how far it looks down.
const EYE_INCHES: f32 = 24.0;
/// Depth runs from this near the eye, out to far off.
const NEAR_INCHES: f32 = 1.0;
/// What lies on a face (edges, marks, print) is set this much nearer, to
/// show over it.
pub(super) const LYING_INCHES: f32 = 0.01;
/// A flat part bent onto a curved face is cut into pieces this long.
const BENT_INCHES: f32 = 0.05;
pub(super) const EYE_TILT_DEGREES: f32 = 35.0;
/// Text laid flat on the machine is set at this many points an inch, then
/// bent onto its surface.
pub(super) const FLAT_TEXT: f32 = 160.0;

/// Toward the seated eye from the machine, unit length.
pub(super) fn toward_eye() -> [f32; 3] {
    let (sin, cos) = EYE_TILT_DEGREES.to_radians().sin_cos();
    [0.0, cos, sin]
}

/// The seated eye, anchored at the printing point.
#[derive(Clone, Copy)]
pub(super) struct Eye {
    pub(super) origin: Pos2,
    pub(super) ppi: f32,
    /// The tilt's sine and cosine.
    tilt: (f32, f32),
    /// Where points are measured from, from the printing point.
    anchor: [f32; 3],
}

impl Eye {
    pub(super) fn new(view: Rect, metrics: &Metrics, typing_y: f32) -> Self {
        Self::at_origin(pos2(view.center().x, typing_y), metrics.points_per_inch)
    }

    fn at_origin(origin: Pos2, ppi: f32) -> Self {
        Self {
            origin,
            ppi,
            tilt: EYE_TILT_DEGREES.to_radians().sin_cos(),
            anchor: [0.0; 3],
        }
    }

    /// The same eye, taking points from `anchor` rather than the printing
    /// point.
    pub(super) fn about(self, anchor: [f32; 3]) -> Self {
        Self { anchor, ..self }
    }

    /// Whether a face at `p` from the anchor, facing `normal`, is turned
    /// toward the eye.
    pub(super) fn faces(&self, p: [f32; 3], normal: [f32; 3]) -> bool {
        Self::sees(add(p, self.anchor), normal)
    }

    /// Whether a face at `p` facing `normal` is turned toward the eye.
    pub(super) fn sees(p: [f32; 3], normal: [f32; 3]) -> bool {
        let eye = toward_eye().map(|c| EYE_INCHES * c);
        dot(sub(eye, p), normal) > 0.0
    }

    /// Inches from the eye to `p`, along its line of sight.
    pub(super) fn distance(&self, p: [f32; 3]) -> f32 {
        let [_, y, z] = add(p, self.anchor);
        let (sin, cos) = self.tilt;
        (EYE_INCHES - y * cos - z * sin).max(NEAR_INCHES)
    }

    /// `p`'s depth: 0 at the eye to 1 far off.
    pub(super) fn depth(&self, p: [f32; 3]) -> f32 {
        depth_at(self.distance(p))
    }

    /// The depth of what lies on a face at `p`: a hair nearer.
    pub(super) fn lying_depth(&self, p: [f32; 3]) -> f32 {
        depth_at(self.distance(p) - LYING_INCHES)
    }

    /// Screen points per inch at `p`: `ppi` at the printing point.
    pub(super) fn scale(&self, p: [f32; 3]) -> f32 {
        self.ppi * EYE_INCHES / self.distance(p)
    }

    /// Where `p` shows on screen.
    pub(super) fn at(&self, p: [f32; 3]) -> Pos2 {
        let [x, y, z] = add(p, self.anchor);
        let (sin, cos) = self.tilt;
        let up = z * cos - y * sin;
        let scale = self.scale(p);
        self.origin + vec2(x * scale, -up * scale)
    }

    pub(super) fn polygon(&self, points: &[[f32; 3]]) -> Vec<Pos2> {
        points.iter().map(|&p| self.at(p)).collect()
    }

    /// Fills convex `points`, each vertex coloured by `colour`: a face, or
    /// see-through lying on what is behind.
    pub(super) fn fill(
        &self,
        canvas: &Canvas,
        points: &[[f32; 3]],
        colour: impl Fn([f32; 3]) -> Color32,
    ) {
        self.fill_on(canvas, points, colour, false);
    }

    /// As [`Eye::fill`], lying on a face: a patch on it.
    pub(super) fn fill_lying(
        &self,
        canvas: &Canvas,
        points: &[[f32; 3]],
        colour: impl Fn([f32; 3]) -> Color32,
    ) {
        self.fill_on(canvas, points, colour, true);
    }

    fn fill_on(
        &self,
        canvas: &Canvas,
        points: &[[f32; 3]],
        colour: impl Fn([f32; 3]) -> Color32,
        lying: bool,
    ) {
        let mut colours = points.iter().map(|&p| colour(p));
        let mesh = convex_mesh(&self.polygon(points), |pos| Vertex {
            pos,
            uv: WHITE_UV,
            color: colours.next().unwrap_or_default(),
        });
        self.add_mesh(canvas, mesh, points, lying);
    }

    /// Fills convex `outline`, flat inches, bent onto the machine by `place`
    /// and cut fine enough to follow it; each vertex coloured by `colour`.
    pub(super) fn fill_bent(
        &self,
        canvas: &Canvas,
        outline: &[Pos2],
        place: impl Fn(Pos2) -> [f32; 3],
        colour: impl Fn([f32; 3]) -> Color32,
    ) {
        let mut mesh = convex_grid(outline, BENT_INCHES);
        let on: Vec<_> = mesh.vertices.iter().map(|v| place(v.pos)).collect();
        for (vertex, &p) in mesh.vertices.iter_mut().zip(&on) {
            vertex.pos = self.at(p);
            vertex.color = colour(p);
        }
        self.add_mesh(canvas, mesh, &on, false);
    }

    /// `mesh` on screen, its vertices at `points`: a face, or see-through or
    /// `lying` on what is behind.
    fn add_mesh(&self, canvas: &Canvas, mesh: Mesh, points: &[[f32; 3]], lying: bool) {
        let see_through = mesh.vertices.iter().any(|v| v.color.a() < u8::MAX);
        let (layer, depth): (_, fn(&Self, [f32; 3]) -> f32) = if lying || see_through {
            (Layer::Decal, Self::lying_depth)
        } else {
            (Layer::Opaque, Self::depth)
        };
        let depths = points.iter().map(|&p| depth(self, p)).collect();
        canvas.mesh(layer, Solid { mesh, depths });
    }

    /// Adds a quad of `corners`, in order round it, each in its colour, to
    /// `solid`.
    pub(super) fn quad(&self, solid: &mut Solid, corners: [([f32; 3], Color32); 4]) {
        self.add_quad(solid, corners, Self::depth);
    }

    /// As [`Eye::quad`], lying on a face.
    pub(super) fn lying_quad(&self, solid: &mut Solid, corners: [([f32; 3], Color32); 4]) {
        self.add_quad(solid, corners, Self::lying_depth);
    }

    fn add_quad(
        &self,
        solid: &mut Solid,
        corners: [([f32; 3], Color32); 4],
        depth: fn(&Self, [f32; 3]) -> f32,
    ) {
        let first = solid.mesh.vertices.len() as u32;
        for (p, colour) in corners {
            solid.mesh.colored_vertex(self.at(p), colour);
            solid.depths.push(depth(self, p));
        }
        solid.mesh.add_triangle(first, first + 1, first + 2);
        solid.mesh.add_triangle(first, first + 2, first + 3);
    }

    /// A line through `points`, `width_inches` thick where it starts, lying
    /// on what it runs along.
    pub(super) fn line(
        &self,
        canvas: &Canvas,
        points: &[[f32; 3]],
        width_inches: f32,
        colour: Color32,
    ) {
        let Some(&start) = points.first() else {
            return;
        };
        let width = width_inches * self.scale(start);
        self.stroke(
            canvas,
            points,
            Shape::line(self.polygon(points), Stroke::new(width, colour)),
        );
    }

    pub(super) fn outline(&self, canvas: &Canvas, points: &[[f32; 3]]) {
        let mut closed = points.to_vec();
        closed.extend(points.first());
        let shape = Shape::closed_line(self.polygon(points), Stroke::new(1.0, EDGE));
        self.stroke(canvas, &closed, shape);
    }

    /// `shape`, drawn on screen along `points`, lying on them: each of its
    /// vertices as deep as the nearest place along them.
    pub(super) fn stroke(&self, canvas: &Canvas, points: &[[f32; 3]], shape: Shape) {
        let path: Vec<(Pos2, f32)> = points
            .iter()
            .map(|&p| (self.at(p), self.lying_depth(p)))
            .collect();
        canvas.lay(vec![shape], |at| (at, nearest_depth(&path, at)));
    }
}

/// The depth of what is `distance` inches from the eye.
fn depth_at(distance: f32) -> f32 {
    1.0 - NEAR_INCHES / distance.max(NEAR_INCHES)
}

/// The depth along `path`, points on screen and their depths, nearest `at`.
fn nearest_depth(path: &[(Pos2, f32)], at: Pos2) -> f32 {
    let mut best = (f32::INFINITY, path.first().map_or(1.0, |&(_, d)| d));
    for pair in path.windows(2) {
        let [(a, da), (b, db)] = [pair[0], pair[1]];
        let along = b - a;
        let t = ((at - a).dot(along) / along.length_sq().max(1e-6)).clamp(0.0, 1.0);
        let off = (a + along * t - at).length_sq();
        if off < best.0 {
            best = (off, da + (db - da) * t);
        }
    }
    best.1
}

/// `text`, `size` inches tall, laid flat on a surface and seen in
/// perspective like print on it: `place` maps inches across and down from
/// the text's `anchor` to the machine.
pub(super) fn paint_flat_text(
    canvas: &Canvas,
    eye: &Eye,
    text: &str,
    size: f32,
    colour: Color32,
    anchor: Align2,
    place: impl Fn(f32, f32) -> [f32; 3],
) {
    let galley = canvas.painter().layout_no_wrap(
        text.to_owned(),
        FontId::proportional(size * FLAT_TEXT),
        colour,
    );
    let at = anchor.anchor_size(Pos2::ZERO, galley.size()).min;
    let shapes = vec![Shape::galley(at, galley, colour)];
    canvas.lay(shapes, |p| {
        let on = place(p.x / FLAT_TEXT, p.y / FLAT_TEXT);
        (eye.at(on), eye.lying_depth(on))
    });
}

#[cfg(test)]
impl Eye {
    /// Its printing point at `(800, typing_y)`.
    pub(super) fn testing(typing_y: f32, ppi: f32) -> Self {
        Self::at_origin(pos2(800.0, typing_y), ppi)
    }
}

#[cfg(test)]
mod tests {
    use super::super::case::{CASE_FRONT, SHELF_Z};
    use super::super::cover::{COVER_FRONT, on_cover};
    use super::super::keyboard::KEY_ROW;
    use super::super::panel::PANEL_HALF_BOTTOM;
    use super::*;

    #[test]
    fn the_printing_point_is_at_the_sheets_scale() {
        let eye = Eye::testing(500.0, 96.0);
        assert_eq!(eye.at([0.0, 0.0, 0.0]), eye.origin);
        assert!((eye.scale([0.0, 0.0, 0.0]) - 96.0).abs() < 1e-3);
        // Nearer the writer: lower on screen, and larger.
        let key = [0.0, KEY_ROW.0, KEY_ROW.1];
        assert!(eye.at(key).y > eye.at(on_cover(0.0, COVER_FRONT.0)).y);
        assert!(eye.scale(key) > 96.0);
    }

    #[test]
    fn the_eye_sees_faces_turned_toward_it_only() {
        let front = [0.0, CASE_FRONT, SHELF_Z];
        assert!(Eye::sees(front, [0.0, 1.0, 0.0]));
        assert!(!Eye::sees(front, [0.0, -1.0, 0.0]));
        // A left-hand side face, turned away from the middle.
        let side = [-PANEL_HALF_BOTTOM, CASE_FRONT - 0.1, SHELF_Z];
        assert!(!Eye::sees(side, [-1.0, 0.0, 0.0]));
        assert!(Eye::sees(side, [1.0, 0.0, 0.0]));
    }
}
