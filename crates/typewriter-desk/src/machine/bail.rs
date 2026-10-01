//! The paper bail across the carriage above the typing line: a thin bar
//! pressing the paper behind the side plates' fronts, the scale printed on
//! it, its rubber rollers clear of the alignment guide. At each end a thin
//! knurled disc floats clear of the side plate, beside the pocket in its
//! top; a short flat stem out of the disc meets the rod running forward
//! from behind, outside the plate.

use std::f32::consts::TAU;

use eframe::egui::Color32;
use typewriter_app::draw::ruler::{self, Scale};
use typewriter_core::carriage::Carriage;

use super::canvas::Canvas;
use super::carriage::{
    ROLLER_BANDS, RUBBER, RUBBER_SHINE, SIDE_PLATE_BACK, SIDE_PLATE_INCHES, cylinder, ends,
    plate_front, platen_axis,
};
use super::eye::{Eye, FLAT_TEXT};
use super::light::matte;
use super::sheet::face_y;
use super::{CHROME, METAL, METAL_SHINE};
use crate::depth::{Layer, Solid};

/// The bar: half its height, and its depth.
const HALF_HEIGHT: f32 = 0.1;
const DEPTH: f32 = 0.06;
/// The rubber rollers either side of the sheet's centre: how far out, half
/// their length, their radius. Their axis is the bar's.
const ROLLER_OUT: f32 = 1.65;
const ROLLER_HALF: f32 = 0.25;
const ROLLER_RADIUS: f32 = 0.21;
// Proud of the bar all round.
const _: () = assert!(ROLLER_RADIUS > DEPTH / 2.0 && ROLLER_RADIUS > HALF_HEIGHT);
/// In from each side plate, across: the rod's gap from the plate, the rod,
/// the gap the disc floats clear by; the disc, its radius, and the knurls
/// round its rim.
const ROD_GAP: f32 = 0.01;
const ROD: f32 = 0.03;
const DISC_GAP: f32 = 0.07;
const DISC: f32 = 0.03;
const DISC_RADIUS: f32 = 0.13;
const KNURLS: u16 = 40;
const KNURL_WIDTH: f32 = 0.006;
/// Half the flat stem's width and thickness, and half the rod's height.
const STEM_HALF: (f32, f32) = (0.04, 0.0125);
const ROD_HALF: f32 = 0.025;
/// The pocket in the side plate's top, past the disc behind, before and
/// below, and the plate left whole ahead of it: the bar runs that far
/// behind the plate's front.
const POCKET_MARGIN: f32 = 0.05;
const AHEAD_OF_POCKET: f32 = 0.1;
/// Searching up the paper for where the bar presses it: how high at most,
/// and how finely.
const HIGHEST: f32 = 4.0;
const SEARCH_STEPS: u16 = 30;
/// Round the disc: enough for a smooth rim.
const ROUND: u16 = 24;
/// Brushed metal down the bar's front, brightest just above the middle.
const BAR_TOP: Color32 = Color32::from_rgb(0xD4, 0xD6, 0xD2);
const BAR_MID: Color32 = Color32::from_rgb(0xEE, 0xEF, 0xEC);
const BAR_LOW: Color32 = Color32::from_rgb(0xAE, 0xB0, 0xAC);

/// The bar's axis `(y, z)`: its disc's pocket closed by the plate ahead of
/// it, its back on the leaning paper.
fn axis() -> (f32, f32) {
    let y = plate_front() - AHEAD_OF_POCKET - POCKET_MARGIN - DISC_RADIUS;
    // The paper leans back as it rises: up until it is that far back.
    let back = y - DEPTH / 2.0;
    let (mut low, mut high) = (0.0, HIGHEST);
    for _ in 0..SEARCH_STEPS {
        let mid = (low + high) / 2.0;
        if face_y(mid) > back {
            low = mid;
        } else {
            high = mid;
        }
    }
    (y, high)
}

/// The bar's front's `y`, top and foot `z`.
fn front() -> (f32, [f32; 2]) {
    let (y, z) = axis();
    (y + DEPTH / 2.0, [z + HALF_HEIGHT, z - HALF_HEIGHT])
}

/// The pocket in a side plate's top beside the disc, open at the top: its
/// back and front `y`, its floor `z`, and the plate's top, level with the
/// disc's.
pub(super) struct Pocket {
    pub(super) back: f32,
    pub(super) front: f32,
    pub(super) floor: f32,
    pub(super) top: f32,
}

