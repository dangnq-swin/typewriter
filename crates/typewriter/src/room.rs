//! The room around the desk, seen from the chair: a wall, and the desktop
//! running from under the window's bottom edge back to it. Fixed to the
//! window, like the plain app's paper.

use std::f32::consts::TAU;

use eframe::egui::{Color32, Mesh, Painter, Pos2, Rect, Shape, Stroke, pos2, vec2};

use typewriter_ui::draw::{Camera, splitmix64, unit};

/// Seated: looking a little down at the machine.
const TILT_DEGREES: f32 = 25.0;
/// Camera distance, in view heights.
const DISTANCE_VIEWS: f32 = 1.2;
/// The desk's far edge, down from the view's top, as a share of its height.
const FAR_EDGE_HEIGHT: f32 = 0.34;
/// Window's bottom edge to the wall.
const DESK_DEPTH_MM: f32 = 750.0;
/// Boards run left to right, grain along them.
const PLANK_MM: f32 = 140.0;
const STREAKS_PER_PLANK: u64 = 18;
const GRAIN_SEGMENTS: u16 = 32;
/// Wall to desk shading: how far it reaches on the desk.
const CONTACT_SHADE_MM: f32 = 40.0;
const SEED: u64 = 0x5EED_DE5C;

const WALL_TOP: Color32 = Color32::from_rgb(0xB4, 0xAB, 0x9C);
const WALL_LOW: Color32 = Color32::from_rgb(0xC8, 0xC0, 0xB0);
const WOOD_NEAR: Color32 = Color32::from_rgb(0x9C, 0x6C, 0x43);
const WOOD_FAR: Color32 = Color32::from_rgb(0x84, 0x59, 0x36);
const GRAIN_DARK: Color32 = Color32::from_rgb(0x5A, 0x38, 0x1E);
const GRAIN_LIGHT: Color32 = Color32::from_rgb(0xC0, 0x8E, 0x60);
const SEAM: Color32 = Color32::from_rgba_premultiplied(0x2A, 0x1A, 0x0E, 0x70);

/// The desk plane in view: the camera, how deep the desk runs, how wide.
struct Desk {
    camera: Camera,
    depth: f32,
    half_width: f32,
    /// Plane units per millimetre.
    mm: f32,
}

impl Desk {
    fn new(view: Rect) -> Self {
        let height = view.height().max(1.0);
        let distance = DISTANCE_VIEWS * height;
        // The depth whose far edge projects to FAR_EDGE_HEIGHT.
        let rise = (1.0 - FAR_EDGE_HEIGHT) * height;
        let (sin, cos) = TILT_DEGREES.to_radians().sin_cos();
        let depth = rise * distance / (distance * cos - rise * sin);
        Self {
            camera: Camera::tilted(pos2(view.center().x, view.bottom()), distance, TILT_DEGREES),
            depth,
            // At the far edge too, past the window's sides.
            half_width: view.width().max(1.0),
            mm: depth / DESK_DEPTH_MM,
        }
    }

    fn at(&self, x: f32, y: f32) -> Pos2 {
        self.camera.project(vec2(x, y), 0.0)
    }

    /// The strip of desk `near..far` deep, across its width.
    fn band(&self, near: f32, far: f32) -> [Pos2; 4] {
        let w = self.half_width;
        [
            self.at(-w, near),
            self.at(w, near),
            self.at(w, far),
            self.at(-w, far),
        ]
    }
}

pub fn paint(painter: &Painter, view: Rect) {
    let desk = Desk::new(view);
    let far_y = desk.at(0.0, desk.depth).y;
    let mut mesh = Mesh::default();
    let wall = Rect::from_min_max(view.min, pos2(view.max.x, far_y + 1.0));
    gradient(&mut mesh, corners(wall), WALL_LOW, WALL_TOP);
    gradient(&mut mesh, desk.band(0.0, desk.depth), WOOD_NEAR, WOOD_FAR);
    grain(&mut mesh, &desk);
    // Where desk meets wall, a little light is kept out.
    let shade = desk.depth - CONTACT_SHADE_MM * desk.mm;
    gradient(
        &mut mesh,
        desk.band(shade, desk.depth),
        Color32::TRANSPARENT,
        Color32::from_black_alpha(60),
    );
    let above = Rect::from_min_max(
        pos2(view.min.x, far_y - 0.02 * view.height()),
        pos2(view.max.x, far_y),
    );
    gradient(
        &mut mesh,
        corners(above),
        Color32::from_black_alpha(35),
        Color32::TRANSPARENT,
    );
    painter.add(Shape::mesh(mesh));
    let planks = (DESK_DEPTH_MM / PLANK_MM).ceil() as u16;
    for plank in 1..planks {
        let y = f32::from(plank) * PLANK_MM * desk.mm;
        let [left, right, ..] = desk.band(y, y);
        painter.line_segment([left, right], Stroke::new(1.0, SEAM));
    }
}

