//! The seated eye: the machine's inches on screen, and text laid flat on it.

use eframe::egui::epaint::{Vertex, WHITE_UV};
use eframe::egui::{Align2, Color32, FontId, Painter, Pos2, Rect, Shape, Stroke, pos2, vec2};

use super::EDGE;
use super::geometry::{dot, sub};
use crate::render::Metrics;
use crate::render::feed::convex_mesh;
use crate::render::perspective::warp;

/// The eye from the printing point, and how far it looks down.
const EYE_INCHES: f32 = 24.0;
const EYE_TILT_DEGREES: f32 = 35.0;
/// Text laid flat on the machine is set at this many points an inch, then
/// bent onto its surface.
pub(super) const FLAT_TEXT: f32 = 160.0;

/// Toward the seated eye from the machine, unit length.
pub(super) fn toward_eye() -> [f32; 3] {
    let (sin, cos) = EYE_TILT_DEGREES.to_radians().sin_cos();
    [0.0, cos, sin]
}

/// The seated eye, anchored at the printing point.
pub(super) struct Eye {
    pub(super) origin: Pos2,
    pub(super) ppi: f32,
    /// The tilt's sine and cosine.
    tilt: (f32, f32),
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
        }
    }

    /// Whether a face at `p` facing `normal` is turned toward the eye.
    pub(super) fn sees(p: [f32; 3], normal: [f32; 3]) -> bool {
        let eye = toward_eye().map(|c| EYE_INCHES * c);
        dot(sub(eye, p), normal) > 0.0
    }

    /// Screen points per inch at `p`: `ppi` at the printing point.
    pub(super) fn scale(&self, [_, y, z]: [f32; 3]) -> f32 {
        let (sin, cos) = self.tilt;
        let depth = (EYE_INCHES - y * cos - z * sin).max(1.0);
        self.ppi * EYE_INCHES / depth
    }

    /// Where `p` shows on screen.
    pub(super) fn at(&self, p: [f32; 3]) -> Pos2 {
        let [x, y, z] = p;
        let (sin, cos) = self.tilt;
        let up = z * cos - y * sin;
        self.origin + self.scale(p) * vec2(x, -up)
    }

    pub(super) fn polygon(&self, points: &[[f32; 3]]) -> Vec<Pos2> {
        points.iter().map(|&p| self.at(p)).collect()
    }

    /// Fills convex `points`, each vertex coloured by `colour`.
    pub(super) fn fill(
        &self,
        painter: &Painter,
        points: &[[f32; 3]],
        colour: impl Fn([f32; 3]) -> Color32,
    ) {
        let mut colours = points.iter().map(|&p| colour(p));
        let mesh = convex_mesh(&self.polygon(points), |pos| Vertex {
            pos,
            uv: WHITE_UV,
            color: colours.next().unwrap_or_default(),
        });
        painter.add(Shape::mesh(mesh));
    }

    /// A line through `points`, `width_inches` thick where it starts.
    pub(super) fn line(
        &self,
        painter: &Painter,
        points: &[[f32; 3]],
        width_inches: f32,
        colour: Color32,
    ) {
        let Some(&start) = points.first() else {
            return;
        };
        let width = width_inches * self.scale(start);
        painter.add(Shape::line(
            self.polygon(points),
            Stroke::new(width, colour),
        ));
    }

    pub(super) fn outline(&self, painter: &Painter, points: &[[f32; 3]]) {
        painter.add(Shape::closed_line(
            self.polygon(points),
            Stroke::new(1.0, EDGE),
        ));
    }
}

/// `text`, `size` inches tall, laid flat on a surface and seen in
/// perspective like print on it: `place` maps inches across and down from
/// the text's `anchor` to the machine.
pub(super) fn paint_flat_text(
    painter: &Painter,
    eye: &Eye,
    text: &str,
    size: f32,
    colour: Color32,
    anchor: Align2,
    place: impl Fn(f32, f32) -> [f32; 3],
) {
    let galley = painter.layout_no_wrap(
        text.to_owned(),
        FontId::proportional(size * FLAT_TEXT),
        colour,
    );
    let at = anchor.anchor_size(Pos2::ZERO, galley.size()).min;
    let shapes = vec![Shape::galley(at, galley, colour)];
    let mesh = warp(painter, shapes, |p| {
        eye.at(place(p.x / FLAT_TEXT, p.y / FLAT_TEXT))
    });
    painter.add(Shape::mesh(mesh));
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
