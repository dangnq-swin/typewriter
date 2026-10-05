//! The carriage-return lever. Its base lies flat in a chrome bracket on the
//! side of the carriage's left end, behind the left knob; a slotless screw, a little
//! proud, holds it there and is what it turns about. Out of the bracket it
//! swoops up over the knob, then runs straight toward the writer well past
//! it. A little past the knob its left edge folds down into a wall, whose
//! foot then curls in underneath. Return throws it: the hand pushes its
//! front in toward the keys, against the stop, holds it while the carriage
//! glides home, and lets go.

use std::f32::consts::{FRAC_PI_2, PI};

use eframe::egui::{self, Shape, Stroke};
use glam::{Quat, Vec3};

use super::canvas::Canvas;
use super::eye::Eye;
use super::light::{brighten, chrome_at, paint_chrome, streak, toward_light};
use super::{EDGE, METAL, METAL_SHINE};
use typewriter_ui::draw::{glide_seconds, smoothstep};

/// The bracket, `x` out from the carriage's left end (negative), `y` behind
/// the knob's axis, `z` round it: a thin strip standing out from the end,
/// longer out than deep, low enough that the knob hides most of it.
const BRACKET_X: (f32, f32) = (-13.0, 0.0);
const BRACKET_Y: (f32, f32) = (-28.0, -18.0);
const BRACKET_Z: (f32, f32) = (2.0, 5.0);
/// The screw's middle on the lever's base, its radius, and how proud of the
/// lever it stands.
const SCREW: (f32, f32) = (-8.0, -22.0);
const SCREW_RADIUS: f32 = 1.0;
const SCREW_PROUD: f32 = 1.0;
/// Round the screw's head.
const SCREW_STEPS: u16 = 16;
/// The lever's width, and how far the middle of its metal is above the
/// bracket's lid: half its thickness.
const WIDTH: f32 = 4.0;
const ABOVE_LID: f32 = 1.0;
/// The lever's middle across, straight all along, and its back end.
const LEVER_X: f32 = SCREW.0;
const BACK_Y: f32 = -27.0;
/// The swoop up off the bracket and over the knob, from and to.
const SWOOP_Y: (f32, f32) = (-19.0, -9.0);
/// The run's height out of the swoop and at the tip, and where the tip is.
const RUN_Z: (f32, f32) = (18.0, 17.0);
const TIP_Y: f32 = 66.0;
/// Points along the swoop and the run: enough that no bend shows a corner.
const SWOOP_STEPS: u16 = 12;
const RUN_STEPS: u16 = 20;
/// Along the run, where the left edge starts to fold down and where the
/// curl is whole: the wall deepens as its foot curls in.
const CURL_Y: (f32, f32) = (20.0, 46.0);
/// The fold from flat down into the wall, the wall, and the curl in at its
/// foot: radii, depth, and how far round the curl turns.
const FOLD_RADIUS: f32 = 1.0;
const WALL_DEPTH: f32 = 3.0;
const CURL_RADIUS: f32 = 1.0;
const CURL_DEGREES: f32 = 160.0;
/// Points round the fold and round the curl.
const FOLD_STEPS: usize = 3;
const CURL_STEPS: usize = 4;
/// Across the lever: its right edge, its left edge, the fold, the wall's
/// foot, the curl.
const ACROSS: usize = 2 + FOLD_STEPS + 1 + CURL_STEPS;
/// Thrown, its front swings in toward the keys about the screw.
const THROW_DEGREES: f32 = 7.0;
/// The hand's push to the stop, and the spring's back once let go.
const SWING_SECONDS: f64 = 0.07;
const RELEASE_SECONDS: f64 = 0.18;

/// A return's throw of the lever.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Throw {
    at: f64,
    /// Held until the carriage is home.
    held_seconds: f64,
}

impl Throw {
    /// At `at`, the carriage gliding `mm` millimetres home.
    pub fn new(at: f64, mm: f64) -> Self {
        Self {
            at,
            held_seconds: glide_seconds(mm).max(SWING_SECONDS),
        }
    }

    /// 0 resting, 1 against the stop.
    pub fn amount(&self, now: f64) -> f32 {
        let t = now - self.at;
        if t < 0.0 {
            return 0.0;
        }
        if t < SWING_SECONDS {
            return smoothstep((t / SWING_SECONDS) as f32);
        }
        1.0 - smoothstep(((t - self.held_seconds) / RELEASE_SECONDS) as f32)
    }

