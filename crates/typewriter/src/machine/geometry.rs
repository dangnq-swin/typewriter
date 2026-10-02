//! Points in millimetres, rounded outlines, and meshes built from them.

use std::collections::HashMap;

use eframe::egui::epaint::{Vertex, WHITE_UV};
use eframe::egui::{Color32, Mesh, Pos2, Rect, Vec2, vec2};
use glam::Vec3;

/// Corners `(point, rounding)` of a flat convex polygon, each rounded by a
/// curve tangent to both sides.
pub(super) fn rounded(corners: &[(Vec3, f32)]) -> Vec<Vec3> {
    let n = corners.len();
    let mut outline = Vec::new();
    for (i, &(corner, rounding)) in corners.iter().enumerate() {
        let (before, after) = (corners[(i + n - 1) % n].0, corners[(i + 1) % n].0);
        outline.extend(fillet(before, corner, after, rounding));
    }
    outline
}

/// A flat rectangle over `x` and `y` ranges at height `z`, its corners
/// rounded by `rounding`.
pub(super) fn rounded_rect(
    [left, right]: [f32; 2],
    [back, front]: [f32; 2],
    z: f32,
    rounding: f32,
) -> Vec<Vec3> {
    rounded(&[
        (Vec3::new(left, back, z), rounding),
        (Vec3::new(right, back, z), rounding),
        (Vec3::new(right, front, z), rounding),
        (Vec3::new(left, front, z), rounding),
    ])
}

/// `corner` rounded between the sides to `before` and `after`: a curve
/// tangent to both, from the side toward `before`. The corner alone if
/// `rounding` is zero.
pub(super) fn fillet(before: Vec3, corner: Vec3, after: Vec3, rounding: f32) -> Vec<Vec3> {
    const STEPS: u16 = 6;
    if rounding <= 0.0 {
        return vec![corner];
    }
    let toward = |other: Vec3| {
        let d = other - corner;
        // At most halfway: neighbouring roundings never cross.
        let t = (rounding / d.length().max(1e-6)).min(0.5);
        corner + d * t
    };
    let (from, to) = (toward(before), toward(after));
    (0..=STEPS)
        .map(|step| {
            let t = f32::from(step) / f32::from(STEPS);
            from.lerp(corner, t).lerp(corner.lerp(to, t), t)
        })
        .collect()
}

/// Convex `outline` filled white, cut on a grid `step` apart: squares inside,
/// squares clipped to it along its edge. Bends evenly, unlike a fan's
/// thin triangles cut smaller.
pub(super) fn convex_grid(outline: &[Pos2], step: f32) -> Mesh {
    let mut mesh = Mesh::default();
    if outline.len() < 3 {
        return mesh;
    }
    let n = outline.len();
    let edges: Vec<(Pos2, Pos2)> = (0..n).map(|i| (outline[i], outline[(i + 1) % n])).collect();
    let turn = |a: Pos2, b: Pos2, p: Pos2| (b - a).x * (p - a).y - (b - a).y * (p - a).x;
    // Which side of each edge is inside, whichever way round it runs.
    let winding = edges
        .iter()
        .map(|&(a, b)| turn(Pos2::ZERO, a, b))
        .sum::<f32>()
        .signum();
    let bounds = Rect::from_points(outline);
    // Safe casts: a part's size in steps.
    let columns = (bounds.width() / step).ceil().max(1.0) as i32;
    let rows = (bounds.height() / step).ceil().max(1.0) as i32;
    let corner = |i: i32, j: i32| bounds.min + vec2(i as f32, j as f32) * step;
    // Grid corners are shared by the cells round them; clipped points not.
    let mut shared: HashMap<(i32, i32), u32> = HashMap::new();
    for j in 0..rows {
        for i in 0..columns {
            let mut cell: Vec<(Pos2, Option<(i32, i32)>)> =
                [(i, j), (i + 1, j), (i + 1, j + 1), (i, j + 1)]
                    .map(|(i, j)| (corner(i, j), Some((i, j))))
                    .to_vec();
            // Sutherland–Hodgman: clipped to inside each edge in turn.
            for &(a, b) in &edges {
                let inside = |p: Pos2| winding * turn(a, b, p);
                let mut kept = Vec::with_capacity(cell.len() + 1);
                for k in 0..cell.len() {
                    let (p, q) = (cell[k], cell[(k + 1) % cell.len()]);
                    let (dp, dq) = (inside(p.0), inside(q.0));
                    if (dp >= 0.0) != (dq >= 0.0) {
                        kept.push((p.0.lerp(q.0, dp / (dp - dq)), None));
                    }
                    if dq >= 0.0 {
                        kept.push(q);
                    }
                }
                cell = kept;
                if cell.is_empty() {
                    break;
                }
            }
            if cell.len() < 3 {
                continue;
            }
            let mut index = |(pos, key): (Pos2, Option<(i32, i32)>)| {
                let add = |mesh: &mut Mesh| {
                    mesh.vertices.push(Vertex {
                        pos,
                        uv: WHITE_UV,
                        color: Color32::WHITE,
                    });
                    // Safe cast: a part's vertices fit egui's u32 indices.
                    mesh.vertices.len() as u32 - 1
                };
                match key {
                    Some(key) => *shared.entry(key).or_insert_with(|| add(&mut mesh)),
                    None => add(&mut mesh),
                }
            };
            let indices: Vec<u32> = cell.into_iter().map(&mut index).collect();
            for k in 1..indices.len() - 1 {
                mesh.add_triangle(indices[0], indices[k], indices[k + 1]);
            }
        }
    }
    mesh
}

