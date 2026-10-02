//! The sheets on their way through the machine, drawn in depth. Measured
//! along the paper from the printing point: up the front, it bends off the
//! platen to lean back as the paper support does; down, it goes round under
//! the platen and up the support behind, its printed side away from the
//! writer. Across, a point keeps its millimetres from the printing point.

use eframe::egui::epaint::Vertex;
use eframe::egui::{Mesh, Painter, Pos2, Rect};
use glam::Vec3;
use typewriter_ui::draw::{FlatSheet, Metrics};

use super::carriage::{PLATEN_DIAMETER_MM, STRIKE_DEGREES, platen_axis};
use super::eye::Eye;
use super::support;
use crate::depth::{self, Layer, Placing, Shade, Solids};

/// Leaving the printing point, the sheet bends from the platen's slope to
/// the support's lean round this radius.
const BEND_MM: f32 = 25.0;
/// Between a sheet's mesh rows: enough to bend round the platen. Its
/// columns: few enough skewing its texture.
const WAY_STEP_MM: f32 = 1.27;
const WAY_COLUMNS: u16 = 8;

/// The point `along` the way, `(y, z)`, and its printed side's normal.
fn way(along: f32) -> ([f32; 2], [f32; 2]) {
    let strike = STRIKE_DEGREES.to_radians();
    let lean = support::LEAN_DEGREES.to_radians();
    if along >= 0.0 {
        // Its lean back from upright, from the platen's slope to the support's.
        let bend = BEND_MM * (strike - lean);
        let tilt = strike - along.min(bend) / BEND_MM;
        let y = BEND_MM * (strike.cos() - tilt.cos());
        let z = BEND_MM * (strike.sin() - tilt.sin());
        let straight = (along - bend).max(0.0);
        let (sin, cos) = tilt.sin_cos();
        return ([y - straight * sin, z + straight * cos], [cos, sin]);
    }
    let wrap = support::wrap_mm();
    if along >= -wrap {
        let radius = PLATEN_DIAMETER_MM / 2.0;
        let axis = platen_axis();
        let (sin, cos) = (strike + along / radius).sin_cos();
        return ([axis.y + radius * cos, axis.z + radius * sin], [cos, sin]);
    }
    let [ny, nz] = support::facing();
    (support::way(-along - wrap), [-ny, -nz])
}

/// How far forward the sheet's printed face is at height `z` before the
/// platen, near the printing point: where the alignment guide presses.
/// Below the printing point it lies on the platen; above, round the bend,
/// then straight on, leaning back as [`way`] runs.
pub(super) fn face_y(z: f32) -> f32 {
    let strike = STRIKE_DEGREES.to_radians();
    if z < 0.0 {
        let radius = PLATEN_DIAMETER_MM / 2.0;
        let axis = platen_axis();
        let up = (z - axis.z).clamp(-radius, radius);
        return axis.y + (radius * radius - up * up).sqrt();
    }
    let lean = support::LEAN_DEGREES.to_radians();
    let bent = BEND_MM * (strike.sin() - lean.sin());
    let sin_tilt = strike.sin() - z.min(bent) / BEND_MM;
    let y = BEND_MM * (strike.cos() - (1.0 - sin_tilt * sin_tilt).sqrt());
    let straight = (z - bent).max(0.0) / lean.cos();
    y - straight * lean.sin()
}

/// The top of the sheet's front edge on screen, `along` millimetres up it.
pub(super) fn front_at(eye: &Eye, along: f32) -> f32 {
    let ([y, z], _) = way(along.max(0.0));
    eye.at(Vec3::new(0.0, y, z)).y
}

/// A point of a sheet on its way.
struct Placed {
    pos: Pos2,
    /// Where it stands, machine millimetres: the pass projects these, and
    /// the lamp lights it from there.
    at: Vec3,
    /// Its seen side's normal: paper as a matte material, its take of the
    /// light the shader's, not a tint.
    normal: Vec3,
    /// Its printed side is turned toward the eye.
    facing: bool,
}

