//! Points in inches, rounded outlines, and meshes built from them.

use eframe::egui::{Color32, Mesh, Pos2, Vec2, vec2};

pub(super) fn dot(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

pub(super) fn add(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

pub(super) fn sub(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

pub(super) fn normalized(v: [f32; 3]) -> [f32; 3] {
    let length = dot(v, v).sqrt().max(1e-6);
    [v[0] / length, v[1] / length, v[2] / length]
}

pub(super) fn lerp3(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
    [0, 1, 2].map(|k| a[k] + (b[k] - a[k]) * t)
}

/// Corners `(point, rounding)` of a flat convex polygon, each rounded by a
/// curve tangent to both sides.
pub(super) fn rounded(corners: &[([f32; 3], f32)]) -> Vec<[f32; 3]> {
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
) -> Vec<[f32; 3]> {
    rounded(&[
        ([left, back, z], rounding),
        ([right, back, z], rounding),
        ([right, front, z], rounding),
        ([left, front, z], rounding),
    ])
}

/// `corner` rounded between the sides to `before` and `after`: a curve
/// tangent to both, from the side toward `before`. The corner alone if
/// `rounding` is zero.
pub(super) fn fillet(
    before: [f32; 3],
    corner: [f32; 3],
    after: [f32; 3],
    rounding: f32,
) -> Vec<[f32; 3]> {
    const STEPS: u16 = 6;
    if rounding <= 0.0 {
        return vec![corner];
    }
    let toward = |other: [f32; 3]| {
        let d = sub(other, corner);
        // At most halfway: neighbouring roundings never cross.
        let t = (rounding / dot(d, d).sqrt().max(1e-6)).min(0.5);
        add(corner, [d[0] * t, d[1] * t, d[2] * t])
    };
    let (from, to) = (toward(before), toward(after));
    (0..=STEPS)
        .map(|step| {
            let t = f32::from(step) / f32::from(STEPS);
            lerp3(lerp3(from, corner, t), lerp3(corner, to, t), t)
        })
        .collect()
}

/// A quad of `corners` in order round it, each in its colour.
pub(super) fn add_quad(mesh: &mut Mesh, corners: [(Pos2, Color32); 4]) {
    let first = mesh.vertices.len() as u32;
    for (pos, colour) in corners {
        mesh.colored_vertex(pos, colour);
    }
    mesh.add_triangle(first, first + 1, first + 2);
    mesh.add_triangle(first, first + 2, first + 3);
}

/// A strip fading from `colour` along `edge` to nothing along `inside`.
pub(super) fn add_fade(
    mesh: &mut Mesh,
    [a, b]: [Pos2; 2],
    [a_in, b_in]: [Pos2; 2],
    colour: Color32,
) {
    let clear = Color32::TRANSPARENT;
    add_quad(
        mesh,
        [(a, colour), (b, colour), (b_in, clear), (a_in, clear)],
    );
}

/// The convex hull of `points` on screen.
pub(super) fn hull(mut points: Vec<Pos2>) -> Vec<Pos2> {
    points.sort_by(|a, b| a.x.total_cmp(&b.x).then(a.y.total_cmp(&b.y)));
    points.dedup();
    if points.len() < 3 {
        return points;
    }
    let turns_left =
        |o: Pos2, a: Pos2, b: Pos2| (a - o).x * (b - o).y - (a - o).y * (b - o).x > 0.0;
    let chain = |points: &mut dyn Iterator<Item = Pos2>| {
        let mut half: Vec<Pos2> = Vec::new();
        for p in points {
            while half.len() >= 2 && !turns_left(half[half.len() - 2], half[half.len() - 1], p) {
                half.pop();
            }
            half.push(p);
        }
        half.pop();
        half
    };
    let mut outline = chain(&mut points.iter().copied());
    outline.extend(chain(&mut points.iter().rev().copied()));
    outline
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
    fn a_hull_keeps_only_the_outline() {
        let square = [
            (0.0, 0.0),
            (2.0, 0.0),
            (2.0, 2.0),
            (0.0, 2.0),
            (1.0, 1.0),
            (1.0, 0.0),
        ];
        let outline = hull(square.iter().map(|&(x, y)| pos2(x, y)).collect());
        assert_eq!(outline.len(), 4);
        assert!(!outline.contains(&pos2(1.0, 1.0)));
    }
}
