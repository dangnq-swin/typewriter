//! The seated eye: the machine's millimetres on screen, how deep they are,
//! the [`Camera`] it hands the shader — built from the same maths, so the
//! flat parts cannot drift from the GPU's — and text laid flat on it.

use eframe::egui::epaint::{Vertex, WHITE_UV};
use eframe::egui::{Align2, Color32, FontId, Mesh, Pos2, Rect, Shape, Stroke, Vec2, pos2, vec2};

use super::EDGE;
use super::canvas::Canvas;
use super::geometry::{add, convex_grid, dot, lerp3, sub};
use super::light::Paint;
use crate::depth::{Camera, Layer, NEAR_MM, Placing, Shade, Solid};
use typewriter_ui::draw::{Metrics, convex_mesh};

/// The eye from the printing point, and how far it looks down.
const EYE_MM: f32 = 609.6;
/// A flat part bent onto a curved face is cut into pieces this long.
const BENT_MM: f32 = 1.27;
pub(super) const EYE_TILT_DEGREES: f32 = 35.0;
/// Text laid flat on the machine is laid out at this many points a
/// millimetre (160 an inch), then bent onto its surface.
pub(super) const FLAT_TEXT: f32 = 160.0 / 25.4;

/// Toward the seated eye from the machine, unit length.
pub(super) fn toward_eye() -> [f32; 3] {
    let (sin, cos) = EYE_TILT_DEGREES.to_radians().sin_cos();
    [0.0, cos, sin]
}

/// How high `p` shows above the printing point, in millimetres at its scale:
/// what stands above what on screen, whatever the view.
#[cfg(test)]
pub(super) fn screen_up(p: [f32; 3]) -> f32 {
    -Eye::at_origin(Pos2::ZERO, 1.0, vec2(1.0, 1.0)).at(p).y
}

/// The seated eye, anchored at the printing point.
#[derive(Clone, Copy)]
pub(super) struct Eye {
    pub(super) origin: Pos2,
    /// Screen points per machine millimetre at the printing point.
    pub(super) ppmm: f32,
    /// The window it projects into, in points: `camera`'s frame.
    view: Vec2,
    /// The tilt's sine and cosine.
    tilt: (f32, f32),
    /// Where points are measured from, from the printing point.
    anchor: [f32; 3],
}

impl Eye {
    pub(super) fn new(view: Rect, metrics: &Metrics, typing_y: f32) -> Self {
        Self::at_origin(
            pos2(view.center().x, typing_y),
            metrics.points_per_mm(),
            view.size(),
        )
    }

    fn at_origin(origin: Pos2, ppmm: f32, view: Vec2) -> Self {
        Self {
            origin,
            ppmm,
            view,
            tilt: EYE_TILT_DEGREES.to_radians().sin_cos(),
            anchor: [0.0; 3],
        }
    }

    /// The same eye, taking points from `anchor` rather than the printing
    /// point. A solid's mm→machine transform lives in the absolute
    /// millimetres its vertices carry, this offset folded in at build time:
    /// a moving part will want a per-draw model matrix instead, and nothing
    /// here is baked that cannot move.
    pub(super) fn about(self, anchor: [f32; 3]) -> Self {
        Self { anchor, ..self }
    }

    /// `p`, measured from the eye's anchor, in absolute machine
    /// millimetres.
    pub(super) fn absolute(&self, p: [f32; 3]) -> [f32; 3] {
        add(p, self.anchor)
    }

    /// The absolute millimetres showing at screen `at`, beside `p` and
    /// across the view from it: for marks and specks drawn in screen
    /// points, whose vertices know no millimetres of their own.
    pub(super) fn mm_under(&self, p: [f32; 3], at: Pos2) -> [f32; 3] {
        let off = at - self.at(p);
        let scale = self.scale(p);
        let [_, cos, sin] = toward_eye();
        // Across the view is [1, 0, 0]; down it, with the view tipped up
        // this far, [0, sin, -cos].
        add(
            self.absolute(p),
            [off.x / scale, off.y * sin / scale, -off.y * cos / scale],
        )
    }

