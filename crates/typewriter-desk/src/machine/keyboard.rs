//! The keys, their levers, and the rod the levers rest on.

use eframe::egui::{Align2, Color32, Pos2, Shape, Stroke, pos2};

use super::canvas::Canvas;
use super::case::{OPENING_HALF, PANEL_EDGE_INCHES};
use super::eye::{Eye, FLAT_TEXT, paint_flat_text};
use super::geometry::{add, lerp3, normalized, rounded_rect, soft, sub};
use super::light::{Paint, brighten, matte, polished, streak, toward_light};
use super::panel::PANEL_BOTTOM;
use super::{METAL_SHINE, SHIFT_CAP, SHIFT_FRONT, STEM, STEM_SHINE};
use crate::depth::{Layer, Solid};

/// Keys: the far row's centre `(y, z)` of its tops, one row to the next.
pub(super) const KEY_ROW: (f32, f32) = (5.45, -2.65);
const KEY_ROW_STEP: (f32, f32) = (0.8, -0.28);
/// The space bar's centre `y` and top `z`: standing above the shelf, its
/// foot down in the notch. Half its depth.
pub(super) const SPACE_ROW: (f32, f32) = (8.75, -3.6);
const SPACE_HALF_DEPTH: f32 = 0.2;
/// Keys reach almost wall to wall, as on the real machine.
const KEY_PITCH: f32 = 0.86;
/// A key cap's top, its height, and how much wider its foot is each side.
pub(super) const KEY_CAP: (f32, f32) = (0.64, 0.56);
pub(super) const KEY_FRONT: f32 = 0.4;
const KEY_FLARE: f32 = 0.03;
/// The keyboard's middle, in key pitches from the far row's first key.
const KEYBOARD_MIDDLE: f32 = 5.65;
/// A key's post drops from under its cap; its lever runs back from the post
/// under the rows behind, into the machine under the panel.
const STEM_DROP: f32 = 0.3;
/// The key levers draw together toward the type basket.
const LEVER_CONVERGE: f32 = 0.94;
/// Levers and rods end this far behind the panel's edge, under it: hidden
/// inside the machine.
pub(super) const INTO_MACHINE: f32 = 0.1;
/// The rod the levers rest on: how far behind the far row, its radius, and
/// the gap between its top and the lowest lever over it.
const ROD_BEHIND: f32 = 0.35;
const ROD_RADIUS: f32 = 0.09;
const ROD_GAP: f32 = 0.02;
/// How far below a key's top its shadow falls, cast by the light.
const KEY_SHADOW_DROP: f32 = 0.6;
/// As on a US SM9 De Luxe, far row first: positions in key pitches from
/// the far row's first key, measured from one.
pub(super) const KEYS: [&[Key]; 4] = [
    &[
        Key::new("=", "+", 0.0),
        Key::new("2", "\"", 1.0),
        Key::new("3", "#", 2.0),
        Key::new("4", "$", 3.0),
        Key::new("5", "%", 4.0),
        Key::new("6", "_", 5.0),
        Key::new("7", "&", 6.0),
        Key::new("8", "'", 7.0),
        Key::new("9", "(", 8.0),
        Key::new("0", ")", 9.0),
        Key::new("-", "*", 10.0),
        Key::new("¾", "!", 11.0),
        Key::new("TAB", "", 12.0),
    ],
    &[
        Key::new("MR", "", -0.71),
        Key::new("Q", "", 0.38),
        Key::new("W", "", 1.38),
        Key::new("E", "", 2.38),
        Key::new("R", "", 3.38),
        Key::new("T", "", 4.38),
        Key::new("Y", "", 5.38),
        Key::new("U", "", 6.38),
        Key::new("I", "", 7.38),
        Key::new("O", "", 8.38),
        Key::new("P", "", 9.38),
        Key::new("½", "¼", 10.38),
        Key::new(BACKSPACE, "", 11.6),
    ],
    &[
        Key::new("", "", -0.38),
        Key::new("A", "", 0.64),
        Key::new("S", "", 1.64),
        Key::new("D", "", 2.64),
        Key::new("F", "", 3.64),
        Key::new("G", "", 4.64),
        Key::new("H", "", 5.64),
        Key::new("J", "", 6.64),
        Key::new("K", "", 7.64),
        Key::new("L", "", 8.64),
        Key::new(";", ":", 9.64),
        Key::new("¢", "@", 10.64),
    ],
    &[
        Key::shift(-0.31),
        Key::new("Z", "", 1.22),
        Key::new("X", "", 2.22),
        Key::new("C", "", 3.22),
        Key::new("V", "", 4.22),
        Key::new("B", "", 5.22),
        Key::new("N", "", 6.22),
        Key::new("M", "", 7.22),
        Key::new(",", ",", 8.22),
        Key::new(".", ".", 9.22),
        Key::new("/", "?", 10.22),
        Key::shift(11.53),
    ],
];
/// The backspace key's legend: drawn as an arrow.
const BACKSPACE: &str = "<-";
/// The tab clear key, the space bar and the tab set key: `x`, and legends.
pub(super) const SPACE_BAR: [(f32, f32, &str); 3] = [
    (-4.4, -3.55, "clear"),
    (-3.45, 3.35, ""),
    (3.45, 4.3, "set"),
];
/// The rod the key levers rest on: steel.
const ROD: Color32 = Color32::from_rgb(0x6A, 0x6D, 0x6A);
const CAP: Color32 = Color32::from_rgb(0xF0, 0xED, 0xE0);
const CAP_FRONT: Color32 = Color32::from_rgb(0xD2, 0xCD, 0xBC);
const LEGEND: Color32 = Color32::from_rgb(0x1E, 0x1E, 0x1E);
const SPRING_LIT: Color32 = Color32::from_rgb(0xD6, 0xD6, 0xCE);
const SPRING_DARK: Color32 = Color32::from_rgb(0x5E, 0x5E, 0x58);

