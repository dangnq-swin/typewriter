//! The room around the desk, seen from the chair: a wall, and the desktop
//! running from under the window's bottom edge back to it. Painted through
//! the machine's own eye, in the machine's millimetres: the wall stands at
//! [`WALL_Y`], and wall and desk move with the zoom like the rest of the
//! scene. Only the wall's flat fill stays a screen shape — it has no depth
//! worth projecting.

use std::f32::consts::TAU;

use eframe::egui::{Color32, Mesh, Painter, Pos2, Rect, Shape, Stroke, lerp, pos2};
use glam::Vec3;
use typewriter_ui::draw::{Metrics, splitmix64, unit};

use crate::machine::{self, DESK_Z, Eye};

/// Where the desk meets the wall, in the machine's millimetres: behind the
/// machine, out of the reach of everything it and the writer do.
pub(crate) const WALL_Y: f32 = -330.0;

/// Boards run left to right, grain along them, counted from the wall.
const PLANK_MM: f32 = 140.0;
const STREAKS_PER_PLANK: u64 = 18;
const GRAIN_SEGMENTS: u16 = 32;
/// Wall to desk shading: how far it reaches on the desk.
const CONTACT_SHADE_MM: f32 = 40.0;
const SEED: u64 = 0x5EED_DE5C;
/// Screen bands a world gradient is cut into: the eye bends millimetres
/// into screen points, so a single quad would not shade evenly.
const SHADES: usize = 32;

const WALL_TOP: Color32 = Color32::from_rgb(0xB4, 0xAB, 0x9C);
const WALL_LOW: Color32 = Color32::from_rgb(0xC8, 0xC0, 0xB0);
const WOOD_NEAR: Color32 = Color32::from_rgb(0x9C, 0x6C, 0x43);
const WOOD_FAR: Color32 = Color32::from_rgb(0x84, 0x59, 0x36);
const GRAIN_DARK: Color32 = Color32::from_rgb(0x5A, 0x38, 0x1E);
const GRAIN_LIGHT: Color32 = Color32::from_rgb(0xC0, 0x8E, 0x60);
const SEAM: Color32 = Color32::from_rgba_premultiplied(0x2A, 0x1A, 0x0E, 0x70);

pub fn paint(painter: &Painter, view: Rect, metrics: &Metrics, zoom_percent: u16) {
    let typing_y = view.height() * machine::typing_line_height(view, metrics, zoom_percent);
    let eye = Eye::new(view, metrics, typing_y);
    let desk = Desk::of(&eye, view);

    let mut mesh = Mesh::default();
    desk.paint_wall(&mut mesh, view);
    desk.paint_wood(&eye, &mut mesh, view);
    desk.paint_grain(&eye, &mut mesh);
    // Where desk meets wall, a little light is kept out.
    desk.world_band(
        &eye,
        &mut mesh,
        WALL_Y,
        WALL_Y + CONTACT_SHADE_MM,
        Color32::from_black_alpha(60),
        Color32::TRANSPARENT,
    );
    if desk.junction > view.top() {
        let above = Rect::from_min_max(
            pos2(
                view.min.x,
                (desk.junction - 0.02 * view.height()).max(view.min.y),
            ),
            pos2(view.max.x, desk.junction),
        );
        gradient(
            &mut mesh,
            corners(above),
            Color32::from_black_alpha(35),
            Color32::TRANSPARENT,
        );
    }
    painter.add(Shape::mesh(mesh));

    let (first, last) = desk.planks();
    for plank in first + 1..last {
        let y = WALL_Y + plank as f32 * PLANK_MM;
        let left = eye.at(Vec3::new(-desk.half, y, DESK_Z));
        let right = eye.at(Vec3::new(desk.half, y, DESK_Z));
        painter.line_segment([left, right], Stroke::new(1.0, SEAM));
    }
}

/// The span of desk the window shows, in world millimetres.
struct Desk {
    /// Far edge: the wall, or the desk line at the view's top when the zoom
    /// lifts the wall out of the window.
    far: f32,
    /// Near edge, past the window's bottom for the grain's overhang.
    near: f32,
    /// The desk line at the window's bottom: the wood gradient's near stop.
    bottom: f32,
    /// Half-width to paint: covers the window even at the far edge, where a
    /// millimetre is narrowest on screen.
    half: f32,
    /// The screen row where desk meets wall; the eye's screen row depends
    /// on depth alone, so it is one line across the window.
    junction: f32,
}