    /// Whether a face at `p` from the anchor, facing `normal`, is turned
    /// toward the eye.
    pub(super) fn faces(&self, p: [f32; 3], normal: [f32; 3]) -> bool {
        Self::sees(add(p, self.anchor), normal)
    }

    /// Whether a face at `p` facing `normal` is turned toward the eye.
    pub(super) fn sees(p: [f32; 3], normal: [f32; 3]) -> bool {
        let eye = toward_eye().map(|c| EYE_MM * c);
        dot(sub(eye, p), normal) > 0.0
    }

    /// Millimetres from the eye to `p`, along its line of sight.
    pub(super) fn distance(&self, p: [f32; 3]) -> f32 {
        let [_, y, z] = add(p, self.anchor);
        let (sin, cos) = self.tilt;
        (EYE_MM - y * cos - z * sin).max(NEAR_MM)
    }

    /// `p`'s depth: 0 at the eye to 1 far off, as the pass's `Camera` rows
    /// give it. Only tests need it on this side: what shows where, and how
    /// steeply a decal's face lies.
    #[cfg(test)]
    pub(super) fn depth(&self, p: [f32; 3]) -> f32 {
        depth_at(self.distance(p))
    }

    /// Screen points per millimetre at `p`: `ppmm` at the printing point.
    pub(super) fn scale(&self, p: [f32; 3]) -> f32 {
        self.ppmm * EYE_MM / self.distance(p)
    }

    /// Where `p` shows on screen.
    pub(super) fn at(&self, p: [f32; 3]) -> Pos2 {
        let [x, y, z] = add(p, self.anchor);
        let (sin, cos) = self.tilt;
        let up = z * cos - y * sin;
        let scale = self.scale(p);
        self.origin + vec2(x * scale, -up * scale)
    }

    /// The depth pass's camera: the clip rows that fold the same `at` and
    /// `depth` maths into a matrix — the shader projects absolute machine
    /// millimetres with it, and dividing by `w` lands the window's NDC
    /// (x right, y up) over `view`. A test keeps them from drifting.
    pub(super) fn camera(&self) -> Camera {
        let (sin, cos) = self.tilt;
        let f = self.ppmm * EYE_MM;
        let (width, height) = (self.view.x.max(1.0), self.view.y.max(1.0));
        let (ox, oy) = (self.origin.x, self.origin.y);
        // The `w` row: the distance from the eye before the near clamp.
        let w = [0.0, -cos, -sin, EYE_MM];
        let row = |mut parts: [f32; 4], along: f32| {
            for (part, edge) in parts.iter_mut().zip(w) {
                *part += along * edge;
            }
            parts
        };
        Camera {
            points: [width, height].into(),
            rows: [
                row([2.0 * f / width, 0.0, 0.0, 0.0], 2.0 * ox / width - 1.0),
                row(
                    [0.0, -2.0 * f * sin / height, 2.0 * f * cos / height, 0.0],
                    1.0 - 2.0 * oy / height,
                ),
                [0.0, -cos, -sin, EYE_MM - NEAR_MM],
                w,
            ],
        }
    }

    pub(super) fn polygon(&self, points: &[[f32; 3]]) -> Vec<Pos2> {
        points.iter().map(|&p| self.at(p)).collect()
    }

    /// Fills convex `points`, each vertex painted by `colour`: a face, or
    /// see-through lying on what is behind.
    pub(super) fn fill<P: Into<Paint>>(
        &self,
        canvas: &Canvas,
        points: &[[f32; 3]],
        colour: impl Fn([f32; 3]) -> P,
    ) {
        self.fill_on(canvas, points, colour, false);
    }

    /// As [`Eye::fill`], lying on a face: a patch on it.
    pub(super) fn fill_lying<P: Into<Paint>>(
        &self,
        canvas: &Canvas,
        points: &[[f32; 3]],
        colour: impl Fn([f32; 3]) -> P,
    ) {
        self.fill_on(canvas, points, colour, true);
    }