/// A key on the keyboard.
pub(super) struct Key {
    legend: &'static str,
    /// Above the legend; empty for none.
    shifted: &'static str,
    /// In key pitches from the far row's first key.
    at: f32,
    /// In key caps.
    width: f32,
    shift: bool,
}

impl Key {
    const fn new(legend: &'static str, shifted: &'static str, at: f32) -> Self {
        Self {
            legend,
            shifted,
            at,
            width: 1.0,
            shift: false,
        }
    }

    const fn shift(at: f32) -> Self {
        Self {
            legend: "",
            shifted: "",
            at,
            width: 1.8,
            shift: true,
        }
    }

    pub(super) fn x(&self) -> f32 {
        (self.at - KEYBOARD_MIDDLE) * KEY_PITCH
    }

    pub(super) fn half_width(&self) -> f32 {
        KEY_CAP.0 * self.width / 2.0
    }
}

/// Where a row's key tops are: `(y, z)`, far row 0.
pub(super) fn key_row(row: f32) -> (f32, f32) {
    (
        KEY_ROW.0 + row * KEY_ROW_STEP.0,
        KEY_ROW.1 + row * KEY_ROW_STEP.1,
    )
}

/// Under every cap, where its post starts: the keys, far row first, then the
/// space bar's.
pub(super) fn key_levers() -> Vec<[f32; 3]> {
    let (space_y, space_z) = SPACE_ROW;
    let keys = KEYS.iter().enumerate().flat_map(|(row, keys)| {
        let (y, z) = key_row(row as f32);
        keys.iter()
            .map(move |key| [key.x(), y - 0.1, z - KEY_FRONT])
    });
    let space = [-3.98, -1.8, 1.8, 3.88].map(|x| [x, space_y - 0.1, space_z - KEY_FRONT]);
    keys.chain(space).collect()
}

/// A lever's foot, where its post ends, and its end under the panel.
fn lever_path(under_cap: [f32; 3]) -> [[f32; 3]; 2] {
    let [x, y, z] = under_cap;
    let (y0, z0) = PANEL_BOTTOM;
    [
        [x, y, z - STEM_DROP],
        [
            x * LEVER_CONVERGE,
            y0 - INTO_MACHINE,
            z0 - PANEL_EDGE_INCHES - 0.05,
        ],
    ]
}

/// Where the lever from `under_cap` crosses depth `y`, if it does.
fn lever_at(under_cap: [f32; 3], y: f32) -> Option<[f32; 3]> {
    let [foot, end] = lever_path(under_cap);
    let t = (foot[1] - y) / (foot[1] - end[1]);
    (0.0..=1.0).contains(&t).then(|| lerp3(foot, end, t))
}

