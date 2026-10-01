//! The platen knobs at the carriage's ends, on the platen's axis: a cream
//! collar against the end, then a knurled disc with a green face outermost,
//! turning with the platen.

use std::f32::consts::TAU;

use eframe::egui::{Color32, Rect};

use super::canvas::Canvas;
use super::carriage::cylinder;
use super::eye::Eye;
use super::light::matte;
use crate::depth::{Layer, Solid};
use typewriter_app::draw::HIGHLIGHT;

/// Out from the carriage's end: the collar's length and radius, then the
/// disc's.
pub(super) const COLLAR: (f32, f32) = (0.22, 0.25);
pub(super) const DISC: (f32, f32) = (0.3, 0.575);
/// The green face: the disc's outer end, its rounded edge a little in from
/// the rim.
const FACE_INCHES: f32 = 0.05;
const FACE_RADIUS: f32 = 0.97 * DISC.1;
/// Knurled ribs round the disc, and their width.
const RIBS: u16 = 60;
const RIB_WIDTH: f32 = 0.01;
const HIGHLIGHT_WIDTH: f32 = 0.02;
/// Round a knob: enough for a smooth rim.
const BANDS: u16 = 48;
const CREAM: Color32 = Color32::from_rgb(0xEC, 0xE6, 0xD4);
const CREAM_SHINE: Color32 = Color32::from_rgb(0xFA, 0xF7, 0xEE);
const RIB: Color32 = Color32::from_rgb(0x8C, 0x84, 0x70);
const FACE: Color32 = Color32::from_rgb(0x4E, 0x80, 0x76);
const FACE_SHINE: Color32 = Color32::from_rgb(0x8E, 0xB8, 0xAE);

/// A knob at the carriage's end `end`, reaching `out` (-1 left, 1 right).
#[derive(Clone, Copy)]
struct Knob {
    end: f32,
    out: f32,
}

impl Knob {
    /// `from` to `to` inches out from the end, left to right.
    fn span(self, from: f32, to: f32) -> [f32; 2] {
        let [a, b] = [from, to].map(|t| self.end + self.out * t);
        [a.min(b), a.max(b)]
    }

    fn collar(self) -> [f32; 2] {
        self.span(0.0, COLLAR.0)
    }

    /// The disc's knurled body, short of its face.
    fn body(self) -> [f32; 2] {
        self.span(COLLAR.0, COLLAR.0 + DISC.0 - FACE_INCHES)
    }

    fn face(self) -> [f32; 2] {
        self.span(COLLAR.0 + DISC.0 - FACE_INCHES, COLLAR.0 + DISC.0)
    }

    /// Across, inches out from the end.
    fn at(self, t: f32) -> f32 {
        self.end + self.out * t
    }
}

/// The circle round the platen's axis at `x`, `radius` inches.
fn circle(x: f32, radius: f32) -> Vec<[f32; 3]> {
    (0..BANDS)
        .map(|i| {
            let (sin, cos) = (TAU * f32::from(i) / f32::from(BANDS)).sin_cos();
            [x, radius * sin, radius * cos]
        })
        .collect()
}

/// The knobs at the carriage's `ends`, inches across, left then right.
fn knobs([left, right]: [f32; 2]) -> [Knob; 2] {
    [
        Knob {
            end: left,
            out: -1.0,
        },
        Knob {
            end: right,
            out: 1.0,
        },
    ]
}

/// Where each knob shows, to take hold of: `eye` about the platen's axis.
pub(super) fn grips(eye: &Eye, ends: [f32; 2]) -> [Rect; 2] {
    knobs(ends).map(|knob| {
        let rims = [
            (knob.at(0.0), COLLAR.1),
            (knob.at(COLLAR.0), DISC.1),
            (knob.at(COLLAR.0 + DISC.0), DISC.1),
        ];
        let points = rims
            .into_iter()
            .flat_map(|(x, radius)| circle(x, radius))
            .map(|p| eye.at(p));
        Rect::from_points(&points.collect::<Vec<_>>())
    })
}

/// The knobs at the carriage's `ends`, `eye` about the platen's axis,
/// turned `turned` radians with the platen (its front rising), each lit if
/// `hovered`.
pub(super) fn paint(canvas: &Canvas, eye: &Eye, ends: [f32; 2], turned: f32, hovered: [bool; 2]) {
    for (knob, hovered) in knobs(ends).into_iter().zip(hovered) {
        let mut solid = Solid::default();
        let round = |x, radius| (x, (0.0, 0.0), radius);
        let cream = [CREAM, CREAM_SHINE];
        cylinder(
            eye,
            &mut solid,
            round(knob.collar(), COLLAR.1),
            BANDS,
            cream,
        );
        cylinder(eye, &mut solid, round(knob.body(), DISC.1), BANDS, cream);
        let face = [FACE, FACE_SHINE];
        cylinder(
            eye,
            &mut solid,
            round(knob.face(), FACE_RADIUS),
            BANDS,
            face,
        );
        canvas.mesh(Layer::Opaque, solid);
        // The disc's inner side, its body's outer rim round the face, the face.
        let (inward, outward) = ([-knob.out, 0.0, 0.0], [knob.out, 0.0, 0.0]);
        let ends = [
            (knob.at(COLLAR.0), DISC.1, CREAM, inward),
            (
                knob.at(COLLAR.0 + DISC.0 - FACE_INCHES),
                DISC.1,
                CREAM,
                outward,
            ),
            (knob.at(COLLAR.0 + DISC.0), FACE_RADIUS, FACE, outward),
        ];
        for (x, radius, colour, normal) in ends {
            let lit = matte(colour, normal);
            eye.fill(canvas, &circle(x, radius), |_| lit);
        }
        let [from, to] = knob.body();
        for rib in 0..RIBS {
            // From the top round the front: rising at the front, it turns back.
            let around = TAU * f32::from(rib) / f32::from(RIBS) - turned;
            let (sin, cos) = around.sin_cos();
            let (y, z) = (DISC.1 * sin, DISC.1 * cos);
            let colour = matte(RIB, [0.0, sin, cos]).lit();
            eye.line(canvas, &[[from, y, z], [to, y, z]], RIB_WIDTH, colour);
        }
        if hovered {
            for x in [knob.at(COLLAR.0), knob.at(COLLAR.0 + DISC.0)] {
                let mut rim = circle(x, DISC.1);
                rim.extend(rim.first().copied());
                eye.line(canvas, &rim, HIGHLIGHT_WIDTH, HIGHLIGHT);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn collar_then_disc_out_from_each_end_the_face_outermost() {
        let [left, right] = knobs([-5.9, 5.9]);
        assert_eq!(left.collar(), [-5.9 - COLLAR.0, -5.9]);
        assert_eq!(right.collar(), [5.9, 5.9 + COLLAR.0]);
        assert!(left.face()[0] < left.body()[0] && left.body()[1] <= left.collar()[0]);
        assert!(right.face()[1] > right.body()[1] && right.body()[0] >= right.collar()[1]);
        assert!(COLLAR.1 < DISC.1 && FACE_RADIUS < DISC.1);
    }

    #[test]
    fn the_grips_cover_each_knob_either_side() {
        let eye = Eye::testing(500.0, 96.0);
        let [left, right] = grips(&eye, [-5.9, 5.9]);
        assert!(left.right() < right.left());
        assert!((left.width() - right.width()).abs() < 1.0, "mirrored");
        let axis = eye.at([0.0; 3]);
        assert!(left.y_range().contains(axis.y) && left.height() > DISC.1 * 96.0);
    }
}