    fn fill_on<P: Into<Paint>>(
        &self,
        canvas: &Canvas,
        points: &[[f32; 3]],
        colour: impl Fn([f32; 3]) -> P,
        lying: bool,
    ) {
        let paints: Vec<Paint> = points.iter().map(|&p| colour(p).into()).collect();
        let mut colours = paints.iter().map(|paint| paint.colour);
        let mesh = convex_mesh(&self.polygon(points), |pos| Vertex {
            pos,
            uv: WHITE_UV,
            color: colours.next().unwrap_or_default(),
        });
        let shades = paints.iter().map(|paint| paint.shade).collect();
        self.add_mesh(canvas, mesh, points, shades, lying);
    }

    /// Fills convex `outline`, flat millimetres, bent onto the machine by
    /// `place` and cut fine enough to follow it; each vertex painted by
    /// `colour`.
    pub(super) fn fill_bent<P: Into<Paint>>(
        &self,
        canvas: &Canvas,
        outline: &[Pos2],
        place: impl Fn(Pos2) -> [f32; 3],
        colour: impl Fn([f32; 3]) -> P,
    ) {
        let mut mesh = convex_grid(outline, BENT_MM);
        let on: Vec<_> = mesh.vertices.iter().map(|v| place(v.pos)).collect();
        let mut shades = Vec::with_capacity(on.len());
        for (vertex, &p) in mesh.vertices.iter_mut().zip(&on) {
            let paint = colour(p).into();
            vertex.pos = self.at(p);
            vertex.color = paint.colour;
            shades.push(paint.shade);
        }
        self.add_mesh(canvas, mesh, &on, shades, false);
    }

    /// `mesh` on screen, its vertices at `points` taking the light as
    /// `shades`: a face, or see-through or `lying` on what is behind.
    fn add_mesh(
        &self,
        canvas: &Canvas,
        mesh: Mesh,
        points: &[[f32; 3]],
        shades: Vec<Shade>,
        lying: bool,
    ) {
        let see_through = mesh.vertices.iter().any(|v| v.color.a() < u8::MAX);
        let layer = if lying || see_through {
            Layer::Decal
        } else {
            Layer::Opaque
        };
        let places = points.iter().map(|&p| self.absolute(p)).collect();
        canvas.mesh(
            layer,
            Solid {
                mesh,
                places,
                shades,
            },
        );
    }

    /// Adds a quad of `corners`, in order round it, each in its paint, to
    /// `solid`: what lies on a face goes in the same solid, and the decal
    /// pass lifts it.
    pub(super) fn quad<P: Into<Paint>>(&self, solid: &mut Solid, corners: [([f32; 3], P); 4]) {
        let first = solid.mesh.vertices.len() as u32;
        for (p, paint) in corners {
            let paint = paint.into();
            solid.mesh.colored_vertex(self.at(p), paint.colour);
            solid.places.push(self.absolute(p));
            solid.shades.push(paint.shade);
        }
        solid.mesh.add_triangle(first, first + 1, first + 2);
        solid.mesh.add_triangle(first, first + 2, first + 3);
    }

    /// A line through `points`, `width_mm` thick where it starts, lying
    /// on what it runs along: `paint` its colour and its take of the light.
    pub(super) fn line<P: Into<Paint>>(
        &self,
        canvas: &Canvas,
        points: &[[f32; 3]],
        width_mm: f32,
        paint: P,
    ) {
        let paint = paint.into();
        let Some(&start) = points.first() else {
            return;
        };
        let width = width_mm * self.scale(start);
        let shape = Shape::line(self.polygon(points), Stroke::new(width, paint.colour));
        self.stroke_laid(canvas, points, shape, paint.shade);
    }

    pub(super) fn outline(&self, canvas: &Canvas, points: &[[f32; 3]]) {
        self.outline_in(canvas, points, EDGE);
    }