/// Where the rod the levers rest on lies at depth `y`: its axis's `z`, just
/// under the lowest lever crossing there. `None` if none crosses.
fn rod_axis(y: f32, levers: &[[f32; 3]]) -> Option<f32> {
    let low = levers
        .iter()
        .filter_map(|&l| lever_at(l, y))
        .map(|p| p[2])
        .reduce(f32::min)?;
    Some(low - ROD_GAP - ROD_RADIUS)
}

/// The steel rod behind the far row, across the well from wall to wall, under
/// the `levers` crossing there: a cylinder lit round its curve, its top in
/// the light, a highlight where it turns toward it, its underside in shade.
pub(super) fn paint_rod(canvas: &Canvas, eye: &Eye, levers: &[[f32; 3]]) {
    let y = key_row(0.0).0 - ROD_BEHIND;
    let Some(z) = rod_axis(y, levers) else {
        return;
    };
    let half = OPENING_HALF;
    // Round the side the eye sees: from behind its top to under its front.
    let bands = 16u8;
    let around: Vec<([f32; 3], Paint)> = (0..=bands)
        .map(|i| {
            let angle = (-60.0 + 190.0 * f32::from(i) / f32::from(bands)).to_radians();
            let normal = [0.0, angle.sin(), angle.cos()];
            let polish = polished(ROD, METAL_SHINE, normal, 24.0);
            let offset = [0.0, ROD_RADIUS * angle.sin(), ROD_RADIUS * angle.cos()];
            (add([0.0, y, z], offset), polish)
        })
        .collect();
    let mut solid = Solid::default();
    for pair in around.windows(2) {
        let [(a, colour_a), (b, colour_b)] = [pair[0], pair[1]];
        let at = |p: [f32; 3], x: f32| [x, p[1], p[2]];
        eye.quad(
            &mut solid,
            [
                (at(a, -half), colour_a),
                (at(a, half), colour_a),
                (at(b, half), colour_b),
                (at(b, -half), colour_b),
            ],
        );
    }
    canvas.mesh(Layer::Opaque, solid);
}

/// Every cap's shadow on the key bed, the space bar's too.
pub(super) fn paint_shadows(canvas: &Canvas, eye: &Eye) {
    let (space_y, space_z) = SPACE_ROW;
    let depth = KEY_CAP.1 / 2.0;
    for (row, keys) in KEYS.iter().enumerate() {
        let (y, z) = key_row(row as f32);
        for key in keys.iter() {
            let half = key.half_width();
            let x = [key.x() - half, key.x() + half];
            paint_key_shadow(canvas, eye, x, [y - depth, y + depth], z);
        }
    }
    for (left, right, _) in SPACE_BAR {
        let span = [space_y - SPACE_HALF_DEPTH, space_y + SPACE_HALF_DEPTH];
        paint_key_shadow(canvas, eye, [left, right], span, space_z);
    }
}

/// A cap's shadow on the key bed below it, over `x` and `y` ranges, its top
/// at `z`.
fn paint_key_shadow(
    canvas: &Canvas,
    eye: &Eye,
    [left, right]: [f32; 2],
    [back, front]: [f32; 2],
    z: f32,
) {
    let light = toward_light();
    let cast = KEY_SHADOW_DROP / light[2];
    let (dx, dy) = (-light[0] * cast, -light[1] * cast);
    let z = z - KEY_SHADOW_DROP;
    let outline: Vec<Pos2> =
        rounded_rect([left + dx, right + dx], [back + dy, front + dy], z, 0.15)
            .into_iter()
            .map(|[x, y, _]| pos2(x, y))
            .collect();
    let mesh = soft(&outline, 0.1, Color32::from_black_alpha(150));
    // As deep as under the foot casting it: over the rod and walls it falls
    // on, behind every cap, its own too.
    let depths = mesh
        .vertices
        .iter()
        .map(|v| eye.depth([v.pos.x - dx, v.pos.y - dy, z]))
        .collect();
    let mut solid = Solid::unlit(mesh, depths);
    for vertex in &mut solid.mesh.vertices {
        vertex.pos = eye.at([vertex.pos.x, vertex.pos.y, z]);
    }
    canvas.mesh(Layer::Decal, solid);
}

/// Each key's post and its lever, from under its cap in `levers`.
pub(super) fn paint_levers(canvas: &Canvas, eye: &Eye, levers: &[[f32; 3]]) {
    for &lever in levers {
        paint_lever(canvas, eye, lever);
    }
}