    pub fn is_moving(&self, now: f64) -> bool {
        now - self.at < self.held_seconds + RELEASE_SECONDS
    }
}

/// The lever and its screw on the carriage's left end at `left` millimetres,
/// `eye` about the platen's axis, thrown `amount` (0..=1): what moves with
/// the carriage and the throw, kept at rest and shifted a frame.
pub(super) fn paint_moving(canvas: &Canvas, eye: &Eye, left: f32, amount: f32) {
    let strap = Strap::new(left, amount);
    paint_strap(canvas, eye, &strap.frames);
    paint_screw(canvas, eye, strap.screw);
}

/// The bracket's faces toward the eye, for the carriage's left end at
/// `left` millimetres.
pub(super) fn paint_bracket(canvas: &Canvas, eye: &Eye, left: f32) {
    let ((x0, x1), (back, front), (bottom, top)) = (BRACKET_X, BRACKET_Y, BRACKET_Z);
    let (x0, x1) = (x0 + left, x1 + left);
    let inner = [
        Vec3::new(x1, back, top),
        Vec3::new(x1, front, top),
        Vec3::new(x1, front, bottom),
        Vec3::new(x1, back, bottom),
    ];
    // Culling, not ordering: a face the eye cannot see may still stand in
    // the lamp's way.
    if eye.faces_lit(inner[0], Vec3::X) {
        eye.fill(canvas, &inner, |_| METAL);
    }
    paint_chrome(canvas, eye, [x0, x1], front, top, top - bottom);
    let lid = [
        Vec3::new(x0, back, top),
        Vec3::new(x1, back, top),
        Vec3::new(x1, front, top),
        Vec3::new(x0, front, top),
    ];
    eye.fill(canvas, &lid, |_| brighten(METAL_SHINE, 0.9));
    eye.outline(canvas, &lid);
}

/// The slotless screw's head, standing proud at `at`: its rim toward the
/// eye, then its face, brightest toward the light.
fn paint_screw(canvas: &Canvas, eye: &Eye, at: Vec3) {
    let round = |z: f32| -> Vec<Vec3> {
        (0..SCREW_STEPS)
            .map(|step| {
                let turn = std::f32::consts::TAU * f32::from(step) / f32::from(SCREW_STEPS);
                let (sin, cos) = turn.sin_cos();
                at + Vec3::new(SCREW_RADIUS * cos, SCREW_RADIUS * sin, z)
            })
            .collect()
    };
    let (base, face) = (round(0.0), round(SCREW_PROUD));
    let n = base.len();
    for i in 0..n {
        let j = (i + 1) % n;
        let outward = base[i] + base[j] - at * 2.0;
        // Culling, not ordering: the rim's far side may still stand in the
        // lamp's way.
        if eye.faces_lit(base[i], outward) {
            eye.fill(canvas, &[face[i], face[j], base[j], base[i]], |_| METAL);
        }
    }
    let light = toward_light();
    eye.fill(canvas, &face, |p| {
        let toward = (p - at).normalize_or_zero();
        let lit = 0.5 + 0.5 * toward.truncate().dot(light.truncate());
        brighten(METAL, 0.9).lerp_to_gamma(METAL_SHINE, lit)
    });
    eye.outline(canvas, &face);
}