    /// A point-wide line round `points`, lying on them.
    pub(super) fn outline_in(&self, canvas: &Canvas, points: &[[f32; 3]], colour: Color32) {
        let mut closed = points.to_vec();
        closed.extend(points.first());
        let shape = Shape::closed_line(self.polygon(points), Stroke::new(1.0, colour));
        self.stroke(canvas, &closed, shape);
    }

    /// `shape`, drawn on screen along `points`, lying on them: each of its
    /// vertices as deep as the nearest place along them.
    pub(super) fn stroke(&self, canvas: &Canvas, points: &[[f32; 3]], shape: Shape) {
        self.stroke_laid(canvas, points, shape, Shade::Unlit);
    }

    /// `shape` along `points` as [`Eye::stroke`], its vertices taking
    /// `shade` from the millimetres they stand at.
    fn stroke_laid(&self, canvas: &Canvas, points: &[[f32; 3]], shape: Shape, shade: Shade) {
        let on: Vec<(Pos2, [f32; 3])> = points.iter().map(|&p| (self.at(p), p)).collect();
        let eye = *self;
        canvas.lay(vec![shape], move |vertex| {
            // The millimetres under the vertex itself, not the path's:
            // a stroke's quads stand aside from their centre, and the GPU
            // draws where the millimetres are — the centre's would
            // collapse the line to zero width.
            let mm = nearest_place(&on, vertex.pos);
            Some(Placing {
                at: eye.mm_under(mm, vertex.pos),
                shade,
            })
        });
    }
}

/// The depth of what is `distance` millimetres from the eye.
#[cfg(test)]
fn depth_at(distance: f32) -> f32 {
    1.0 - NEAR_MM / distance.max(NEAR_MM)
}

/// The millimetres along `path` nearest `at`, points on screen with the
/// millimetres they stand at, from the eye's anchor: those, between the
/// two ends of the nearest stretch.
fn nearest_place(path: &[(Pos2, [f32; 3])], at: Pos2) -> [f32; 3] {
    let mut best = (f32::INFINITY, path.first().map_or([0.0; 3], |&(_, mm)| mm));
    for pair in path.windows(2) {
        let [(a, ma), (b, mb)] = [pair[0], pair[1]];
        let along = b - a;
        let t = ((at - a).dot(along) / along.length_sq().max(1e-6)).clamp(0.0, 1.0);
        let off = (a + along * t - at).length_sq();
        if off < best.0 {
            best = (off, lerp3(ma, mb, t));
        }
    }
    best.1
}

/// `text`, `size` millimetres tall, laid flat on a surface and seen in
/// perspective like print on it: `place` maps millimetres across and down
/// from the text's `anchor` to the machine.
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
    canvas.lay(shapes, |vertex| {
        let on = place(vertex.pos.x / FLAT_TEXT, vertex.pos.y / FLAT_TEXT);
        vertex.pos = eye.at(on);
        Some(Placing {
            at: eye.absolute(on),
            shade: Shade::Unlit,
        })
    });
}

#[cfg(test)]
impl Eye {
    /// Its printing point at `(800, typing_y)` in a 1600 by 1000 point
    /// window, `ppi` screen points an inch there: the sheet's convention,
    /// kept for the tests' readability.
    pub(super) fn testing(typing_y: f32, ppi: f32) -> Self {
        Self::at_origin(pos2(800.0, typing_y), ppi / 25.4, vec2(1600.0, 1000.0))
    }