impl Desk {
    fn of(eye: &Eye, view: Rect) -> Self {
        let far = eye.plane_y(view.top(), DESK_Z).max(WALL_Y);
        let bottom = eye.plane_y(view.bottom(), DESK_Z);
        let near = eye.plane_y(view.bottom() + 50.0, DESK_Z);
        let junction = screen_row(eye, WALL_Y);
        // Screen points per millimetre at the far edge, from a probe.
        let probe = eye.at(Vec3::new(100.0, far, DESK_Z)).x - view.center().x;
        let half = (view.width() / 2.0 + 60.0) * 100.0 / probe.max(1e-3);
        Self {
            far,
            near,
            bottom,
            half,
            junction,
        }
    }

    /// The boards the window shows, counted from the wall.
    fn planks(&self) -> (u64, u64) {
        let first = ((self.far - WALL_Y) / PLANK_MM).floor().max(0.0);
        let last = ((self.near - WALL_Y) / PLANK_MM).ceil().max(first);
        (first as u64, last as u64)
    }

    fn paint_wall(&self, mesh: &mut Mesh, view: Rect) {
        if self.junction <= view.top() {
            return;
        }
        let wall = Rect::from_min_max(view.min, pos2(view.max.x, self.junction + 1.0));
        gradient(mesh, corners(wall), WALL_LOW, WALL_TOP);
    }

    fn paint_wood(&self, eye: &Eye, mesh: &mut Mesh, view: Rect) {
        let top = self.junction.max(view.top());
        for shade in 0..SHADES {
            let y0 = top + (view.bottom() - top) * shade as f32 / SHADES as f32;
            let y1 = top + (view.bottom() - top) * (shade + 1) as f32 / SHADES as f32;
            let quad = [
                Pos2::new(view.left(), y1),
                Pos2::new(view.right(), y1),
                Pos2::new(view.right(), y0),
                Pos2::new(view.left(), y0),
            ];
            gradient(mesh, quad, self.wood(eye, y1), self.wood(eye, y0));
        }
    }

    /// The wood tone at a screen row: its desk depth, between the wall and
    /// the window's bottom.
    fn wood(&self, eye: &Eye, screen_y: f32) -> Color32 {
        let y = eye.plane_y(screen_y, DESK_Z);
        let t = ((y - WALL_Y) / (self.bottom - WALL_Y)).clamp(0.0, 1.0);
        WOOD_FAR.lerp_to_gamma(WOOD_NEAR, t)
    }

    fn paint_grain(&self, eye: &Eye, mesh: &mut Mesh) {
        let (first, last) = self.planks();
        for plank in first..last {
            let back = WALL_Y + plank as f32 * PLANK_MM;
            // Each board a shade apart.
            let tone = splitmix64(SEED ^ plank);
            let tint = if unit(tone, 0) < 0.5 {
                GRAIN_DARK
            } else {
                GRAIN_LIGHT
            };
            self.world_band(
                eye,
                mesh,
                back,
                back + PLANK_MM,
                with_alpha(tint, 10 + (unit(tone, 32) * 20.0) as u8),
                with_alpha(tint, 10 + (unit(tone, 16) * 20.0) as u8),
            );
            for streak in 0..STREAKS_PER_PLANK {
                let bits = splitmix64(SEED ^ (plank << 16) ^ streak);
                Streak {
                    y: back + unit(bits, 0) * PLANK_MM,
                    thickness: 0.5 + unit(bits, 32) * 2.5,
                    amplitude: 1.0 + unit(bits, 8) * 4.0,
                    wavelength: 300.0 + unit(bits, 24) * 600.0,
                    phase: unit(bits, 40) * TAU,
                    colour: if unit(bits, 48) < 0.7 {
                        GRAIN_DARK
                    } else {
                        GRAIN_LIGHT
                    },
                    alpha: 25.0 + unit(bits, 16) * 50.0,
                }
                .paint(eye, mesh, self);
            }
        }
    }

    /// A gradient over a world span of the desk, cut into screen-even bands:
    /// the eye's rows are not evenly spaced in millimetres.
    fn world_band(
        &self,
        eye: &Eye,
        mesh: &mut Mesh,
        y_far: f32,
        y_near: f32,
        far: Color32,
        near: Color32,
    ) {
        let a = y_far.max(self.far);
        let b = y_near.min(self.near);
        if b - a < 1e-3 {
            return;
        }
        let top = screen_row(eye, a);
        let bottom = screen_row(eye, b);
        let steps = ((bottom - top) / 8.0).ceil().clamp(1.0, 16.0) as usize;
        for step in 0..steps {
            let (t0, t1) = (step as f32 / steps as f32, (step + 1) as f32 / steps as f32);
            let quad = band(eye, self.half, lerp(a..=b, t1), lerp(a..=b, t0));
            gradient(
                mesh,
                quad,
                far.lerp_to_gamma(near, t1),
                far.lerp_to_gamma(near, t0),
            );
        }
    }
}

