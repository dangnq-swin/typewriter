//! The ribbon cover over the type bars, and the type basket in its opening.

use std::f32::consts::PI;

use crate::depth::{Layer, Placing, Shade, Solid};
use eframe::egui::{Color32, Shape, lerp};
use glam::Vec3;

use super::canvas::Canvas;
use super::eye::Eye;
use super::geometry::fillet;
use super::light::{Paint, brighten, brushed, matte, paint_steel};
use super::{IVORY, IVORY_SHADE, METAL, METAL_SHINE};
use typewriter_ui::draw::{splitmix64, unit};

/// The ribbon cover, sloping toward the writer from just in front of the
/// ribbon, its back edge up at the alignment guide's foot, to its fold into
/// the front panel just past the opening: back and front `(y, z)`, and its
/// half widths there. From the chair the ribbon shows only through its
/// opening, `(y, half)`: a small one at the back, just holding the ribbon,
/// its vibrator and the segment under them, running out at the back between
/// the plates' tips.
pub(super) const COVER_BACK: (f32, f32) = (8.0, -4.0);
pub(super) const COVER_FRONT: (f32, f32) = (71.0, -23.0);
pub(super) const COVER_HALF: (f32, f32) = (151.0, 161.0);
const OPENING_BACK: (f32, f32) = (8.0, 38.0);
const OPENING_FRONT: (f32, f32) = (58.0, 79.0);
/// The plates' tips, rounded thick; the opening's front corners; the
/// plates' thickness, showing where they drop into the opening.
const PLATE_TIP: f32 = 9.0;
const OPENING_CORNER: f32 = 10.0;
const PLATE_THICKNESS: f32 = 8.0;
/// The type segment, a half disc straight edge back along the ribbon's foot:
/// its teal plate's and silver rim's radii, its height; how far its type
/// bars reach from its centre, past the opening's edges, and how low.
const SEGMENT_CORE: f32 = 24.0;
const SEGMENT_RIM: f32 = 28.0;
const SEGMENT_Z: f32 = -19.0;
const TYPE_BAR_REACH: (f32, f32) = (97.0, -48.0);
/// How far the cover's shadow reaches inside the opening.
pub(super) const OPENING_SHADE_MM: f32 = 8.0;
const INSIDE: Color32 = Color32::from_rgb(0x16, 0x12, 0x0E);
const TYPE_BAR: Color32 = Color32::from_rgb(0x2E, 0x2B, 0x26);
const TEAL: Color32 = Color32::from_rgb(0x3A, 0x68, 0x64);
const TEAL_LIGHT: Color32 = Color32::from_rgb(0x5A, 0x8A, 0x84);
const PALE_RING: Color32 = Color32::from_rgb(0xCC, 0xC6, 0xB2);

/// A point on the ribbon cover's slope.
pub(super) fn on_cover(x: f32, y: f32) -> Vec3 {
    let t = (y - COVER_BACK.0) / (COVER_FRONT.0 - COVER_BACK.0);
    Vec3::new(x, y, COVER_BACK.1 + t * (COVER_FRONT.1 - COVER_BACK.1))
}

/// Out of the cover's slope, square to it, unit length.
fn cover_normal() -> Vec3 {
    let dy = COVER_FRONT.0 - COVER_BACK.0;
    let dz = COVER_FRONT.1 - COVER_BACK.1;
    Vec3::new(0.0, -dz, dy).normalize()
}

/// The opening's corners: the plates' tips at the back, left and right,
/// then its front corners, right and left.
fn opening() -> [Vec3; 4] {
    inset_opening(0.0, 0.0)
}

/// The opening with its sides moved in `side` millimetres across and its
/// front `front` millimetres back, corners as [`opening`]'s.
fn inset_opening(side: f32, front: f32) -> [Vec3; 4] {
    let (ob, of) = (OPENING_BACK, OPENING_FRONT);
    let front_y = of.0 - front;
    // The sides slant: their half width where the front now is.
    let t = (front_y - ob.0) / (of.0 - ob.0);
    let (back_half, front_half) = (ob.1 - side, lerp(ob.1..=of.1, t) - side);
    [
        on_cover(-back_half, ob.0),
        on_cover(back_half, ob.0),
        on_cover(front_half, front_y),
        on_cover(-front_half, front_y),
    ]
}

/// Along `opening`'s left side, round its front, `rounding` its front
/// corners, and up its right side.
fn round_the_front(opening: [Vec3; 4], rounding: f32) -> Vec<Vec3> {
    let [bl, br, fr, fl] = opening;
    std::iter::once(bl)
        .chain(fillet(bl, fl, fr, rounding))
        .chain(fillet(fl, fr, br, rounding))
        .chain([br])
        .collect()
}