    /// The millimetres toward the eye the depth pass lifts a decal lying at
    /// `p`, whose triangles run at `slope` depth a screen point: what a face
    /// must stay clear of among the decals on it. Keep in step with
    /// `depth::decal_bias`.
    pub(super) fn lift_mm(&self, p: [f32; 3], slope: f32) -> f32 {
        let d = self.distance(p);
        d - NEAR_MM / (NEAR_MM / d + crate::depth::decal_bias(slope))
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
        assert!((eye.scale([0.0, 0.0, 0.0]) - 96.0 / 25.4).abs() < 2.5e-3);
        // Nearer the writer: lower on screen, and larger.
        let key = [0.0, KEY_ROW.0, KEY_ROW.1];
        assert!(eye.at(key).y > eye.at(on_cover(0.0, COVER_FRONT.0)).y);
        assert!(eye.scale(key) > eye.scale([0.0; 3]));
    }

    #[test]
    fn the_eye_sees_faces_turned_toward_it_only() {
        let front = [0.0, CASE_FRONT, SHELF_Z];
        assert!(Eye::sees(front, [0.0, 1.0, 0.0]));
        assert!(!Eye::sees(front, [0.0, -1.0, 0.0]));
        // A left-hand side face, turned away from the middle.
        let side = [-PANEL_HALF_BOTTOM, CASE_FRONT - 2.54, SHELF_Z];
        assert!(!Eye::sees(side, [-1.0, 0.0, 0.0]));
        assert!(Eye::sees(side, [1.0, 0.0, 0.0]));
    }

    #[test]
    fn the_camera_projects_where_the_eye_looks() {
        let eye = Eye::testing(500.0, 96.0);
        let camera = eye.camera();
        let project =
            |row: [f32; 4], [x, y, z]: [f32; 3]| row[0] * x + row[1] * y + row[2] * z + row[3];
        // The printing point, off-centre, near, far, and on real parts.
        let samples = [
            [0.0, 0.0, 0.0],
            [0.0, 20.0, 40.0],
            [12.7, KEY_ROW.0, KEY_ROW.1],
            [-60.0, CASE_FRONT, SHELF_Z],
            [-220.0, 90.0, -110.0],
            [175.0, -140.0, 260.0],
            [300.0, -600.0, 200.0],
        ];
        let [width, height] = eye.view.into();
        for &p in &samples {
            let (x, y, z, w) = (
                project(camera.rows[0], p),
                project(camera.rows[1], p),
                project(camera.rows[2], p),
                project(camera.rows[3], p),
            );
            assert!(w >= NEAR_MM, "{p:?} behind the near plane");
            let at = eye.at(p);
            assert!(
                ((x / w + 1.0) * width / 2.0 - at.x).abs() < 1e-3,
                "{p:?} across"
            );
            assert!(
                ((1.0 - y / w) * height / 2.0 - at.y).abs() < 1e-3,
                "{p:?} down"
            );
            assert!((z / w - eye.depth(p)).abs() < 1e-6, "{p:?} deep");
        }
    }

    #[test]
    fn an_anchored_eye_shows_the_same_camera() {
        // Solids stand in absolute millimetres: the rows do not move with
        // the anchor, only the points going through them.
        let anchor = [-7.5, -20.0, 3.0];
        let eye = Eye::testing(500.0, 96.0).about(anchor);
        assert_eq!(eye.camera().rows, Eye::testing(500.0, 96.0).camera().rows);
        let rows = eye.camera().rows;
        let project =
            |row: [f32; 4], [x, y, z]: [f32; 3]| row[0] * x + row[1] * y + row[2] * z + row[3];
        let [width, height] = eye.view.into();
        for p in [[0.0; 3], [40.0, 30.0, -60.0], [-150.0, 120.0, 90.0]] {
            let absolute = eye.absolute(p);
            let w = project(rows[3], absolute);
            let at = eye.at(p);
            assert!(
                ((project(rows[0], absolute) / w + 1.0) * width / 2.0 - at.x).abs() < 1e-3,
                "{p:?} across"
            );
            assert!(
                ((1.0 - project(rows[1], absolute) / w) * height / 2.0 - at.y).abs() < 1e-3,
                "{p:?} down"
            );
            assert!(
                (project(rows[2], absolute) / w - eye.depth(p)).abs() < 1e-6,
                "{p:?} deep"
            );
        }
    }
}