pub(super) fn pocket() -> Pocket {
    let (y, z) = axis();
    let reach = DISC_RADIUS + POCKET_MARGIN;
    Pocket {
        back: y - reach,
        front: y + reach,
        floor: z - reach,
        top: z + DISC_RADIUS,
    }
}

/// A side plate's inner face at `x`, `inward` (1 or -1) toward the middle.
#[derive(Clone, Copy)]
struct End {
    x: f32,
    inward: f32,
}

impl End {
    /// `[from, to]` inches in from the plate's inner face, left to right.
    fn span(self, from: f32, to: f32) -> [f32; 2] {
        let [a, b] = [from, to].map(|t| self.x + self.inward * t);
        [a.min(b), a.max(b)]
    }

    fn rod(self) -> [f32; 2] {
        self.span(ROD_GAP, ROD_GAP + ROD)
    }

    /// From the rod's middle in to the disc's outer side.
    fn stem(self) -> [f32; 2] {
        self.span(ROD_GAP + ROD / 2.0, DISC_GAP)
    }

    fn disc(self) -> [f32; 2] {
        self.span(DISC_GAP, DISC_GAP + DISC)
    }

    /// Where the bar meets its disc.
    fn bar(self) -> f32 {
        self.x + self.inward * (DISC_GAP + DISC)
    }
}

/// The side plates' inner faces for the sheet centred `middle` inches across.
fn plate_ends(middle: f32) -> [End; 2] {
    let [left, right] = ends(middle);
    [
        End {
            x: left + SIDE_PLATE_INCHES,
            inward: 1.0,
        },
        End {
            x: right - SIDE_PLATE_INCHES,
            inward: -1.0,
        },
    ]
}

/// The paper bail for the sheet centred `middle` inches across: the bar,
/// its rollers, and at each end the disc, its stem and the rod.
pub(super) fn paint(canvas: &Canvas, eye: &Eye, middle: f32) {
    let [left, right] = plate_ends(middle);
    paint_bar(canvas, eye, [left.bar(), right.bar()]);
    let mut solid = Solid::default();
    for side in [-1.0, 1.0] {
        let x = middle + side * ROLLER_OUT;
        let across = [x - ROLLER_HALF, x + ROLLER_HALF];
        let roller = (across, axis(), ROLLER_RADIUS);
        cylinder(
            eye,
            &mut solid,
            roller,
            ROLLER_BANDS,
            [RUBBER, RUBBER_SHINE],
        );
    }
    canvas.mesh(Layer::Opaque, solid);
    let (y, z) = axis();
    for end in [left, right] {
        paint_disc(canvas, eye, end);
        let stem = [y - STEM_HALF.0, y + STEM_HALF.0];
        let flat = [z - STEM_HALF.1, z + STEM_HALF.1];
        paint_box(canvas, eye, [end.stem(), stem, flat], CHROME);
        // From behind the plate, outside it.
        let rod = [platen_axis()[1] + SIDE_PLATE_BACK, y - STEM_HALF.0];
        paint_box(
            canvas,
            eye,
            [end.rod(), rod, [z - ROD_HALF, z + ROD_HALF]],
            CHROME,
        );
    }
}

/// The bar over `x`: brushed metal down its front, its lid lit from above.
fn paint_bar(canvas: &Canvas, eye: &Eye, [x0, x1]: [f32; 2]) {
    let (front, [top, foot]) = front();
    let back = front - DEPTH;
    let stops = [
        (top, BAR_TOP),
        (top + 0.35 * (foot - top), BAR_MID),
        (foot, BAR_LOW),
    ];
    let mut solid = Solid::default();
    for pair in stops.windows(2) {
        let [(upper, upper_colour), (lower, lower_colour)] = [pair[0], pair[1]];
        eye.quad(
            &mut solid,
            [
                ([x0, front, upper], upper_colour),
                ([x1, front, upper], upper_colour),
                ([x1, front, lower], lower_colour),
                ([x0, front, lower], lower_colour),
            ],
        );
    }
    let lid = matte(BAR_TOP, [0.0, 0.0, 1.0]);
    let under = matte(BAR_LOW, [0.0, 0.0, -1.0]);
    for (z, colour) in [(top, lid), (foot, under)] {
        eye.quad(
            &mut solid,
            [
                ([x0, back, z], colour),
                ([x1, back, z], colour),
                ([x1, front, z], colour),
                ([x0, front, z], colour),
            ],
        );
    }
    canvas.mesh(Layer::Opaque, solid);
    let outline = [
        [x0, front, top],
        [x1, front, top],
        [x1, front, foot],
        [x0, front, foot],
    ];
    eye.outline(canvas, &outline);
}