/// A key's flat steel post down from under its cap, its spring coiled
/// round it, lit in front and dark behind; and its lever running back from
/// the post under the rows behind, into the machine under the panel, a
/// streak of light along it.
fn paint_lever(canvas: &Canvas, eye: &Eye, under_cap: [f32; 3]) {
    let [x, y, z] = under_cap;
    let [foot, end] = lever_path(under_cap);
    let shine = streak(sub(end, foot), 10).max(0.25);
    paint_steel(canvas, eye, &[under_cap, foot, end], shine);
    let turns = 4u8;
    let coil: Vec<[f32; 3]> = (0..=2 * turns)
        .map(|i| {
            let t = 0.1 + 0.8 * f32::from(i) / f32::from(2 * turns);
            let out = if i % 2 == 0 { -0.07 } else { 0.07 };
            [x + out, y, z - STEM_DROP * t]
        })
        .collect();
    // Strands crossing in front go one way, those behind the other.
    for (i, strand) in coil.windows(2).enumerate() {
        let colour = if i % 2 == 0 { SPRING_LIT } else { SPRING_DARK };
        eye.line(canvas, strand, 0.022, colour);
    }
}

/// A steel rod along `path`, a streak of light `shine` bright (0..=1) down
/// its lit side.
pub(super) fn paint_steel(canvas: &Canvas, eye: &Eye, path: &[[f32; 3]], shine: f32) {
    eye.line(canvas, path, 0.05, STEM);
    let lit: Vec<[f32; 3]> = path.iter().map(|&[x, y, z]| [x - 0.018, y, z]).collect();
    eye.line(canvas, &lit, 0.014, STEM_SHINE.gamma_multiply(shine));
}

/// The caps, far row first, then the tab clear key, the space bar and the
/// tab set key.
pub(super) fn paint_caps(canvas: &Canvas, eye: &Eye) {
    let (space_y, space_z) = SPACE_ROW;
    for (row, keys) in KEYS.iter().enumerate() {
        let (y, z) = key_row(row as f32);
        for key in keys.iter() {
            paint_key(canvas, eye, key, [key.x(), y, z]);
        }
    }
    for (left, right, name) in SPACE_BAR {
        let (back, front) = (space_y - SPACE_HALF_DEPTH, space_y + SPACE_HALF_DEPTH);
        paint_cap(
            canvas,
            eye,
            [left, right],
            [back, front],
            space_z,
            [CAP, CAP_FRONT],
        );
        legend(
            canvas,
            eye,
            [(left + right) / 2.0, space_y, space_z],
            name,
            "",
            0.55,
        );
    }
}

/// A key's cap, its top's centre at `top`, with its legends.
fn paint_key(canvas: &Canvas, eye: &Eye, key: &Key, top: [f32; 3]) {
    let [x, y, z] = top;
    let half = key.half_width();
    let depth = KEY_CAP.1 / 2.0;
    let colours = if key.shift {
        [SHIFT_CAP, SHIFT_FRONT]
    } else {
        [CAP, CAP_FRONT]
    };
    paint_cap(
        canvas,
        eye,
        [x - half, x + half],
        [y - depth, y + depth],
        z,
        colours,
    );
    let size = if key.legend.chars().count() > 1 {
        0.6
    } else {
        1.0
    };
    legend(canvas, eye, top, key.legend, key.shifted, size);
}

