//! The ribbon cover over the type bars, and the type basket in its opening.

use std::f32::consts::PI;

use eframe::egui::{Color32, Mesh, Painter, Shape};

use super::eye::Eye;
use super::geometry::{add, add_fade, add_quad, fillet, sub};
use super::light::{brighten, matte, streak};
use super::{IVORY_LIT, IVORY_SHADE, METAL, METAL_SHINE};
use crate::render::{splitmix64, unit};

/// The ribbon cover, sloping toward the writer from just in front of the
/// ribbon, its back edge up at the alignment guide's foot, to its fold into
/// the front panel just past the opening: back and front `(y, z)`, and its
/// half widths there. From the chair the ribbon shows only through its
/// opening, `(y, half)`: a small one at the back, just holding the ribbon,
/// its vibrator and the segment under them, running out at the back between
/// the plates' tips.
pub(super) const COVER_BACK: (f32, f32) = (0.3, -0.14);
pub(super) const COVER_FRONT: (f32, f32) = (2.8, -0.89);
pub(super) const COVER_HALF: (f32, f32) = (5.95, 6.35);
const OPENING_BACK: (f32, f32) = (0.3, 1.5);
const OPENING_FRONT: (f32, f32) = (2.3, 3.1);
/// The plates' tips, rounded thick; the opening's front corners; the
/// plates' thickness, showing where they drop into the opening.
const PLATE_TIP: f32 = 0.35;
const OPENING_CORNER: f32 = 0.4;
const PLATE_THICKNESS: f32 = 0.3;
/// The type segment, a half disc straight edge back along the ribbon's foot:
/// its teal plate's and silver rim's radii, its height; how far its type
/// bars reach from its centre, past the opening's edges, and how low.
const SEGMENT_CORE: f32 = 0.95;
const SEGMENT_RIM: f32 = 1.1;
const SEGMENT_Z: f32 = -0.76;
const TYPE_BAR_REACH: (f32, f32) = (4.6, -1.9);
/// How far the cover's shadow reaches inside the opening.
const OPENING_SHADE_INCHES: f32 = 0.3;
const INSIDE: Color32 = Color32::from_rgb(0x16, 0x12, 0x0E);
const TYPE_BAR: Color32 = Color32::from_rgb(0x2E, 0x2B, 0x26);
const TEAL: Color32 = Color32::from_rgb(0x3A, 0x68, 0x64);
const TEAL_LIGHT: Color32 = Color32::from_rgb(0x5A, 0x8A, 0x84);
const PALE_RING: Color32 = Color32::from_rgb(0xCC, 0xC6, 0xB2);

/// A point on the ribbon cover's slope.
pub(super) fn on_cover(x: f32, y: f32) -> [f32; 3] {
    let t = (y - COVER_BACK.0) / (COVER_FRONT.0 - COVER_BACK.0);
    [x, y, COVER_BACK.1 + t * (COVER_FRONT.1 - COVER_BACK.1)]
}

/// The opening's corners: the plates' tips at the back, left and right,
/// then its front corners, right and left.
fn opening() -> [[f32; 3]; 4] {
    let (ob, of) = (OPENING_BACK, OPENING_FRONT);
    [
        on_cover(-ob.1, ob.0),
        on_cover(ob.1, ob.0),
        on_cover(of.1, of.0),
        on_cover(-of.1, of.0),
    ]
}

/// Down through the opening: the dark insides, the type bars and segment in
/// them, the cover's shadow round its edges. Before what hangs down into it
/// at the back, the ribbon and its vibrator.
pub(super) fn paint_opening(painter: &Painter, eye: &Eye) {
    eye.fill(painter, &opening(), |_| INSIDE);
    paint_type_basket(painter, eye);
    paint_opening_shade(painter, eye, opening());
}