/// Down through the opening: the dark insides, the type bars and segment in
/// them, the cover's shadow round its edges.
pub(super) fn paint_opening(canvas: &Canvas, eye: &Eye) {
    // Deep under the cover, below the type bars: the cover and panel hide
    // all but what shows through the opening.
    let ((back, _), (front, half)) = (OPENING_BACK, OPENING_FRONT);
    let floor = TYPE_BAR_REACH.1 - 3.0;
    let (wide, back, front) = (half + 51.0, back - 76.0, front + 13.0);
    let insides = [
        Vec3::new(-wide, back, floor),
        Vec3::new(wide, back, floor),
        Vec3::new(wide, front, floor),
        Vec3::new(-wide, front, floor),
    ];
    eye.fill(canvas, &insides, |_| INSIDE);
    paint_type_basket(canvas, eye);
    paint_opening_shade(canvas, eye);
}

/// The cover: two plates, their tips rounded thick where the opening runs
/// out at the back, and the strip in front; the plates' thickness where they
/// drop into the opening.
pub(super) fn paint(canvas: &Canvas, eye: &Eye) {
    let (back, front) = COVER_HALF;
    let (bl, br) = (on_cover(-back, COVER_BACK.0), on_cover(back, COVER_BACK.0));
    let (fl, fr) = (
        on_cover(-front, COVER_FRONT.0),
        on_cover(front, COVER_FRONT.0),
    );
    let [tip_l, tip_r, ofr, ofl] = opening();

    // The plates' inner edges: round each tip, then down to the front.
    let left_edge: Vec<Vec3> = fillet(bl, tip_l, ofl, PLATE_TIP)
        .into_iter()
        .chain([ofl])
        .collect();
    let right_edge: Vec<Vec3> = fillet(br, tip_r, ofr, PLATE_TIP)
        .into_iter()
        .chain([ofr])
        .collect();
    // Round the opening's front corners, as the cover fills them in.
    let left_wall: Vec<Vec3> = fillet(bl, tip_l, ofl, PLATE_TIP)
        .into_iter()
        .chain(fillet(tip_l, ofl, ofr, OPENING_CORNER))
        .collect();
    let right_wall: Vec<Vec3> = fillet(br, tip_r, ofr, PLATE_TIP)
        .into_iter()
        .chain(fillet(tip_r, ofr, ofl, OPENING_CORNER))
        .collect();
    for (edge, facing) in [(&left_wall, 1.0), (&right_wall, -1.0)] {
        paint_plate_wall(canvas, eye, edge, facing);
    }
    // One slope, one normal: the lamp washes it the way the gradient faked.
    let lit = |_| matte(IVORY, cover_normal());
    let left: Vec<Vec3> = fillet(fl, bl, tip_l, 6.0)
        .into_iter()
        .chain(left_edge.iter().copied())
        .chain([fl])
        .collect();
    let right: Vec<Vec3> = fillet(fr, br, tip_r, 6.0)
        .into_iter()
        .chain(right_edge.iter().copied())
        .chain([fr])
        .collect();
    // Square at the fold: the panel below meets it edge for edge.
    for piece in [left, right, vec![ofl, ofr, fr, fl]] {
        eye.fill(canvas, &piece, lit);
    }
    // The opening's front corners rounded: cover fills them in.
    for (corner, before, after) in [(ofl, tip_l, ofr), (ofr, tip_r, ofl)] {
        let fill: Vec<Vec3> = std::iter::once(corner)
            .chain(fillet(before, corner, after, OPENING_CORNER))
            .collect();
        eye.fill(canvas, &fill, lit);
    }
    // The rounded top of the plates' edges catches the light.
    for edge in [&left_wall, &right_wall] {
        eye.line(canvas, edge, 1.0, Color32::from_white_alpha(150));
    }
    let outline = [
        fillet(fl, bl, br, 6.0),
        fillet(bl, br, fr, 6.0),
        vec![fr, fl],
    ]
    .concat();
    eye.outline(canvas, &outline);
}

/// A plate's thickness under its inner `edge`, facing into the opening
/// (`facing` +1 right, -1 left): lit as it faces.
fn paint_plate_wall(canvas: &Canvas, eye: &Eye, edge: &[Vec3], facing: f32) {
    let mut solid = Solid::default();
    for pair in edge.windows(2) {
        let [a, b] = [pair[0], pair[1]];
        // Across the edge, the way it faces: each edge runs back to front.
        let along = b - a;
        let normal = Vec3::new(facing * along.y, -facing * along.x, 0.2);
        let (colour, foot) = (
            matte(IVORY_SHADE, normal),
            matte(brighten(IVORY_SHADE, 0.7), normal),
        );
        let down = |p: Vec3| p - Vec3::Z * PLATE_THICKNESS;
        eye.quad(
            &mut solid,
            [(a, colour), (b, colour), (down(b), foot), (down(a), foot)],
        );
    }
    canvas.mesh(Layer::Opaque, solid);
}