/// Convex `inner` in `colour`, fading to nothing `blur` points outside it.
pub(super) fn soft(inner: &[Pos2], blur: f32, colour: Color32) -> Mesh {
    let n = inner.len() as u32;
    let centre = inner.iter().fold(Vec2::ZERO, |sum, p| sum + p.to_vec2()) / n as f32;
    let outward = |a: Pos2, b: Pos2| {
        let along = (b - a).normalized();
        let normal = vec2(along.y, -along.x);
        if (a.to_vec2() - centre).dot(normal) < 0.0 {
            -normal
        } else {
            normal
        }
    };
    let mut mesh = Mesh::default();
    for &p in inner {
        mesh.colored_vertex(p, colour);
    }
    for i in 1..n.saturating_sub(1) {
        mesh.add_triangle(0, i, i + 1);
    }
    let len = inner.len();
    for (i, &p) in inner.iter().enumerate() {
        let (before, after) = (inner[(i + len - 1) % len], inner[(i + 1) % len]);
        let normal = (outward(before, p) + outward(p, after)).normalized();
        mesh.colored_vertex(p + normal * blur, Color32::TRANSPARENT);
    }
    for i in 0..n {
        let j = (i + 1) % n;
        mesh.add_triangle(i, j, n + j);
        mesh.add_triangle(i, n + j, n + i);
    }
    mesh
}

#[cfg(test)]
mod tests {
    use super::*;
    use eframe::egui::pos2;

    #[test]
    fn a_grid_covers_its_outline_in_small_cells() {
        // A rounded plate, wide and low, as the alignment guide's.
        let corners = [(0.0, 0.0), (3.0, 0.0), (3.0, 0.8), (0.0, 0.8)]
            .map(|(x, y)| (Vec3::new(x, y, 0.0), 0.2));
        let outline: Vec<Pos2> = rounded(&corners)
            .into_iter()
            .map(|p| pos2(p.x, p.y))
            .collect();
        let step = 0.05;
        let mesh = convex_grid(&outline, step);
        let area: f32 = mesh
            .indices
            .as_chunks::<3>()
            .0
            .iter()
            .map(|t| {
                let [a, b, c] = t.map(|i| mesh.vertices[i as usize].pos);
                ((b - a).x * (c - a).y - (b - a).y * (c - a).x).abs() / 2.0
            })
            .sum();
        let corner_cut = 4.0 * (0.2 * 0.2 - std::f32::consts::FRAC_PI_4 * 0.2 * 0.2);
        assert!((area - (2.4 - corner_cut)).abs() < 0.02, "{area}");
        let diagonal = step * std::f32::consts::SQRT_2 + 1e-4;
        for t in mesh.indices.as_chunks::<3>().0 {
            let [a, b, c] = t.map(|i| mesh.vertices[i as usize].pos);
            for (p, q) in [(a, b), (b, c), (c, a)] {
                assert!(p.distance(q) <= diagonal, "{p:?} {q:?}");
            }
        }
        // About a vertex a cell, not thousands.
        assert!(mesh.vertices.len() < 1300, "{}", mesh.vertices.len());
    }
}
