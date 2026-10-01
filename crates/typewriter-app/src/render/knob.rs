//! The SM9's platen knobs, one at each end of the platen, seen from above: a
//! thin shaft, a narrow cream collar, then a short knurled disc with a green
//! face. They hang below the margin scale, their tops level with the scale's.
//! Drag or scroll either to roll a half-line at a time.

use eframe::egui::{Color32, Mesh, Painter, Rect, Shape, Stroke, StrokeKind, pos2, vec2};

use typewriter_core::Side;

use super::{HIGHLIGHT, Metrics};

/// Paper edge to collar: just enough shaft that the collar clears the paper.
const SHAFT_INCHES: f32 = 0.08;
const SHAFT_DIAMETER_INCHES: f32 = 0.1;
const COLLAR_INCHES: f32 = 0.22;
const COLLAR_DIAMETER_INCHES: f32 = 0.5;
const DISC_INCHES: f32 = 0.3;
const DISC_DIAMETER_INCHES: f32 = 1.15;
/// The green face, seen edge-on at the disc's outer end.
const FACE_INCHES: f32 = 0.05;
/// Knurled ribs around the disc.
const RIBS: u16 = 60;
/// Shading bands across a cylinder.
const BANDS: u16 = 24;
/// Drag this far (points at 96 ppi) for a notch.
pub const DRAG_POINTS_PER_NOTCH: f32 = 8.0;

const CREAM: Color32 = Color32::from_rgb(0xEC, 0xE6, 0xD4);
const CREAM_SHADE: Color32 = Color32::from_rgb(0xA8, 0xA0, 0x8A);
const RIB: Color32 = Color32::from_rgb(0x8C, 0x84, 0x70);
const FACE: Color32 = Color32::from_rgb(0x4E, 0x80, 0x76);
const FACE_SHADE: Color32 = Color32::from_rgb(0x24, 0x40, 0x3A);
const SHAFT: Color32 = Color32::from_rgb(0x3A, 0x38, 0x36);
const SHAFT_SHADE: Color32 = Color32::from_rgb(0x12, 0x12, 0x12);

/// Where the knob's parts lie on screen.
#[derive(Debug, Clone, Copy)]
pub struct Knob {
    side: Side,
    shaft: Rect,
    collar: Rect,
    disc: Rect,
}

impl Knob {
    /// Beside the paper's `side` edge at `paper_edge`, hanging from `scale_top`.
    pub fn new(metrics: &Metrics, side: Side, paper_edge: f32, scale_top: f32) -> Self {
        let inches = |v: f32| v * metrics.points_per_inch;
        let axis = scale_top + inches(DISC_DIAMETER_INCHES) / 2.0;
        let part = |left: f32, length: f32, diameter: f32| {
            Rect::from_center_size(
                pos2(left + inches(length) / 2.0, axis),
                vec2(inches(length), inches(diameter)),
            )
        };
        // Laid out rightward, then mirrored about the paper's edge for the left.
        let shaft = part(paper_edge, SHAFT_INCHES, SHAFT_DIAMETER_INCHES);
        let collar = part(shaft.right(), COLLAR_INCHES, COLLAR_DIAMETER_INCHES);
        let disc = part(collar.right(), DISC_INCHES, DISC_DIAMETER_INCHES);
        let place = |rect: Rect| match side {
            Side::Right => rect,
            Side::Left => Rect::from_x_y_ranges(
                2.0 * paper_edge - rect.right()..=2.0 * paper_edge - rect.left(),
                rect.y_range(),
            ),
        };
        Self {
            side,
            shaft: place(shaft),
            collar: place(collar),
            disc: place(disc),
        }
    }

    /// What a hand can take hold of: collar and disc.
    pub fn grip(&self) -> Rect {
        self.collar.union(self.disc)
    }

    /// Turned by `rolled` points of paper: rolling without slip, the ribs
    /// keep pace with the sheet.
    pub fn paint(&self, painter: &Painter, rolled: f32, hovered: bool) {
        let mut mesh = Mesh::default();
        cylinder(&mut mesh, self.shaft, SHAFT, SHAFT_SHADE);
        cylinder(&mut mesh, self.collar, CREAM, CREAM_SHADE);
        let face_width = FACE_INCHES / DISC_INCHES * self.disc.width();
        // The face is at the outer end.
        let (body, face) = match self.side {
            Side::Right => self
                .disc
                .split_left_right_at_x(self.disc.right() - face_width),
            Side::Left => {
                let (face, body) = self
                    .disc
                    .split_left_right_at_x(self.disc.left() + face_width);
                (body, face)
            }
        };
        cylinder(&mut mesh, body, CREAM, CREAM_SHADE);
        // The face's rounded edge sits a little in from the rim.
        cylinder(
            &mut mesh,
            face.shrink2(vec2(0.0, 0.03 * face.height())),
            FACE,
            FACE_SHADE,
        );
        painter.add(Shape::mesh(mesh));

        let radius = body.height() / 2.0;
        let step = std::f32::consts::TAU / f32::from(RIBS);
        let turned = rolled / radius;
        for rib in 0..RIBS {
            let angle = (f32::from(rib) * step + turned).rem_euclid(std::f32::consts::TAU);
            // Front half only; ribs crowd toward the rims.
            let facing = angle.cos();
            if facing <= 0.0 {
                continue;
            }
            let y = body.center().y + radius * angle.sin();
            painter.line_segment(
                [pos2(body.left() + 1.0, y), pos2(body.right() - 1.0, y)],
                Stroke::new(1.0, RIB.gamma_multiply(0.3 + 0.7 * facing)),
            );
        }
        if hovered {
            painter.rect_stroke(
                self.disc,
                2.0,
                Stroke::new(1.0, HIGHLIGHT),
                StrokeKind::Outside,
            );
        }
    }
}