/// The circle round the bar's axis at `x`, `radius` inches.
fn round_axis(x: f32, radius: f32) -> Vec<[f32; 3]> {
    let (y, z) = axis();
    (0..ROUND)
        .map(|i| {
            let (sin, cos) = (TAU * f32::from(i) / f32::from(ROUND)).sin_cos();
            [x, y + radius * sin, z + radius * cos]
        })
        .collect()
}

/// The thin disc holding the bar's end, knurled round its rim.
fn paint_disc(canvas: &Canvas, eye: &Eye, end: End) {
    let [x0, x1] = end.disc();
    let mut solid = Solid::default();
    let rim = ([x0, x1], axis(), DISC_RADIUS);
    cylinder(eye, &mut solid, rim, ROUND, [METAL, METAL_SHINE]);
    canvas.mesh(Layer::Opaque, solid);
    for (x, out) in [(x0, -1.0), (x1, 1.0)] {
        let lit = matte(CHROME, [out, 0.0, 0.0]);
        eye.fill(canvas, &round_axis(x, DISC_RADIUS), |_| lit);
    }
    let (y, z) = axis();
    for knurl in 0..KNURLS {
        let (sin, cos) = (TAU * f32::from(knurl) / f32::from(KNURLS)).sin_cos();
        let (ky, kz) = (y + DISC_RADIUS * sin, z + DISC_RADIUS * cos);
        let colour = matte(METAL, [0.0, sin, cos]);
        eye.line(canvas, &[[x0, ky, kz], [x1, ky, kz]], KNURL_WIDTH, colour);
    }
}

/// A box over `[x, y, z]` ranges, each face lit as it faces, outlined.
fn paint_box(
    canvas: &Canvas,
    eye: &Eye,
    [[x0, x1], [y0, y1], [z0, z1]]: [[f32; 2]; 3],
    colour: Color32,
) {
    let faces = [
        (
            [0.0, 1.0, 0.0],
            [[x0, y1, z1], [x1, y1, z1], [x1, y1, z0], [x0, y1, z0]],
        ),
        (
            [0.0, -1.0, 0.0],
            [[x0, y0, z1], [x1, y0, z1], [x1, y0, z0], [x0, y0, z0]],
        ),
        (
            [0.0, 0.0, 1.0],
            [[x0, y0, z1], [x1, y0, z1], [x1, y1, z1], [x0, y1, z1]],
        ),
        (
            [0.0, 0.0, -1.0],
            [[x0, y0, z0], [x1, y0, z0], [x1, y1, z0], [x0, y1, z0]],
        ),
        (
            [-1.0, 0.0, 0.0],
            [[x0, y0, z1], [x0, y1, z1], [x0, y1, z0], [x0, y0, z0]],
        ),
        (
            [1.0, 0.0, 0.0],
            [[x1, y0, z1], [x1, y1, z1], [x1, y1, z0], [x1, y0, z0]],
        ),
    ];
    for (normal, face) in faces {
        // Hidden faces only cost: the depth buffer hides them anyway.
        if !eye.faces(face[0], normal) {
            continue;
        }
        let lit = matte(colour, normal);
        eye.fill(canvas, &face, |_| lit);
        eye.outline(canvas, &face);
    }
}

/// Prints `scale`, laid out on screen at the sheet's scale, and `carriage`'s
/// stops on the bar's front: where it shows on screen, for its stops.
pub(super) fn print_scale(canvas: &Canvas, eye: &Eye, scale: &Scale, carriage: &Carriage) -> Scale {
    let (front, [top, foot]) = front();
    let origin = eye.origin.x;
    // Laid out at FLAT_TEXT points an inch, the band the bar's front: sharp
    // up close.
    let flat = scale.stretched(origin, FLAT_TEXT / eye.ppi, 0.0, (top - foot) * FLAT_TEXT);
    let marks = ruler::scale_marks(canvas.painter(), &flat, carriage);
    canvas.lay(marks, |p| {
        let on = [(p.x - origin) / FLAT_TEXT, front, top - p.y / FLAT_TEXT];
        (eye.at(on), eye.lying_depth(on))
    });
    let [top_y, foot_y] = [top, foot].map(|z| eye.at([0.0, front, z]).y);
    let by = eye.scale([0.0, front, (top + foot) / 2.0]) / eye.ppi;
    scale.stretched(origin, by, top_y, foot_y - top_y)
}