/// The cover: two plates, their tips rounded thick where the opening runs
/// out at the back, and the strip in front; the plates' thickness where they
/// drop into the opening.
pub(super) fn paint(painter: &Painter, eye: &Eye) {
    let (back, front) = COVER_HALF;
    let (bl, br) = (on_cover(-back, COVER_BACK.0), on_cover(back, COVER_BACK.0));
    let (fl, fr) = (
        on_cover(-front, COVER_FRONT.0),
        on_cover(front, COVER_FRONT.0),
    );
    let [tip_l, tip_r, ofr, ofl] = opening();

    // The plates' inner edges: round each tip, then down to the front.
    let left_edge: Vec<[f32; 3]> = fillet(bl, tip_l, ofl, PLATE_TIP)
        .into_iter()
        .chain([ofl])
        .collect();
    let right_edge: Vec<[f32; 3]> = fillet(br, tip_r, ofr, PLATE_TIP)
        .into_iter()
        .chain([ofr])
        .collect();
    for (edge, facing) in [(&left_edge, 1.0), (&right_edge, -1.0)] {
        paint_plate_wall(painter, eye, edge, facing);
    }
    let lit = |[_, y, _]: [f32; 3]| {
        let t = (y - COVER_BACK.0) / (COVER_FRONT.0 - COVER_BACK.0);
        IVORY_SHADE.lerp_to_gamma(IVORY_LIT, t)
    };
    let left: Vec<[f32; 3]> = fillet(fl, bl, tip_l, 0.25)
        .into_iter()
        .chain(left_edge.iter().copied())
        .chain([fl])
        .collect();
    let right: Vec<[f32; 3]> = fillet(fr, br, tip_r, 0.25)
        .into_iter()
        .chain(right_edge.iter().copied())
        .chain([fr])
        .collect();
    // Square at the fold: the panel below meets it edge for edge.
    for piece in [left, right, vec![ofl, ofr, fr, fl]] {
        eye.fill(painter, &piece, lit);
    }
    // The opening's front corners rounded: cover fills them in.
    for (corner, before, after) in [(ofl, tip_l, ofr), (ofr, tip_r, ofl)] {
        let fill: Vec<[f32; 3]> = std::iter::once(corner)
            .chain(fillet(before, corner, after, OPENING_CORNER))
            .collect();
        eye.fill(painter, &fill, lit);
    }
    // The rounded top of the plates' edges catches the light.
    for edge in [&left_edge, &right_edge] {
        eye.line(painter, edge, 0.03, Color32::from_white_alpha(150));
    }
    let outline = [
        fillet(fl, bl, br, 0.25),
        fillet(bl, br, fr, 0.25),
        vec![fr, fl],
    ]
    .concat();
    eye.outline(painter, &outline);
}

/// A plate's thickness under its inner `edge`, facing into the opening
/// (`facing` +1 right, -1 left): lit as it faces.
fn paint_plate_wall(painter: &Painter, eye: &Eye, edge: &[[f32; 3]], facing: f32) {
    let mut mesh = Mesh::default();
    for pair in edge.windows(2) {
        let [a, b] = [pair[0], pair[1]];
        let along = sub(b, a);
        // Across the edge, the way it faces.
        let across = if along[1] * facing >= 0.0 {
            [along[1], -along[0]]
        } else {
            [-along[1], along[0]]
        };
        let colour = matte(IVORY_SHADE, [across[0], across[1], 0.2]);
        let down = |p: [f32; 3]| [p[0], p[1], p[2] - PLATE_THICKNESS];
        add_quad(
            &mut mesh,
            [
                (eye.at(a), colour),
                (eye.at(b), colour),
                (eye.at(down(b)), brighten(colour, 0.7)),
                (eye.at(down(a)), brighten(colour, 0.7)),
            ],
        );
    }
    painter.add(Shape::mesh(mesh));
}

/// The cover's shadow just inside the `opening`'s sides and front edge: its
/// back is open.
fn paint_opening_shade(painter: &Painter, eye: &Eye, opening: [[f32; 3]; 4]) {
    let [obl, obr, ofr, ofl] = opening;
    let reach = OPENING_SHADE_INCHES;
    let dark = Color32::from_black_alpha(190);
    let mut mesh = Mesh::default();
    // Each edge, and which way is inside from it.
    for (a, b, inward) in [
        (obl, ofl, [reach, 0.0]),
        (obr, ofr, [-reach, 0.0]),
        (ofl, ofr, [0.0, -0.6 * reach]),
    ] {
        let inside = |p: [f32; 3]| eye.at(on_cover(p[0] + inward[0], p[1] + inward[1]));
        add_fade(
            &mut mesh,
            [eye.at(a), eye.at(b)],
            [inside(a), inside(b)],
            dark,
        );
    }
    painter.add(Shape::mesh(mesh));
}