/// The cover's shadow just inside the opening, round its sides and front:
/// its back is open.
fn paint_opening_shade(canvas: &Canvas, eye: &Eye) {
    let reach = OPENING_SHADE_MM;
    let (dark, clear) = (Color32::from_black_alpha(190), Color32::TRANSPARENT);
    let edge = round_the_front(opening(), OPENING_CORNER);
    // A smaller opening inside it, point for point: offsetting each point
    // instead would fold back round the tight front corners.
    let inside = round_the_front(
        inset_opening(reach, 0.6 * reach),
        OPENING_CORNER - 0.6 * reach,
    );
    let mut solid = Solid::default();
    for i in 0..edge.len() - 1 {
        let corners = [
            (edge[i], dark),
            (edge[i + 1], dark),
            (inside[i + 1], clear),
            (inside[i], clear),
        ];
        eye.quad(&mut solid, corners);
    }
    canvas.mesh(Layer::Decal, solid);
}

/// The type basket under the cover's opening: the type bars — the arms that
/// carry their type slugs up to strike the ribbon — fanned out wider than the
/// opening, the cover cropping them, and the segment they hang in: a teal
/// plate in a brushed silver rim. The link from each key lever up to its bar
/// is under the panel, out of sight.
fn paint_type_basket(canvas: &Canvas, eye: &Eye) {
    let centre = OPENING_BACK.0 - 4.0;
    let at = |angle: f32, radius: f32, z: f32| {
        Vec3::new(radius * angle.sin(), centre + radius * angle.cos(), z)
    };
    let bars = 53u16;
    let (reach, low) = TYPE_BAR_REACH;
    for i in 0..bars {
        let angle = (-86.0 + 172.0 * f32::from(i) / f32::from(bars - 1)).to_radians();
        let (from, to) = (at(angle, SEGMENT_RIM, SEGMENT_Z), at(angle, reach, low));
        // Steel, lit like the key levers'.
        paint_steel(canvas, eye, &[from, to], TYPE_BAR, METAL_SHINE);
    }

    // The rim: brushed, so its streak runs round it.
    ring(
        canvas,
        eye,
        centre,
        [SEGMENT_CORE, SEGMENT_RIM],
        |angle, _| {
            brushed(
                METAL,
                METAL_SHINE,
                Vec3::new(angle.cos(), -angle.sin(), 0.0),
                6.0,
            )
        },
    );
    ring(canvas, eye, centre, [0.0, SEGMENT_CORE], |_, radius| {
        TEAL_LIGHT.lerp_to_gamma(TEAL, radius / SEGMENT_CORE)
    });
    speckle(canvas, eye, 90, 0x7E_A1, |u, v| {
        at((u - 0.5) * PI, v.sqrt() * SEGMENT_CORE * 0.97, SEGMENT_Z)
    });
    let edge: Vec<Vec3> = (0..=36u8)
        .map(|i| {
            at(
                (-90.0 + 5.0 * f32::from(i)).to_radians(),
                SEGMENT_CORE,
                SEGMENT_Z,
            )
        })
        .collect();
    eye.line(canvas, &edge, 0.6, PALE_RING);
}

/// The front half of an annulus round `(0, centre_y)` at the segment's
/// height, between `radii`; `colour` by angle and radius.
fn ring<P: Into<Paint>>(
    canvas: &Canvas,
    eye: &Eye,
    centre_y: f32,
    [inner, outer]: [f32; 2],
    colour: impl Fn(f32, f32) -> P,
) {
    let at = |angle: f32, radius: f32| {
        Vec3::new(
            radius * angle.sin(),
            centre_y + radius * angle.cos(),
            SEGMENT_Z,
        )
    };
    let steps = 40u8;
    let mut solid = Solid::default();
    for i in 0..steps {
        let [a, b] =
            [i, i + 1].map(|k| (-90.0 + 180.0 * f32::from(k) / f32::from(steps)).to_radians());
        let corner = |angle: f32, radius: f32| (at(angle, radius), colour(angle, radius));
        eye.quad(
            &mut solid,
            [
                corner(a, inner),
                corner(b, inner),
                corner(b, outer),
                corner(a, outer),
            ],
        );
    }
    canvas.mesh(Layer::Opaque, solid);
}

/// `count` specks of grain, light and dark, at `place(u, v)` for seeded
/// `u` and `v` in 0..=1.
fn speckle(canvas: &Canvas, eye: &Eye, count: u64, seed: u64, place: impl Fn(f32, f32) -> Vec3) {
    for i in 0..count {
        let bits = splitmix64(seed ^ i);
        let p = place(unit(bits, 0), unit(bits, 16));
        let colour = if unit(bits, 32) < 0.5 {
            Color32::from_white_alpha(40)
        } else {
            Color32::from_black_alpha(50)
        };
        let radius = (0.3 * eye.scale(p)).max(0.6);
        let speck = Shape::circle_filled(eye.at(p), radius, colour);
        // Each vertex keeps its spot on screen: its millimetres beside `p`,
        // or the pass would draw the whole speck at one point.
        canvas.lay(vec![speck], |vertex| {
            Some(Placing {
                at: eye.mm_under(p, vertex.pos),
                shade: Shade::Unlit,
            })
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_type_bars_reach_past_the_opening() {
        let back = OPENING_BACK.0 - 4.0;
        let reach = TYPE_BAR_REACH.0;
        assert!(back + reach > OPENING_FRONT.0);
        assert!(reach * 84.0_f32.to_radians().sin() > OPENING_FRONT.1);
    }
}