#[cfg(test)]
mod tests {
    use super::super::carriage::PLATEN_DIAMETER_INCHES;
    use super::super::eye::screen_up;
    use super::super::printing_point::guide_top;
    use super::*;

    #[test]
    fn the_bar_presses_the_paper_behind_the_plates_its_rollers_clear_of_the_guide() {
        let (y, z) = axis();
        let (front, _) = front();
        assert!((front - DEPTH - face_y(z)).abs() < 1e-4);
        assert!(plate_front() > front, "behind the plates' fronts");
        // Clear on screen too, the guide's glass whole under them.
        let guide = screen_up(guide_top());
        let lowest = (0..36)
            .map(|i| (i as f32 * 10.0).to_radians().sin_cos())
            .map(|(sin, cos)| screen_up([0.0, y + ROLLER_RADIUS * sin, z + ROLLER_RADIUS * cos]))
            .fold(f32::INFINITY, f32::min);
        assert!(lowest > guide, "{lowest}");
    }

    #[test]
    fn its_ends_float_beside_the_side_plates_pockets() {
        let [_, axis_y, axis_z] = platen_axis();
        let Pocket {
            back: pocket_back,
            front: pocket_front,
            floor,
            top,
        } = pocket();
        let (y, z) = axis();
        // Inside the plate's top round the disc, the plate whole ahead of it.
        assert!(axis_y + SIDE_PLATE_BACK < pocket_back);
        assert!((plate_front() - pocket_front - AHEAD_OF_POCKET).abs() < 1e-6);
        assert!(pocket_back < y - DISC_RADIUS && y + DISC_RADIUS < pocket_front);
        assert!(floor < z - DISC_RADIUS && top > z);
        // The rod clear over the platen's end.
        assert!(z - ROD_HALF - axis_z > PLATEN_DIAMETER_INCHES / 2.0, "{z}");
        for end in plate_ends(0.0) {
            let inside = |x: f32| (x - end.x) * end.inward;
            // The rod outside the plate, the disc beyond it, the stem between.
            let [rod, disc, stem] = [end.rod(), end.disc(), end.stem()].map(|s| s.map(inside));
            let near = |s: [f32; 2]| s[0].min(s[1]);
            let far = |s: [f32; 2]| s[0].max(s[1]);
            assert!(near(rod) > 0.0 && far(rod) < near(disc));
            assert!(near(stem) < far(rod) && far(stem) >= near(disc) - 1e-6);
        }
    }

    #[test]
    fn the_printed_scale_shows_where_the_bar_does() {
        let eye = Eye::testing(500.0, 96.0);
        let sm9 = include_str!("../../../../profiles/olympia-sm9.toml");
        let profile = typewriter_core::Profile::from_toml_str(sm9).unwrap();
        let metrics = typewriter_app::draw::Metrics::new(&profile, 96.0);
        let paper_left = eye.origin.x - metrics.paper_size.x / 2.0;
        let scale = Scale::new(&metrics, 80, paper_left, 600.0);
        let ctx = eframe::egui::Context::default();
        let mut printed = None;
        let mut output = ctx.run_ui(Default::default(), |ui| {
            let canvas = Canvas::flat(ui.painter());
            let carriage = Carriage::new(0, 80, 0);
            printed = Some(print_scale(&canvas, &eye, &scale, &carriage));
        });
        output.textures_delta.clear();
        let printed = printed.unwrap();
        let (front, [top, _]) = front();
        let stop = printed.stop(&Carriage::new(0, 80, 0), typewriter_core::Side::Left);
        assert!((stop.top() - eye.at([0.0, front, top]).y).abs() < 1e-3);
        // Nearer than the sheet, so wider on screen: the same inches out on
        // the bar as on the sheet are the same column.
        let on_bar = printed.column_at(eye.at([2.0, front, axis().1]).x);
        let on_sheet = scale.column_at(eye.origin.x + 2.0 * metrics.points_per_inch);
        assert_eq!(on_bar, on_sheet);
    }
}
