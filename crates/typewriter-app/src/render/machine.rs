//! The desk edition's typewriter, as a silhouette: the platen behind the
//! sheet, travelling with it, and the body in front of it below the typing
//! line, still.

use std::f32::consts::{FRAC_PI_2, PI};

use eframe::egui::epaint::{Vertex, WHITE_UV};
use eframe::egui::{Color32, Mesh, Painter, Pos2, Rect, Shape, Stroke, pos2};

use super::Metrics;
use super::feed::convex_mesh;

const PLATEN_DIAMETER_INCHES: f32 = 1.3;
/// Past each paper edge: under the knobs' shafts and collars.
const PLATEN_OVERHANG_INCHES: f32 = 0.3;
/// The body's top, across, in paper widths. The SM9 is about 32 cm wide.
const BODY_PAPERS: f32 = 1.5;
/// The body widens toward the keyboard: this much out per point down.
const BODY_FLARE: f32 = 0.12;
const BODY_CORNER_INCHES: f32 = 0.35;
const CORNER_STEPS: u16 = 6;

const RUBBER: Color32 = Color32::from_rgb(0x1C, 0x1B, 0x1A);
const RUBBER_SHINE: Color32 = Color32::from_rgb(0x3A, 0x39, 0x37);
const BODY_TOP: Color32 = Color32::from_rgb(0x4A, 0x50, 0x4C);
const BODY_LOW: Color32 = Color32::from_rgb(0x2A, 0x2E, 0x2B);
const BODY_RIM: Color32 = Color32::from_rgb(0x70, 0x78, 0x72);

/// Behind the sheet: a rubber roller across it, centred on the typing line.
pub fn paint_platen(painter: &Painter, metrics: &Metrics, paper_left: f32, typing_y: f32) {
    let ppi = metrics.points_per_inch;
    let overhang = PLATEN_OVERHANG_INCHES * ppi;
    let radius = PLATEN_DIAMETER_INCHES * ppi / 2.0;
    let (left, right) = (
        paper_left - overhang,
        paper_left + metrics.paper_size.x + overhang,
    );
    // Rounded: dark at top and bottom, a shine just above the middle.
    let rows = [
        (typing_y - radius, RUBBER),
        (typing_y - 0.25 * radius, RUBBER_SHINE),
        (typing_y + radius, RUBBER),
    ];
    let mut mesh = Mesh::default();
    for pair in rows.windows(2) {
        let [(top, top_colour), (bottom, bottom_colour)] = [pair[0], pair[1]];
        let first = mesh.vertices.len() as u32;
        mesh.colored_vertex(pos2(left, top), top_colour);
        mesh.colored_vertex(pos2(right, top), top_colour);
        mesh.colored_vertex(pos2(right, bottom), bottom_colour);
        mesh.colored_vertex(pos2(left, bottom), bottom_colour);
        mesh.add_triangle(first, first + 1, first + 2);
        mesh.add_triangle(first, first + 2, first + 3);
    }
    painter.add(Shape::mesh(mesh));
}

/// In front of the sheet: from `top` down past the window, centred in it.
/// Hides the paper wound round the platen.
pub fn paint_body(painter: &Painter, view: Rect, metrics: &Metrics, top: f32) {
    let outline = body_outline(view, metrics, top);
    let bottom = view.bottom().max(top + 1.0);
    let mesh = convex_mesh(&outline, |pos| {
        let t = ((pos.y - top) / (bottom - top)).clamp(0.0, 1.0);
        Vertex {
            pos,
            uv: WHITE_UV,
            color: BODY_TOP.lerp_to_gamma(BODY_LOW, t),
        }
    });
    painter.add(Shape::mesh(mesh));
    // The rim along the top and round its corners.
    let rim = 2 * usize::from(CORNER_STEPS) + 2;
    painter.add(Shape::line(
        outline[..rim].to_vec(),
        Stroke::new(1.5, BODY_RIM),
    ));
}

/// Clockwise from the left side's top: both rounded corners, then the flared
/// bottom. Each corner turns only as far as the side's slope: convex.
fn body_outline(view: Rect, metrics: &Metrics, top: f32) -> Vec<Pos2> {
    let half = BODY_PAPERS * metrics.paper_size.x / 2.0;
    let radius = BODY_CORNER_INCHES * metrics.points_per_inch;
    let centre = view.center().x;
    let bottom = view.bottom().max(top + radius) + 1.0;
    let slope = BODY_FLARE.atan();
    let arc = |x: f32, from: f32| {
        let span = FRAC_PI_2 - slope;
        (0..=CORNER_STEPS).map(move |i| {
            let angle = from + span * f32::from(i) / f32::from(CORNER_STEPS);
            pos2(
                x + radius * angle.cos(),
                top + radius + radius * angle.sin(),
            )
        })
    };
    let mut outline: Vec<Pos2> = arc(centre - half + radius, PI + slope)
        .chain(arc(centre + half - radius, 3.0 * FRAC_PI_2))
        .collect();
    let (left, right) = (outline[0], outline[outline.len() - 1]);
    outline.push(pos2(right.x + BODY_FLARE * (bottom - right.y), bottom));
    outline.push(pos2(left.x - BODY_FLARE * (bottom - left.y), bottom));
    outline
}

#[cfg(test)]
mod tests {
    use super::*;
    use eframe::egui::vec2;

    #[test]
    fn the_body_is_convex_and_reaches_the_window_bottom() {
        let profile = typewriter_core::Profile::from_toml_str(include_str!(
            "../../../../profiles/olympia-sm9.toml"
        ))
        .unwrap();
        let metrics = Metrics::new(&profile, 96.0);
        let view = Rect::from_min_size(pos2(0.0, 0.0), vec2(1600.0, 1000.0));
        let outline = body_outline(view, &metrics, 700.0);
        assert!(outline.iter().all(|p| p.y >= 700.0 - 0.01));
        assert!(outline.iter().any(|p| p.y > view.bottom()));
        let n = outline.len();
        let turns: Vec<f32> = (0..n)
            .map(|i| {
                let (a, b, c) = (outline[i], outline[(i + 1) % n], outline[(i + 2) % n]);
                (b - a).x * (c - b).y - (b - a).y * (c - b).x
            })
            .collect();
        assert!(turns.iter().all(|&t| t >= -1e-2), "{turns:?}");
    }
}