/// A streak of grain, a sine strip along the desk in millimetres.
struct Streak {
    y: f32,
    thickness: f32,
    amplitude: f32,
    wavelength: f32,
    phase: f32,
    colour: Color32,
    alpha: f32,
}

impl Streak {
    fn paint(&self, eye: &Eye, mesh: &mut Mesh, desk: &Desk) {
        if self.y + self.amplitude < desk.far || self.y - self.amplitude > desk.near {
            return;
        }
        let half = self.thickness / 2.0;
        let pixels = screen_row(eye, self.y + half) - screen_row(eye, self.y - half);
        // Under a pixel: a pixel wide, as faint as it is thin.
        let (half, alpha) = if pixels < 1.0 {
            (half / pixels.max(0.05), self.alpha * pixels.max(0.05))
        } else {
            (half, self.alpha)
        };
        let colour = with_alpha(self.colour, alpha as u8);
        let first = mesh.vertices.len() as u32;
        for i in 0..=GRAIN_SEGMENTS {
            let x = desk.half * (2.0 * f32::from(i) / f32::from(GRAIN_SEGMENTS) - 1.0);
            let y = self.y + self.amplitude * (x / self.wavelength * TAU + self.phase).sin();
            mesh.colored_vertex(eye.at(Vec3::new(x, y - half, DESK_Z)), colour);
            mesh.colored_vertex(eye.at(Vec3::new(x, y + half, DESK_Z)), colour);
        }
        for i in 0..u32::from(GRAIN_SEGMENTS) {
            let a = first + 2 * i;
            mesh.add_triangle(a, a + 1, a + 2);
            mesh.add_triangle(a + 1, a + 3, a + 2);
        }
    }
}

/// The screen row a desk depth lands on, the same across the window.
fn screen_row(eye: &Eye, y: f32) -> f32 {
    eye.at(Vec3::new(0.0, y, DESK_Z)).y
}

/// The strip of desk between two depths, near edge first.
fn band(eye: &Eye, half: f32, y_near: f32, y_far: f32) -> [Pos2; 4] {
    [
        eye.at(Vec3::new(-half, y_near, DESK_Z)),
        eye.at(Vec3::new(half, y_near, DESK_Z)),
        eye.at(Vec3::new(half, y_far, DESK_Z)),
        eye.at(Vec3::new(-half, y_far, DESK_Z)),
    ]
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
    use eframe::egui::vec2;
    use typewriter_ui::draw::points_per_inch;

    fn desk_at(zoom_percent: u16) -> (Rect, Eye, Desk) {
        let view = Rect::from_min_size(pos2(0.0, 0.0), vec2(1600.0, 1000.0));
        let profile = typewriter_core::Profile::from_toml_str(include_str!(
            "../../../profiles/olympia-sm9.toml"
        ))
        .expect("the default profile parses");
        let metrics = Metrics::new(&profile, points_per_inch(zoom_percent));
        let typing_y = view.height() * machine::typing_line_height(view, &metrics, zoom_percent);
        let eye = Eye::new(view, &metrics, typing_y);
        let desk = Desk::of(&eye, view);
        (view, eye, desk)
    }

    #[test]
    fn the_desk_runs_from_the_window_bottom_to_the_wall() {
        let (view, eye, desk) = desk_at(100);
        let bottom = eye.at(Vec3::new(0.0, desk.bottom, DESK_Z)).y;
        assert!((bottom - view.bottom()).abs() < 0.5, "{bottom}");
        assert_eq!(desk.far, WALL_Y);
        // The junction stays where the screen-fixed wall used to be.
        assert!(
            (desk.junction - 0.34 * view.height()).abs() < 20.0,
            "{}",
            desk.junction
        );
        let [near_left, near_right, far_right, far_left] =
            band(&eye, desk.half, desk.bottom, desk.far);
        assert!(far_left.x < view.left() && far_right.x > view.right());
        let _ = near_left;
        let _ = near_right;
    }

    #[test]
    fn zooming_in_lifts_the_wall_out_of_the_window() {
        let (view, _, desk) = desk_at(200);
        assert!(desk.junction < view.top(), "{}", desk.junction);
        assert!(
            desk.far > WALL_Y,
            "the desk, not the wall, is the far edge: {}",
            desk.far
        );
    }

    #[test]
    fn zooming_out_pushes_the_wall_down() {
        let (view, _, desk) = desk_at(25);
        assert_eq!(desk.far, WALL_Y);
        assert!(
            desk.junction > 0.34 * view.height(),
            "more desk than before: {}",
            desk.junction
        );
    }
}
