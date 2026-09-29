//! A sheet scrunched up into a ball and tossed off the desk.

use eframe::egui::{Color32, Mesh, Painter, Pos2, Shape, Stroke, Vec2, vec2};

use super::paper::splitmix64;

/// As long as the crumpling sound.
pub const SECONDS: f64 = 0.9;
/// The share of the time spent crumpling; the rest is the toss.
const CRUMPLE: f32 = 0.65;
const RAYS: usize = 48;
const CREASES: usize = 9;

const SHEET: Color32 = Color32::from_rgb(0xF7, 0xF4, 0xEC);
const EDGE: Color32 = Color32::from_rgb(0xA8, 0xA0, 0x92);
const CREASE: Color32 = Color32::from_rgba_premultiplied(0x50, 0x4A, 0x40, 0x50);
const SHADOW: Color32 = Color32::from_rgba_premultiplied(0, 0, 0, 0x30);

/// Draws the sheet whose outline was `quad`, `t` seconds into being
/// scrunched up. `seed` varies the folds from one sheet to the next.
pub fn paint(painter: &Painter, quad: [Pos2; 4], t: f64, seed: u64) {
    let t = (t / SECONDS).clamp(0.0, 1.0) as f32;
    let crumple = smoothstep((t / CRUMPLE).min(1.0));
    let toss = ((t - CRUMPLE) / (1.0 - CRUMPLE)).max(0.0);

    let centre = quad.iter().fold(Vec2::ZERO, |sum, p| sum + p.to_vec2()) / 4.0;
    let centre = centre.to_pos2();
    let span = quad
        .iter()
        .map(|p| (*p - centre).length())
        .fold(0.0_f32, f32::max);
    let ball = 0.16 * span;
    // Up and away to the lower left, off the desk.
    let flight = vec2(-2.2 * span * toss, span * (-0.9 * toss + 3.2 * toss * toss));
    let centre = centre + flight;

    let outline: Vec<Pos2> = (0..RAYS)
        .map(|k| {
            let angle = std::f32::consts::TAU * k as f32 / RAYS as f32;
            let direction = Vec2::angled(angle);
            let flat = distance_to_edge(&quad, centre - flight, direction);
            let bits = splitmix64(seed ^ k as u64);
            let noise = |shift: u32| ((bits >> shift) & 0xFFFF) as f32 / 65535.0;
            let lumpy = ball * (0.75 + 0.5 * noise(0));
            // Folds bite in and out while it is being crushed.
            let wrinkle = (std::f32::consts::PI * crumple).sin() * 0.18 * flat * (noise(16) - 0.5);
            let radius = flat + (lumpy - flat) * crumple + wrinkle;
            centre + direction * radius.max(1.0)
        })
        .collect();

    let shadow_offset = vec2(3.0, 5.0) * (1.0 - toss);
    fan(
        painter,
        centre + shadow_offset,
        &outline,
        shadow_offset,
        SHADOW,
    );
    fan(painter, centre, &outline, Vec2::ZERO, SHEET);
    painter.add(Shape::closed_line(outline.clone(), Stroke::new(1.0, EDGE)));
    // Creases show as it crumples.
    let crease = Stroke::new(1.0, CREASE.gamma_multiply(crumple));
    for c in 0..CREASES {
        let bits = splitmix64(seed.rotate_left(17) ^ c as u64);
        let a = (bits as usize) % RAYS;
        let b = (a + RAYS / 4 + ((bits >> 20) as usize) % (RAYS / 2)) % RAYS;
        let middle = centre + ((outline[a] - centre) + (outline[b] - centre)) * 0.2;
        painter.add(Shape::line(vec![outline[a], middle, outline[b]], crease));
    }
}

/// How far from `from` the quad's edge is along `direction`. `from` is
/// inside the quad.
fn distance_to_edge(quad: &[Pos2; 4], from: Pos2, direction: Vec2) -> f32 {
    (0..4)
        .filter_map(|i| {
            let (a, b) = (quad[i], quad[(i + 1) % 4]);
            let edge = b - a;
            let denominator = direction.x * edge.y - direction.y * edge.x;
            if denominator.abs() < 1e-6 {
                return None;
            }
            let to_a = a - from;
            let along_ray = (to_a.x * edge.y - to_a.y * edge.x) / denominator;
            let along_edge = (to_a.x * direction.y - to_a.y * direction.x) / denominator;
            ((0.0..=1.0).contains(&along_edge) && along_ray > 0.0).then_some(along_ray)
        })
        .fold(f32::INFINITY, f32::min)
        .min(1e4)
}

/// A filled outline that is star-shaped around `centre`, as a triangle fan.
fn fan(painter: &Painter, centre: Pos2, outline: &[Pos2], offset: Vec2, color: Color32) {
    let mut mesh = Mesh::default();
    mesh.colored_vertex(centre, color);
    for p in outline {
        mesh.colored_vertex(*p + offset, color);
    }
    let n = outline.len() as u32;
    for i in 0..n {
        mesh.add_triangle(0, 1 + i, 1 + (i + 1) % n);
    }
    painter.add(Shape::mesh(mesh));
}

fn smoothstep(t: f32) -> f32 {
    t * t * (3.0 - 2.0 * t)
}

#[cfg(test)]
mod tests {
    use super::*;
    use eframe::egui::pos2;

    #[test]
    fn the_edge_of_a_square_is_found_along_any_ray() {
        let quad = [
            pos2(-10.0, -10.0),
            pos2(10.0, -10.0),
            pos2(10.0, 10.0),
            pos2(-10.0, 10.0),
        ];
        let d = |x, y| distance_to_edge(&quad, Pos2::ZERO, vec2(x, y));
        assert!((d(1.0, 0.0) - 10.0).abs() < 1e-4);
        assert!((d(0.0, -1.0) - 10.0).abs() < 1e-4);
        let diagonal = Vec2::angled(std::f32::consts::FRAC_PI_4);
        assert!((d(diagonal.x, diagonal.y) - 10.0 * 2.0_f32.sqrt()).abs() < 1e-3);
    }
}