/// A rounded cap over `x` and `y` ranges, its top at `z`: its sides down to
/// its flared, rounded foot, lit as they turn; its dished top over them.
pub(super) fn paint_cap(
    canvas: &Canvas,
    eye: &Eye,
    [left, right]: [f32; 2],
    [back, front]: [f32; 2],
    z: f32,
    [top_colour, front_colour]: [Color32; 2],
) {
    let outline = |z: f32, grow: f32| {
        let (l, r, b, f) = (left - grow, right + grow, back - grow, front + grow);
        // The foot rounder than the top: moulded plastic.
        rounded_rect([l, r], [b, f], z, 0.12 + 2.0 * grow)
    };
    let top = outline(z, 0.0);
    let foot = outline(z - KEY_FRONT, KEY_FLARE);
    // Each point faces between its neighbours, tilted up by the flare: the
    // shading rounds the corners instead of stepping.
    let n = top.len();
    let lit: Vec<Paint> = (0..n)
        .map(|i| {
            let along = sub(top[(i + 1) % n], top[(i + n - 1) % n]);
            let out = normalized([along[1], -along[0], 0.0]);
            matte(
                front_colour,
                [out[0] * KEY_FRONT, out[1] * KEY_FRONT, KEY_FLARE],
            )
        })
        .collect();
    let mut sides = Solid::default();
    for i in 0..n {
        let j = (i + 1) % n;
        eye.quad(
            &mut sides,
            [
                (top[i], lit[i]),
                (top[j], lit[j]),
                (foot[j], lit[j]),
                (foot[i], lit[i]),
            ],
        );
    }
    canvas.mesh(Layer::Opaque, sides);
    eye.outline_in(canvas, &foot, brighten(front_colour, 0.8));
    // Dished: its normals turn from the back's to the front's, so the lamp
    // strikes the front and leaves the back its own shade.
    eye.fill(canvas, &top, |[_, py, _]| {
        let tilt = -0.15 + 0.65 * (py - back) / (front - back);
        matte(top_colour, [0.0, tilt, 1.0])
    });
    eye.outline_in(canvas, &top, brighten(front_colour, 0.85));
}

/// `main` printed on a key top at `centre`, `shifted` above it; `size` of a
/// letter's. Laid flat on the top, it foreshortens with it.
fn legend(canvas: &Canvas, eye: &Eye, centre: [f32; 3], main: &str, shifted: &str, size: f32) {
    if main.is_empty() {
        return;
    }
    let [x, y, z] = centre;
    let on_top = |across: f32, down: f32| [x + across, y + down, z];
    if main == BACKSPACE {
        let (half, head) = (0.15, 0.06);
        let stroke = Stroke::new(0.03 * FLAT_TEXT, LEGEND);
        let at = |u: f32, v: f32| pos2(u * FLAT_TEXT, v * FLAT_TEXT);
        let shapes = vec![
            Shape::line_segment([at(-half, 0.0), at(half, 0.0)], stroke),
            Shape::line(
                vec![
                    at(head - half, -head),
                    at(-half, 0.0),
                    at(head - half, head),
                ],
                stroke,
            ),
        ];
        canvas.lay(shapes, |p| {
            let on = on_top(p.x / FLAT_TEXT, p.y / FLAT_TEXT);
            (eye.at(on), eye.lying_depth(on))
        });
    } else if shifted.is_empty() {
        paint_flat_text(
            canvas,
            eye,
            main,
            0.24 * size,
            LEGEND,
            Align2::CENTER_CENTER,
            on_top,
        );
    } else {
        let size = 0.16 * size;
        paint_flat_text(
            canvas,
            eye,
            shifted,
            size,
            LEGEND,
            Align2::CENTER_BOTTOM,
            on_top,
        );
        paint_flat_text(canvas, eye, main, size, LEGEND, Align2::CENTER_TOP, on_top);
    }
}

#[cfg(test)]
mod tests {
    use super::super::case::KEY_BED_Z;
    use super::*;

    #[test]
    fn the_keys_sit_between_the_pillars() {
        for key in KEYS.iter().flat_map(|row| row.iter()) {
            let half = key.half_width();
            assert!(key.x().abs() + half < OPENING_HALF, "{}", key.legend);
        }
    }

    #[test]
    fn the_rod_lies_under_every_lever_and_below_the_far_caps() {
        let levers = key_levers();
        let y = key_row(0.0).0 - ROD_BEHIND;
        let axis = rod_axis(y, &levers).unwrap();
        // Every lever runs back over it.
        for lever in &levers {
            let over = lever_at(*lever, y).unwrap();
            assert!(over[2] > axis + ROD_RADIUS, "{lever:?}");
        }
        assert!(axis + ROD_RADIUS < key_row(0.0).1 - KEY_FRONT);
        assert!(axis - ROD_RADIUS > KEY_BED_Z);
    }

    #[test]
    fn the_layout_starts_with_plus_equals_and_has_no_one() {
        let first = &KEYS[0][0];
        assert_eq!((first.legend, first.shifted), ("=", "+"));
        let keys = KEYS.iter().flat_map(|row| row.iter());
        assert!(keys.clone().all(|key| key.legend != "1"));
        assert!(SPACE_BAR.iter().any(|&(_, _, name)| name == "clear"));
    }
}