/// `sheets`, laid flat as the typing view would, on their way through the
/// machine for the typing line at `typing_y`: the paper and its print matte
/// faces the lamp lights per fragment, the print on the side turned toward
/// the eye.
pub fn paint_sheets(
    painter: &Painter,
    view: Rect,
    metrics: &Metrics,
    typing_y: f32,
    sheets: Vec<FlatSheet>,
) {
    let eye = Eye::new(view, metrics, typing_y);
    let per_mm = metrics.points_per_mm();
    let place = |at: Pos2| {
        let ([y, z], [ny, nz]) = way((typing_y - at.y) / per_mm);
        let p = Vec3::new((at.x - eye.origin.x) / per_mm, y, z);
        let normal = Vec3::new(0.0, ny, nz);
        let facing = Eye::sees(p, normal);
        let seen = if facing { normal } else { -normal };
        Placed {
            pos: eye.at(p),
            at: p,
            normal: seen,
            facing,
        }
    };
    // Safe cast: a sheet's length in 1.27 mm steps.
    let rows = (metrics.paper_size.y / per_mm / WAY_STEP_MM)
        .ceil()
        .max(1.0) as u16;
    let mut solids = Solids::default();
    for sheet in sheets {
        let paper = |vertex: &mut Vertex| {
            let placed = place(vertex.pos);
            vertex.pos = placed.pos;
            Some(Placing {
                at: placed.at,
                shade: Shade::Matte(placed.normal),
            })
        };
        solids.add(Layer::Opaque, grid(&sheet.paper, rows), paper);
        // Long marks (the frame's sides) bend with the sheet. Matte as the
        // paper it lies on: correction chalk then matches it, lit and all.
        let longest = WAY_STEP_MM * 2.0 * per_mm;
        let print = |vertex: &mut Vertex| {
            let placed = place(vertex.pos);
            vertex.pos = placed.pos;
            placed.facing.then_some(Placing {
                at: placed.at,
                shade: Shade::Matte(placed.normal),
            })
        };
        solids.add_shapes(painter, Layer::Decal, sheet.print, longest, print);
    }
    depth::gather(painter.ctx(), solids);
}

/// The `paper` quad, corners clockwise from the top left, cut into `rows`
/// and [`WAY_COLUMNS`] to bend. Empty if it is no quad.
fn grid(paper: &Mesh, rows: u16) -> Mesh {
    let mut mesh = Mesh::with_texture(paper.texture_id);
    let &[top_left, top_right, bottom_right, bottom_left] = paper.vertices.as_slice() else {
        return mesh;
    };
    let lerp = |a: Vertex, b: Vertex, t: f32| Vertex {
        pos: a.pos.lerp(b.pos, t),
        uv: a.uv.lerp(b.uv, t),
        color: a.color.lerp_to_gamma(b.color, t),
    };
    let width = u32::from(WAY_COLUMNS) + 1;
    for row in 0..=rows {
        let down = f32::from(row) / f32::from(rows);
        let (left, right) = (
            lerp(top_left, bottom_left, down),
            lerp(top_right, bottom_right, down),
        );
        for column in 0..=WAY_COLUMNS {
            let across = f32::from(column) / f32::from(WAY_COLUMNS);
            mesh.vertices.push(lerp(left, right, across));
        }
        if row > 0 {
            for column in 0..u32::from(WAY_COLUMNS) {
                let below = u32::from(row) * width + column;
                let above = below - width;
                mesh.add_triangle(above, above + 1, below + 1);
                mesh.add_triangle(above, below + 1, below);
            }
        }
    }
    mesh
}

#[cfg(test)]
mod tests {
    use super::super::eye::toward_eye;
    use super::*;
    use eframe::egui::Color32;

    #[test]
    fn the_way_runs_on_without_a_break() {
        let step = 0.025;
        let mut along = -200.0;
        while along < 200.0 {
            let ((a, _), (b, _)) = (way(along), way(along + step));
            let gap = (a[0] - b[0]).hypot(a[1] - b[1]);
            assert!((gap - step).abs() < 0.0025, "{along}: {gap}");
            along += step;
        }
    }

    #[test]
    fn the_face_lies_on_the_way_before_the_platen() {
        // From near the platen's bottom, up past the bend.
        let mut along = -33.0;
        while along < 51.0 {
            let ([y, z], _) = way(along);
            assert!((face_y(z) - y).abs() < 0.0025, "{along}: {} {y}", face_y(z));
            along += 0.25;
        }
    }

    #[test]
    fn a_sheet_s_grid_spans_its_quad() {
        let mut quad = Mesh::default();
        for (x, y) in [(10.0, 20.0), (110.0, 20.0), (110.0, 220.0), (10.0, 220.0)] {
            quad.colored_vertex(eframe::egui::pos2(x, y), Color32::WHITE);
        }
        let mesh = grid(&quad, 4);
        let width = usize::from(WAY_COLUMNS) + 1;
        assert_eq!(mesh.vertices.len(), 5 * width);
        assert_eq!(mesh.indices.len(), 4 * usize::from(WAY_COLUMNS) * 6);
        let last = mesh.vertices[mesh.vertices.len() - 1].pos;
        assert_eq!(
            (mesh.vertices[0].pos, last),
            (quad.vertices[0].pos, quad.vertices[2].pos)
        );
        assert_eq!(mesh.vertices[width].pos.y, 70.0);
    }

    #[test]
    fn the_printing_point_faces_the_eye_and_the_back_turns_away() {
        let ([y, z], [ny, nz]) = way(0.0);
        assert!(y.abs() < 2.5e-5 && z.abs() < 2.5e-5);
        let eye = toward_eye();
        assert!((ny - eye.y).abs() < 1e-5 && (nz - eye.z).abs() < 1e-5);
        let behind = -support::wrap_mm() - 51.0;
        let (_, normal) = way(behind);
        assert!(normal[0] < 0.0, "{normal:?}");
    }
}