/// Paper rolled `done` of `travel` points into a feed that ends resting at
/// `to`, from resting at `from` (`None`: nothing in before). The knob keeps
/// pace with the sheets and makes up the ribs' odd offset on the way, so
/// neither end jumps.
pub fn feed_roll(metrics: &Metrics, from: Option<f32>, to: f32, done: f32, travel: f32) -> f32 {
    let start = to - travel;
    let Some(from) = from.filter(|_| travel > 0.0) else {
        return start + done;
    };
    // One rib to the next, as paper: the disc's radius over the rib angle.
    let pitch = DISC_DIAMETER_INCHES * metrics.points_per_inch / 2.0 * std::f32::consts::TAU
        / f32::from(RIBS);
    let offset = (from - start + pitch / 2.0).rem_euclid(pitch) - pitch / 2.0;
    start + done + offset * (1.0 - done / travel)
}

/// A cylinder lying across the view, lit from the front: `light` facing
/// the viewer, `shade` at the top and bottom edges.
fn cylinder(mesh: &mut Mesh, rect: Rect, light: Color32, shade: Color32) {
    let band = |i: u16| {
        // Equal angles, so bands crowd toward the edges as on a real curve.
        let angle = (f32::from(i) / f32::from(BANDS) - 0.5) * std::f32::consts::PI;
        rect.center().y + rect.height() / 2.0 * angle.sin()
    };
    for i in 0..BANDS {
        let (top, bottom) = (band(i), band(i + 1));
        let middle = (f32::from(i) + 0.5) / f32::from(BANDS) - 0.5;
        let facing = (middle * std::f32::consts::PI).cos();
        let color = shade.lerp_to_gamma(light, facing);
        mesh.add_colored_rect(Rect::from_x_y_ranges(rect.x_range(), top..=bottom), color);
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use typewriter_core::Profile;

    use super::*;

    #[test]
    fn the_knobs_hang_from_the_scale_either_side_of_the_paper() {
        let profile =
            Profile::from_toml_str(include_str!("../../../../profiles/olympia-sm9.toml")).unwrap();
        let metrics = Metrics::new(&profile, 96.0);
        let left = Knob::new(&metrics, Side::Left, 100.0, 300.0);
        let right = Knob::new(&metrics, Side::Right, 800.0, 300.0);
        assert!(left.grip().right() < 100.0);
        assert!(
            left.disc.left() < left.collar.left(),
            "the disc is outermost"
        );
        assert!(
            (left.grip().width() - right.grip().width()).abs() < 1e-3,
            "mirrored"
        );
        assert_eq!(left.grip().top(), right.grip().top());
        let knob = right;
        let grip = knob.grip();
        assert!(grip.left() > 800.0);
        assert!(
            (grip.top() - 300.0).abs() < 1e-3,
            "disc top on the scale's top"
        );
        assert!(knob.collar.height() < knob.disc.height());
        assert!(knob.disc.width() < knob.disc.height() / 3.0, "a thin disc");
    }

    #[test]
    fn a_feed_turns_the_knob_without_a_jump_at_either_end() {
        let profile =
            Profile::from_toml_str(include_str!("../../../../profiles/olympia-sm9.toml")).unwrap();
        let metrics = Metrics::new(&profile, 96.0);
        let knob = Knob::new(&metrics, Side::Right, 800.0, 300.0);
        let pitch = knob.disc.height() / 2.0 * std::f32::consts::TAU / f32::from(RIBS);
        let (from, to, travel) = (700.0, 96.0, 2400.0);
        let start = feed_roll(&metrics, Some(from), to, 0.0, travel);
        let ribs = (start - from) / pitch;
        assert!((ribs - ribs.round()).abs() < 1e-3, "whole ribs off: {ribs}");
        assert_eq!(feed_roll(&metrics, Some(from), to, travel, travel), to);
        let rolls: Vec<f32> = (0..=100)
            .map(|i| feed_roll(&metrics, Some(from), to, 24.0 * i as f32, travel))
            .collect();
        assert!(rolls.windows(2).all(|w| w[1] > w[0]), "turns one way");
        assert_eq!(feed_roll(&metrics, None, to, 0.0, travel), to - travel);
    }
}