/// The lever through `frames`, back to front: each strip across it chrome
/// for the way its seen side faces. Then its two edges, as single strokes no
/// join can spike, and its tip's cut, showing the fold and curl.
fn paint_strap(canvas: &Canvas, eye: &Eye, frames: &[Frame]) {
    let stroke = |a: Vec3, b: Vec3| {
        let edge = Shape::line_segment([eye.at(a), eye.at(b)], Stroke::new(1.0, EDGE));
        eye.stroke(canvas, &[a, b], edge);
    };
    for pair in frames.windows(2) {
        let (a, b) = (&pair[0], &pair[1]);
        let shine = 0.85 + 0.3 * streak(b.middle - a.middle, 4);
        for k in 0..ACROSS - 1 {
            let strip = [a.across[k], a.across[k + 1], b.across[k + 1], b.across[k]];
            let normal = (strip[2] - strip[0]).cross(strip[3] - strip[1]);
            // Not yet folded: no area.
            if normal.length_squared() < 1e-10 {
                continue;
            }
            let seen = if eye.faces(strip[0], normal) {
                normal
            } else {
                -normal
            };
            // Up to the sky bright, sideways to the light's band, down dark.
            let colour = brighten(chrome_at((1.0 - seen.normalize_or_zero().z) / 2.0), shine);
            eye.fill(canvas, &strip, |_| colour);
        }
        stroke(a.across[0], b.across[0]);
        stroke(a.across[1], b.across[1]);
    }
    if let Some(end) = frames.last() {
        for pair in end.across[1..].windows(2) {
            stroke(pair[0], pair[1]);
        }
        stroke(end.across[0], end.across[1]);
    }
}

/// Across the lever, `(across, up)` millimetres from its middle, `across` toward
/// the keys: its right edge, its left edge, then the fold down, the wall
/// and the curl in underneath, `formed` (0..=1) of the way. Unformed, those
/// all lie on the left edge.
fn profile(formed: f32) -> [(f32, f32); ACROSS] {
    let formed = formed.clamp(0.0, 1.0);
    let mut points = [(WIDTH / 2.0, 0.0); ACROSS];
    let mut at = (-WIDTH / 2.0, 0.0);
    points[1] = at;
    // Out to the left, turning down, then in: turns are counterclockwise.
    // The fold always turns right down, smaller while forming: a lip that
    // deepens, never a wall slanting out.
    let mut heading = PI;
    let mut next = 2;
    let mut walk = |turn: f32, length: f32, at: &mut (f32, f32), heading: &mut f32| {
        *heading += turn / 2.0;
        *at = (at.0 + length * heading.cos(), at.1 + length * heading.sin());
        *heading += turn / 2.0;
        points[next] = *at;
        next += 1;
    };
    let fold = FRAC_PI_2 / FOLD_STEPS as f32;
    for _ in 0..FOLD_STEPS {
        walk(fold, FOLD_RADIUS * fold * formed, &mut at, &mut heading);
    }
    walk(0.0, WALL_DEPTH * formed, &mut at, &mut heading);
    let turn = CURL_DEGREES.to_radians() * formed / CURL_STEPS as f32;
    for _ in 0..CURL_STEPS {
        walk(turn, CURL_RADIUS * turn, &mut at, &mut heading);
    }
    points
}

/// The lever's middle at rest, back to front, each with how formed its
/// curl is there: flat on the bracket, the swoop up over the knob, then the
/// run straight toward the writer. Each part starts level with the last, so
/// the bends are smooth.
fn path() -> Vec<(Vec3, f32)> {
    let lid = BRACKET_Z.1 + ABOVE_LID;
    let swoop = (0..=SWOOP_STEPS).map(|step| {
        let t = f32::from(step) / f32::from(SWOOP_STEPS);
        let z = egui::lerp(lid..=RUN_Z.0, smoothstep(t));
        Vec3::new(LEVER_X, egui::lerp(SWOOP_Y.0..=SWOOP_Y.1, t), z)
    });
    let run = (1..=RUN_STEPS).map(|step| {
        let s = f32::from(step) / f32::from(RUN_STEPS);
        Vec3::new(
            LEVER_X,
            egui::lerp(SWOOP_Y.1..=TIP_Y, s),
            egui::lerp(RUN_Z.0..=RUN_Z.1, s),
        )
    });
    std::iter::once(Vec3::new(LEVER_X, BACK_Y, lid))
        .chain(swoop)
        .chain(run)
        .map(|p| {
            let formed = smoothstep((p.y - CURL_Y.0) / (CURL_Y.1 - CURL_Y.0));
            (p, formed)
        })
        .collect()
}

/// The lever at one point of its path.
#[derive(Debug, Clone, Copy)]
struct Frame {
    middle: Vec3,
    /// Its [`profile`] there, in the machine.
    across: [Vec3; ACROSS],
}

/// The lever along its path, in the machine's millimetres.
struct Strap {
    frames: Vec<Frame>,
    /// Its screw's head, on top of its base.
    screw: Vec3,
}