/// The type basket under the cover's opening: the type bars fanned out
/// wider than the opening, the cover cropping them, and the segment they
/// hang in: a teal plate in a brushed silver rim.
fn paint_type_basket(painter: &Painter, eye: &Eye) {
    let centre = OPENING_BACK.0 - 0.15;
    let at =
        |angle: f32, radius: f32, z: f32| [radius * angle.sin(), centre + radius * angle.cos(), z];
    let bars = 52u16;
    let (reach, low) = TYPE_BAR_REACH;
    for i in 0..=bars {
        let angle = (-86.0 + 172.0 * f32::from(i) / f32::from(bars)).to_radians();
        let (from, to) = (at(angle, SEGMENT_RIM, SEGMENT_Z), at(angle, reach, low));
        eye.line(painter, &[from, to], 0.075, TYPE_BAR);
        // A streak down its lit edge, bright where it points to catch the light.
        let shine = streak(sub(to, from), 12);
        let aside = [-0.022 * angle.cos(), 0.022 * angle.sin(), 0.0];
        let edge = [add(from, aside), add(to, aside)];
        let colour = METAL_SHINE.gamma_multiply(0.12 + 0.88 * shine);
        eye.line(painter, &edge, 0.02, colour);
    }

    // The rim: brushed, so its streak runs round it.
    ring(
        painter,
        eye,
        centre,
        [SEGMENT_CORE, SEGMENT_RIM],
        |angle, _| {
            let along = [angle.cos(), -angle.sin(), 0.0];
            METAL.lerp_to_gamma(METAL_SHINE, streak(along, 6))
        },
    );
    ring(painter, eye, centre, [0.0, SEGMENT_CORE], |_, radius| {
        TEAL_LIGHT.lerp_to_gamma(TEAL, radius / SEGMENT_CORE)
    });
    speckle(painter, eye, 90, 0x7E_A1, |u, v| {
        at((u - 0.5) * PI, v.sqrt() * SEGMENT_CORE * 0.97, SEGMENT_Z)
    });
    let edge: Vec<[f32; 3]> = (0..=36u8)
        .map(|i| {
            at(
                (-90.0 + 5.0 * f32::from(i)).to_radians(),
                SEGMENT_CORE,
                SEGMENT_Z,
            )
        })
        .collect();
    eye.line(painter, &edge, 0.025, PALE_RING);
}

/// The front half of an annulus round `(0, centre_y)` at the segment's
/// height, between `radii`; `colour` by angle and radius.
fn ring(
    painter: &Painter,
    eye: &Eye,
    centre_y: f32,
    [inner, outer]: [f32; 2],
    colour: impl Fn(f32, f32) -> Color32,
) {
    let at = |angle: f32, radius: f32| {
        [
            radius * angle.sin(),
            centre_y + radius * angle.cos(),
            SEGMENT_Z,
        ]
    };
    let steps = 40u8;
    let mut mesh = Mesh::default();
    for i in 0..steps {
        let [a, b] =
            [i, i + 1].map(|k| (-90.0 + 180.0 * f32::from(k) / f32::from(steps)).to_radians());
        let corner = |angle: f32, radius: f32| (eye.at(at(angle, radius)), colour(angle, radius));
        add_quad(
            &mut mesh,
            [
                corner(a, inner),
                corner(b, inner),
                corner(b, outer),
                corner(a, outer),
            ],
        );
    }
    painter.add(Shape::mesh(mesh));
}

/// `count` specks of grain, light and dark, at `place(u, v)` for seeded
/// `u` and `v` in 0..=1.
fn speckle(
    painter: &Painter,
    eye: &Eye,
    count: u64,
    seed: u64,
    place: impl Fn(f32, f32) -> [f32; 3],
) {
    for i in 0..count {
        let bits = splitmix64(seed ^ i);
        let p = place(unit(bits, 0), unit(bits, 16));
        let colour = if unit(bits, 32) < 0.5 {
            Color32::from_white_alpha(40)
        } else {
            Color32::from_black_alpha(50)
        };
        let radius = (0.012 * eye.scale(p)).max(0.6);
        painter.circle_filled(eye.at(p), radius, colour);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_type_bars_reach_past_the_opening() {
        let back = OPENING_BACK.0 - 0.15;
        let reach = TYPE_BAR_REACH.0;
        assert!(back + reach > OPENING_FRONT.0);
        assert!(reach * 84.0_f32.to_radians().sin() > OPENING_FRONT.1);
    }
}
