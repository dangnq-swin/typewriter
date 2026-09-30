//! The sheet's way through the machine, drawn in depth. Measured along the
//! paper from the printing point: up the front, it bends off the platen to
//! lean back as the paper support does; down, it goes round under the
//! platen and up the support behind, its printed side away from the writer.
//! Across, a point keeps its inches from the printing point.

use eframe::egui::Rect;
use typewriter_app::draw::{Metrics, Placed, SheetWay};

use super::carriage::{PLATEN_DIAMETER_INCHES, STRIKE_DEGREES, platen_axis};
use super::eye::{Eye, toward_eye};
use super::geometry::dot;
use super::light::toward_light;
use super::support;

/// Leaving the printing point, the sheet bends from the platen's slope to
/// the support's lean round this radius.
const BEND_INCHES: f32 = 1.0;
/// Paper: lit over an ambient floor, never past white.
const PAPER_AMBIENT: f32 = 0.74;
const PAPER_LIT: f32 = 0.3;

/// How lit paper facing `normal` is.
pub(super) fn paper_shade(normal: [f32; 3]) -> f32 {
    (PAPER_AMBIENT + PAPER_LIT * dot(normal, toward_light()).max(0.0)).min(1.0)
}

/// The point `along` the way, `(y, z)`, and its printed side's normal.
fn way(along: f32) -> ([f32; 2], [f32; 2]) {
    let strike = STRIKE_DEGREES.to_radians();
    let lean = support::LEAN_DEGREES.to_radians();
    if along >= 0.0 {
        // Its lean back from upright, from the platen's slope to the support's.
        let bend = BEND_INCHES * (strike - lean);
        let tilt = strike - along.min(bend) / BEND_INCHES;
        let y = BEND_INCHES * (strike.cos() - tilt.cos());
        let z = BEND_INCHES * (strike.sin() - tilt.sin());
        let straight = (along - bend).max(0.0);
        let (sin, cos) = tilt.sin_cos();
        return ([y - straight * sin, z + straight * cos], [cos, sin]);
    }
    let wrap = support::wrap_inches();
    if along >= -wrap {
        let radius = PLATEN_DIAMETER_INCHES / 2.0;
        let [_, axis_y, axis_z] = platen_axis();
        let (sin, cos) = (strike + along / radius).sin_cos();
        return ([axis_y + radius * cos, axis_z + radius * sin], [cos, sin]);
    }
    let [ny, nz] = support::facing();
    (support::way(-along - wrap), [-ny, -nz])
}

/// How far forward the sheet's printed face is at height `z` before the
/// platen, near the printing point: where the alignment guide presses.
pub(super) fn face_y(z: f32) -> f32 {
    // Below: past the platen's bottom, where the way turns back up.
    let (mut low, mut high) = (-1.4, 2.0);
    for _ in 0..40 {
        let mid = (low + high) / 2.0;
        if way(mid).0[1] < z {
            low = mid;
        } else {
            high = mid;
        }
    }
    way(high).0[0]
}

/// The top of the sheet's front edge on screen, `along` inches up it.
pub(super) fn front_at(eye: &Eye, along: f32) -> f32 {
    let ([y, z], _) = way(along.max(0.0));
    eye.at([0.0, y, z]).y
}

/// The sheet's way, for the typing line at `typing_y`.
pub fn sheet_way(view: Rect, metrics: &Metrics, typing_y: f32) -> SheetWay {
    let eye = Eye::new(view, metrics, typing_y);
    // The line being typed as bright as a sheet flat on screen.
    let typing = paper_shade(toward_eye());
    SheetWay {
        place: Box::new(move |x, along| {
            let ([y, z], [ny, nz]) = way(along);
            let p = [(x - eye.origin.x) / eye.ppi, y, z];
            let normal = [0.0, ny, nz];
            let facing = Eye::sees(p, normal);
            let seen = if facing { normal } else { normal.map(|c| -c) };
            Placed {
                pos: eye.at(p),
                depth: eye.depth(p),
                print_depth: eye.lying_depth(p),
                lit: (paper_shade(seen) / typing).min(1.0),
                facing,
            }
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_way_runs_on_without_a_break() {
        let step = 0.001;
        let mut along = -8.0;
        while along < 8.0 {
            let ((a, _), (b, _)) = (way(along), way(along + step));
            let gap = (a[0] - b[0]).hypot(a[1] - b[1]);
            assert!((gap - step).abs() < 1e-4, "{along}: {gap}");
            along += step;
        }
    }

    #[test]
    fn the_printing_point_faces_the_eye_and_the_back_turns_away() {
        let ([y, z], [ny, nz]) = way(0.0);
        assert!(y.abs() < 1e-6 && z.abs() < 1e-6);
        let [_, ey, ez] = toward_eye();
        assert!((ny - ey).abs() < 1e-5 && (nz - ez).abs() < 1e-5);
        let behind = -support::wrap_inches() - 2.0;
        let (_, normal) = way(behind);
        assert!(normal[0] < 0.0, "{normal:?}");
    }
}