impl Strap {
    /// For the carriage's left end at `left`, thrown `amount`.
    fn new(left: f32, amount: f32) -> Self {
        let swing = (THROW_DEGREES * amount.clamp(0.0, 1.0)).to_radians();
        let pivot = Vec3::new(SCREW.0, SCREW.1, 0.0);
        let shift = Vec3::new(left, 0.0, 0.0);
        let rotation = Quat::from_rotation_z(-swing);
        let turn = |p: Vec3| rotation * (p - pivot) + pivot + shift;
        let resting = path();
        let points: Vec<Vec3> = resting.iter().map(|&(p, _)| turn(p)).collect();
        let last = points.len() - 1;
        let frames = (0..=last)
            .map(|i| {
                let (before, after) = (points[i.saturating_sub(1)], points[(i + 1).min(last)]);
                let along = (after - before).normalize_or_zero();
                let inward = along.cross(Vec3::Z).normalize_or_zero();
                let up = inward.cross(along).normalize_or_zero();
                let across = profile(resting[i].1).map(|(u, v)| points[i] + inward * u + up * v);
                Frame {
                    middle: points[i],
                    across,
                }
            })
            .collect();
        let screw = turn(Vec3::new(SCREW.0, SCREW.1, BRACKET_Z.1 + 2.0 * ABOVE_LID));
        Self { frames, screw }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::super::carriage::{ends, platen_axis};
    use super::super::knob::{self, COLLAR, DISC};
    use super::*;

    /// The knob's disc: how far out past the carriage's end, and its radius.
    const DISC_REACH: f32 = COLLAR.0 + DISC.0;
    const DISC_RADIUS: f32 = DISC.1;
    // The screw's head sits on the lever, which folds only past the knob.
    const _: () = assert!(SCREW_RADIUS < WIDTH / 2.0 && CURL_Y.0 > DISC_RADIUS);

    #[test]
    fn a_throw_swings_holds_while_the_carriage_glides_and_springs_back() {
        let throw = Throw::new(10.0, 152.0);
        let held = glide_seconds(152.0);
        assert_eq!(throw.amount(9.9), 0.0);
        assert!(throw.amount(10.0 + SWING_SECONDS / 2.0) > 0.0);
        assert_eq!(throw.amount(10.0 + SWING_SECONDS + 0.01), 1.0);
        assert_eq!(throw.amount(10.0 + held), 1.0);
        assert!(throw.amount(10.0 + held + RELEASE_SECONDS / 2.0) < 1.0);
        let over = 10.0 + held + RELEASE_SECONDS + 0.01;
        assert_eq!(throw.amount(over), 0.0);
        assert!(!throw.is_moving(over));
        // Before any return: at rest.
        let never = Throw::new(f64::NEG_INFINITY, 0.0);
        assert!(!never.is_moving(0.0));
        assert_eq!(never.amount(0.0), 0.0);
    }

    #[test]
    fn a_longer_return_holds_the_lever_longer() {
        let (short, long) = (Throw::new(0.0, 13.0), Throw::new(0.0, 152.0));
        let at = glide_seconds(13.0) + RELEASE_SECONDS;
        assert_eq!(short.amount(at), 0.0);
        assert_eq!(long.amount(at), 1.0);
    }

    #[test]
    fn the_bracket_stands_out_from_the_carriage_s_end_the_knob_hiding_its_front() {
        assert_eq!(BRACKET_X.1, 0.0, "on the carriage's end");
        let (across, deep, tall) = (
            BRACKET_X.1 - BRACKET_X.0,
            BRACKET_Y.1 - BRACKET_Y.0,
            BRACKET_Z.1 - BRACKET_Z.0,
        );
        assert!(across > deep && deep > tall, "a thin strip, long across");
        assert!(-BRACKET_X.0 <= DISC_REACH, "no further out than the knob");
        let eye = Eye::testing(600.0, 96.0).about(platen_axis());
        let [end, _] = ends(0.0);
        let [knob, _] = knob::grips(&eye, ends(0.0));
        // The knob hides its outer front.
        for z in [BRACKET_Z.0, BRACKET_Z.1] {
            let corner = eye.at(Vec3::new(end + BRACKET_X.0, BRACKET_Y.1, z));
            assert!(knob.contains(corner), "{corner:?} outside {knob:?}");
        }
    }

    #[test]
    fn its_base_lies_on_the_bracket_held_by_the_screw_resting_and_thrown() {
        for amount in [0.0, 1.0] {
            let strap = Strap::new(0.0, amount);
            let flat = strap
                .frames
                .iter()
                .filter(|f| f.middle.y < SWOOP_Y.0 + 0.03);
            for frame in flat {
                for point in frame.across {
                    let (x, y, z) = (point.x, point.y, point.z);
                    assert!((BRACKET_X.0..=BRACKET_X.1).contains(&x), "{amount}: {x}");
                    assert!(y > BRACKET_Y.0);
                    assert!((z - BRACKET_Z.1 - ABOVE_LID).abs() < 0.26, "on the bracket");
                }
            }
            let (x, y) = (strap.screw.x, strap.screw.y);
            assert!((x - SCREW.0).abs() < 2.5e-3 && (y - SCREW.1).abs() < 2.5e-3);
        }
    }

    #[test]
    fn it_swoops_up_over_the_knob_without_touching_it() {
        for amount in [0.0, 1.0] {
            let strap = Strap::new(0.0, amount);
            for pair in strap.frames.windows(2) {
                for k in 0..ACROSS {
                    let (a, b) = (pair[0].across[k], pair[1].across[k]);
                    for step in 0..=10_u8 {
                        let p = a.lerp(b, f32::from(step) / 10.0);
                        let (x, y, z) = (p.x, p.y, p.z);
                        if (-DISC_REACH..=0.0).contains(&x) && y.abs() < DISC_RADIUS {
                            let surface = (DISC_RADIUS * DISC_RADIUS - y * y).sqrt();
                            assert!(z > surface, "{amount}: {p} into the knob");
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn it_runs_straight_and_its_bends_are_smooth() {
        let points: Vec<Vec3> = path().into_iter().map(|(p, _)| p).collect();
        assert!(points.iter().all(|p| p.x == LEVER_X), "straight");
        for three in points.windows(3) {
            let first = (three[1] - three[0]).normalize_or_zero();
            let second = (three[2] - three[1]).normalize_or_zero();
            let turn = first.dot(second).clamp(-1.0, 1.0).acos();
            assert!(turn.to_degrees() < 25.0, "{three:?}");
        }
        let tip = *points.last().unwrap();
        assert!(tip.y > DISC_RADIUS + 38.0, "well past the knob");
    }

    #[test]
    fn flat_until_the_bend_then_a_wall_whose_foot_curls_in_as_it_deepens() {
        let flat = profile(0.0);
        assert!(flat[1..].iter().all(|&p| p == (-WIDTH / 2.0, 0.0)));
        // Half formed: the wall half down, its foot already turning in.
        let walled = profile(0.5);
        let foot = walled[2 + FOLD_STEPS];
        assert!(foot.0 < -WIDTH / 2.0, "out past the left edge");
        assert!(
            foot.1 < 0.0 && foot.1 > -WALL_DEPTH - FOLD_RADIUS,
            "partway down"
        );
        assert!(walled[ACROSS - 1].1 < foot.1, "curling under");
        // Whole: its end turned back in, under the lever.
        let curled = profile(1.0);
        let end = curled[ACROSS - 1];
        assert!(end.0 > curled[2 + FOLD_STEPS].0 && end.1 < 0.0, "{end:?}");
        let before = path().into_iter().filter(|(p, _)| p.y <= CURL_Y.0);
        assert!(before.map(|(_, formed)| formed).all(|f| f == 0.0));
    }

    #[test]
    fn thrown_its_front_swings_in_toward_the_keys() {
        let tip = |amount| Strap::new(0.0, amount).frames.last().unwrap().middle;
        let (resting, thrown) = (tip(0.0), tip(1.0));
        assert!(thrown.x - resting.x > 7.5, "{resting:?} to {thrown:?}");
        assert_eq!(thrown.z, resting.z, "level");
        let eye = Eye::testing(500.0, 96.0);
        let strap = Strap::new(-150.0, 0.0);
        let (start, end) = (strap.frames[0].middle, strap.frames.last().unwrap().middle);
        // Nearer the writer: lower on screen.
        assert!(eye.at(end).y > eye.at(start).y);
    }
}