/// Streaks along each board, fading to a hairline with distance rather than
/// thinning below a pixel: sub-pixel strips shimmer.
fn grain(mesh: &mut Mesh, desk: &Desk) {
    let planks = (DESK_DEPTH_MM / PLANK_MM).ceil() as u64;
    for plank in 0..planks {
        let plank_near = plank as f32 * PLANK_MM;
        // Each board a shade apart.
        let tone = splitmix64(SEED ^ plank);
        let (near, far) = (
            plank_near * desk.mm,
            (plank_near + PLANK_MM).min(DESK_DEPTH_MM) * desk.mm,
        );
        let tint = if unit(tone, 0) < 0.5 {
            GRAIN_DARK
        } else {
            GRAIN_LIGHT
        };
        gradient(
            mesh,
            desk.band(near, far),
            with_alpha(tint, 10 + (unit(tone, 16) * 20.0) as u8),
            with_alpha(tint, 10 + (unit(tone, 32) * 20.0) as u8),
        );
        for streak in 0..STREAKS_PER_PLANK {
            let bits = splitmix64(SEED ^ (plank << 16) ^ streak);
            let y = plank_near + unit(bits, 0) * PLANK_MM;
            if y >= DESK_DEPTH_MM {
                continue;
            }
            let colour = if unit(bits, 48) < 0.7 {
                GRAIN_DARK
            } else {
                GRAIN_LIGHT
            };
            let alpha = 25.0 + unit(bits, 16) * 50.0;
            streak_strip(
                mesh,
                desk,
                Streak {
                    y: y * desk.mm,
                    thickness: (0.5 + unit(bits, 32) * 2.5) * desk.mm,
                    amplitude: (1.0 + unit(bits, 8) * 4.0) * desk.mm,
                    wavelength: (300.0 + unit(bits, 24) * 600.0) * desk.mm,
                    phase: unit(bits, 40) * TAU,
                    colour,
                    alpha,
                },
            );
        }
    }
}

struct Streak {
    y: f32,
    thickness: f32,
    amplitude: f32,
    wavelength: f32,
    phase: f32,
    colour: Color32,
    alpha: f32,
}

fn streak_strip(mesh: &mut Mesh, desk: &Desk, streak: Streak) {
    let half = streak.thickness / 2.0;
    let pixels = desk.at(0.0, streak.y - half).y - desk.at(0.0, streak.y + half).y;
    // Under a pixel: a pixel wide, as faint as it is thin.
    let (half, alpha) = if pixels < 1.0 {
        (half / pixels.max(0.05), streak.alpha * pixels.max(0.05))
    } else {
        (half, streak.alpha)
    };
    let colour = with_alpha(streak.colour, alpha as u8);
    let first = mesh.vertices.len() as u32;
    for i in 0..=GRAIN_SEGMENTS {
        let x = desk.half_width * (2.0 * f32::from(i) / f32::from(GRAIN_SEGMENTS) - 1.0);
        let y = streak.y + streak.amplitude * (x / streak.wavelength * TAU + streak.phase).sin();
        mesh.colored_vertex(desk.at(x, y - half), colour);
        mesh.colored_vertex(desk.at(x, y + half), colour);
    }
    for i in 0..u32::from(GRAIN_SEGMENTS) {
        let a = first + 2 * i;
        mesh.add_triangle(a, a + 1, a + 2);
        mesh.add_triangle(a + 1, a + 3, a + 2);
    }
}

fn with_alpha(colour: Color32, alpha: u8) -> Color32 {
    Color32::from_rgba_unmultiplied(colour.r(), colour.g(), colour.b(), alpha)
}

fn corners(rect: Rect) -> [Pos2; 4] {
    [
        rect.left_bottom(),
        rect.right_bottom(),
        rect.right_top(),
        rect.left_top(),
    ]
}

/// A quad given near (or lower) edge first (`[near-left, near-right, far-right,
/// far-left]`), shaded from `near` to `far`.
fn gradient(mesh: &mut Mesh, quad: [Pos2; 4], near: Color32, far: Color32) {
    let first = mesh.vertices.len() as u32;
    for (p, colour) in quad.into_iter().zip([near, near, far, far]) {
        mesh.colored_vertex(p, colour);
    }
    mesh.add_triangle(first, first + 1, first + 2);
    mesh.add_triangle(first, first + 2, first + 3);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_desk_runs_from_the_window_bottom_to_its_far_edge() {
        let view = Rect::from_min_size(pos2(0.0, 0.0), vec2(1600.0, 1000.0));
        let desk = Desk::new(view);
        assert!((desk.at(0.0, 0.0).y - view.bottom()).abs() < 0.5);
        let far = desk.at(0.0, desk.depth).y;
        assert!((far - FAR_EDGE_HEIGHT * view.height()).abs() < 0.5, "{far}");
        let [.., far_right, far_left] = desk.band(0.0, desk.depth);
        assert!(far_left.x < view.left() && far_right.x > view.right());
    }
}
