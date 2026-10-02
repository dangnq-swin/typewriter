//! A finished sheet going into the folder icon: rolled out of the platen,
//! it flies there shrinking during the wind-out and slips in; the icon answers with a dip,
//! a glow and its count turning over.

use eframe::egui::{Pos2, Vec2, pos2, vec2};

use crate::render::smoothstep;

/// Free of the platen once its bottom edge is this share of its height past
/// the typing line.
const CLEAR_HEIGHT: f32 = 0.05;
/// Rolling out slows to a stop over this last share of the roll.
const ROLL_TAIL: f32 = 0.2;
/// At most this share of the wind-out rolls: the rest is left to fly.
const ROLL_SHARE_MOST: f32 = 0.7;
/// Width at the folder icon, points.
const IN_FOLDER_WIDTH: f32 = 36.0;
/// The flight ends with the top edge this far above where it peeks, points.
const ABOVE_PEEK: f32 = 20.0;
/// After the wind-out: slipping in until only the top edge shows.
const SINK_SECONDS: f64 = 0.2;
/// Top edge showing above the mouth, points; then it drops in.
const PEEK: f32 = 4.0;
const PEEK_SECONDS: f64 = 0.65;
const DROP_SECONDS: f64 = 0.25;
/// After landing: the count turns over, the icon dips and glows.
const TURN_SECONDS: f64 = 0.28;
const DIP_SECONDS: f64 = 0.8;
const DIP_POINTS: f32 = 3.5;
const GLOW_IN_SECONDS: f64 = 0.12;
const GLOW_OUT_FROM_SECONDS: f64 = 1.15;
const ANSWER_SECONDS: f64 = 1.6;
/// In calm mode the icon only shows for the landing: in this long before it.
const REVEAL_BEFORE_SECONDS: f64 = 0.4;
const REVEAL_FADE_SECONDS: f64 = 0.25;

/// A typed sheet on its way into the folder, from the feed's start.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Flight {
    started: f64,
    /// The feed's wind-out, which the flight fills.
    wind_out: f64,
}

/// Where a filed sheet goes, in window points.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Route {
    /// The sheet's centre in the machine.
    pub from: Pos2,
    pub size: Vec2,
    /// The typing line: once past it, the sheet is free of the platen.
    pub platen_y: f32,
    /// Rolling out keeps the pace of a sheet wound past this in the whole
    /// wind-out.
    pub window_top: f32,
    /// The folder icon body's top centre.
    pub mouth: Pos2,
}

/// Where the flying sheet is.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Pose {
    pub centre: Pos2,
    /// Of its size in the machine.
    pub scale: f32,
    /// Shadow strength, as a sheet off the desk.
    pub lift: f32,
    /// Hidden below this y: inside the folder.
    pub mouth: Option<f32>,
}

/// The folder icon's answer to a landing.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Answer {
    /// Downward, points.
    pub dip: f32,
    /// 0..=1 towards the hover colours.
    pub glow: f32,
    /// 0..=1 through the count going up by one; 1 when still.
    pub turn: f32,
}

impl Answer {
    pub const STILL: Self = Self {
        dip: 0.0,
        glow: 0.0,
        turn: 1.0,
    };
}

impl Flight {
    pub fn new(started: f64, wind_out: f64) -> Self {
        Self {
            started,
            wind_out: wind_out.max(0.0),
        }
    }

    /// Seconds from the feed's start to the sheet slipping in.
    fn landing(&self) -> f64 {
        self.wind_out + SINK_SECONDS
    }

    /// Seconds since landing; negative before.
    fn since_landing(&self, now: f64) -> f64 {
        now - self.started - self.landing()
    }

    /// Until then the folder's count is one short: the sheet isn't in yet.
    pub fn has_landed(&self, now: f64) -> bool {
        self.since_landing(now) >= 0.0
    }

    /// Sheet in and the icon still.
    pub fn is_over(&self, now: f64) -> bool {
        self.since_landing(now) >= ANSWER_SECONDS
    }

    /// The sheet rolling out of the machine, then flying into the icon.
    /// `None` once inside.
    pub fn sheet(&self, now: f64, route: &Route) -> Option<Pose> {
        let Route {
            from, size, mouth, ..
        } = *route;
        let t = now - self.started;
        let (roll, pull_end) = self.pull(route);
        let pulled = from - vec2(0.0, roll);
        if t < pull_end {
            let rolled = steady_then_stopping((t / pull_end) as f32);
            return Some(Pose {
                centre: from - vec2(0.0, rolled * roll),
                scale: 1.0,
                lift: 1.0,
                mouth: None,
            });
        }
        let end_scale = (IN_FOLDER_WIDTH / size.x).min(1.0);
        let peek_top = mouth.y - PEEK;
        let centre_at = |top: f32| pos2(mouth.x, top + 0.5 * end_scale * size.y);
        if t < self.wind_out {
            // An arc: across the top first, then down into the folder.
            let u = ease_in_out(((t - pull_end) / (self.wind_out - pull_end)) as f32);
            let end = centre_at(peek_top - ABOVE_PEEK);
            let bend = pos2(
                end.x + 0.15 * (pulled.x - end.x),
                pulled.y + 0.1 * (end.y - pulled.y),
            );
            let centre = pulled.lerp(bend, u).lerp(bend.lerp(end, u), u);
            return Some(Pose {
                centre,
                scale: end_scale.powf(u),
                lift: 1.0,
                mouth: Some(mouth.y),
            });
        }
        let since = t - self.wind_out;
        let (top, lift) = if since < SINK_SECONDS {
            let sunk = ease_out((since / SINK_SECONDS) as f32);
            (peek_top - ABOVE_PEEK * (1.0 - sunk), 1.0)
        } else {
            let dropping = since - SINK_SECONDS - PEEK_SECONDS;
            if dropping >= DROP_SECONDS {
                return None;
            }
            let dropped = ease_in((dropping / DROP_SECONDS) as f32);
            (peek_top + (PEEK + 1.0) * dropped, 0.0)
        };
        Some(Pose {
            centre: centre_at(top),
            scale: end_scale,
            lift,
            mouth: Some(mouth.y),
        })
    }

    /// Points the platen has rolled the sheet out by `now`, and in all.
    pub fn pulled_out(&self, now: f64, route: &Route) -> (f32, f32) {
        let (roll, pull_end) = self.pull(route);
        let t = now - self.started;
        let rolled = if t < pull_end {
            steady_then_stopping((t / pull_end) as f32)
        } else {
            1.0
        };
        (rolled * roll, roll)
    }

    /// How far the platen rolls the sheet out, points, and in how many
    /// seconds from the feed's start.
    fn pull(&self, route: &Route) -> (f32, f64) {
        let bottom = route.from.y + 0.5 * route.size.y;
        let roll = (bottom - route.platen_y).max(0.0) + CLEAR_HEIGHT * route.size.y;
        // The old roll-out's steady pace took the whole sheet past the
        // window's top in the wind-out; the tail's slowing takes longer.
        let exit = (bottom - route.window_top).max(roll);
        let share = (roll / exit / (1.0 - 0.5 * ROLL_TAIL)).min(ROLL_SHARE_MOST);
        (roll, f64::from(share) * self.wind_out)
    }

    pub fn answer(&self, now: f64) -> Answer {
        let a = self.since_landing(now);
        if !(0.0..ANSWER_SECONDS).contains(&a) {
            return Answer::STILL;
        }
        let dip = if a < DIP_SECONDS {
            // A damped bounce, down first.
            let a = a as f32;
            DIP_POINTS * (-7.0 * a).exp() * (18.0 * a).sin()
        } else {
            0.0
        };
        let glow_in = smoothstep((a / GLOW_IN_SECONDS) as f32);
        let glow_out = smoothstep(
            ((a - GLOW_OUT_FROM_SECONDS) / (ANSWER_SECONDS - GLOW_OUT_FROM_SECONDS)) as f32,
        );
        Answer {
            dip,
            glow: glow_in * (1.0 - glow_out),
            turn: smoothstep((a / TURN_SECONDS) as f32),
        }
    }

    /// The icon's opacity in calm mode, where it shows only for the landing.
    pub fn reveal(&self, now: f64) -> f32 {
        let a = self.since_landing(now);
        let fade_in = smoothstep(((a + REVEAL_BEFORE_SECONDS) / REVEAL_FADE_SECONDS) as f32);
        let fade_out =
            smoothstep(((a - ANSWER_SECONDS + REVEAL_FADE_SECONDS) / REVEAL_FADE_SECONDS) as f32);
        fade_in * (1.0 - fade_out)
    }
}

/// 0..=1 at an even pace, slowing to a stop over [`ROLL_TAIL`].
fn steady_then_stopping(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    let pace = 1.0 / (1.0 - 0.5 * ROLL_TAIL);
    let tail_from = 1.0 - ROLL_TAIL;
    if t <= tail_from {
        return pace * t;
    }
    let into = t - tail_from;
    pace * (tail_from + into - into * into / (2.0 * ROLL_TAIL))
}

fn ease_out(t: f32) -> f32 {
    1.0 - (1.0 - t.clamp(0.0, 1.0)).powi(3)
}

fn ease_in(t: f32) -> f32 {
    t.clamp(0.0, 1.0).powi(3)
}

fn ease_in_out(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    if t < 0.5 {
        4.0 * t.powi(3)
    } else {
        1.0 - (2.0 - 2.0 * t).powi(3) / 2.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SIZE: Vec2 = vec2(794.0, 1123.0);
    const FROM: Pos2 = pos2(600.0, 100.0);
    const MOUTH: Pos2 = pos2(40.0, 700.0);
    /// The last line typed: the bottom margin still in.
    const PLATEN_Y: f32 = 580.0;
    const ROUTE: Route = Route {
        from: FROM,
        size: SIZE,
        platen_y: PLATEN_Y,
        window_top: 0.0,
        mouth: MOUTH,
    };

    fn flight() -> Flight {
        Flight::new(10.0, 1.6)
    }

    fn pose(at: f64) -> Option<Pose> {
        flight().sheet(10.0 + at, &ROUTE)
    }

    #[test]
    fn starts_in_the_machine_and_ends_inside_the_folder() {
        let start = pose(0.0).unwrap();
        assert_eq!((start.centre, start.scale, start.mouth), (FROM, 1.0, None));
        let peeking = pose(1.6 + SINK_SECONDS + 0.1).unwrap();
        let top = peeking.centre.y - 0.5 * peeking.scale * SIZE.y;
        assert!((top - (MOUTH.y - PEEK)).abs() < 1e-3, "{top}");
        assert!((peeking.scale * SIZE.x - IN_FOLDER_WIDTH).abs() < 1e-3);
        assert_eq!(peeking.centre.x, MOUTH.x);
        assert_eq!(pose(3.0), None);
    }

    #[test]
    fn rolls_out_at_the_old_steady_pace() {
        // The old roll-out: the whole sheet past the window's top in 1.6 s.
        let old_pace = (FROM.y + 0.5 * SIZE.y) / 1.6;
        let y = |at| pose(at).unwrap().centre.y;
        let pace = (y(0.05) - y(0.15)) / 0.1;
        assert!((pace - old_pace).abs() < 1.0, "{pace} vs {old_pace}");
    }

    #[test]
    fn a_sheet_typed_near_its_top_rolls_all_the_way_out_before_it_flies() {
        let f = flight();
        let route = Route {
            from: pos2(600.0, PLATEN_Y + 0.4 * SIZE.y),
            ..ROUTE
        };
        let poses: Vec<Pose> = (0..=160)
            .filter_map(|i| f.sheet(10.0 + 0.01 * f64::from(i), &route))
            .collect();
        let last_rolling = poses.iter().rposition(|p| p.scale == 1.0).unwrap();
        let out = poses[last_rolling];
        let bottom = out.centre.y + 0.5 * SIZE.y;
        assert!(bottom <= PLATEN_Y - CLEAR_HEIGHT * SIZE.y + 5.0, "{bottom}");
        let flying = poses[last_rolling + 1];
        assert!((flying.centre - out.centre).length() < 10.0, "no jump");
        assert!(last_rolling < 120, "time left to fly");
    }

    #[test]
    fn the_pull_is_the_rolling_sheet_s_travel() {
        let f = flight();
        for i in 0..=30 {
            let at = 10.0 + 0.05 * f64::from(i);
            let (pulled, roll) = f.pulled_out(at, &ROUTE);
            match f.sheet(at, &ROUTE).unwrap() {
                pose if pose.mouth.is_none() => {
                    assert!((FROM.y - pose.centre.y - pulled).abs() < 1e-3);
                }
                _ => assert_eq!(pulled, roll, "still once it flies"),
            }
        }
    }

    #[test]
    fn the_roll_eases_to_a_stop_without_a_jump() {
        assert_eq!(steady_then_stopping(0.0), 0.0);
        assert!((steady_then_stopping(1.0) - 1.0).abs() < 1e-6);
        let steps: Vec<f32> = (0..=100)
            .map(|i| steady_then_stopping(0.01 * i as f32))
            .collect();
        assert!(steps.windows(2).all(|w| w[1] >= w[0]));
        let last_step = steps[100] - steps[99];
        assert!(last_step < 0.001, "{last_step}");
    }

    #[test]
    fn shrinks_steadily_on_the_way() {
        let scales: Vec<f32> = (0..=32)
            .map(|i| pose(0.05 * f64::from(i)).unwrap().scale)
            .collect();
        assert!(scales.windows(2).all(|w| w[1] <= w[0]));
        assert!(scales[0] == 1.0 && *scales.last().unwrap() < 0.1);
    }

    #[test]
    fn nothing_shows_below_the_mouth_once_it_flies() {
        assert_eq!(pose(0.2).unwrap().mouth, None);
        assert_eq!(pose(0.8).unwrap().mouth, Some(MOUTH.y));
        assert_eq!(pose(1.9).unwrap().mouth, Some(MOUTH.y));
    }

    #[test]
    fn the_count_turns_over_only_once_the_sheet_is_in() {
        let f = flight();
        let landing = 10.0 + 1.6 + SINK_SECONDS;
        assert!(!f.has_landed(landing - 0.01));
        assert_eq!(f.answer(landing - 0.01), Answer::STILL);
        let just_in = landing + 1e-6;
        assert!(f.has_landed(just_in));
        assert!(f.answer(just_in).turn < 1e-3);
        let answering = f.answer(landing + 0.1);
        assert!(answering.dip > 0.0 && answering.glow > 0.5, "{answering:?}");
        assert_eq!(f.answer(just_in + TURN_SECONDS).turn, 1.0);
        assert!(!f.is_over(landing + 1.0));
        assert!(f.is_over(just_in + ANSWER_SECONDS));
        assert_eq!(f.answer(just_in + ANSWER_SECONDS), Answer::STILL);
    }

    #[test]
    fn in_calm_mode_the_icon_shows_just_for_the_landing() {
        let f = flight();
        let landing = 10.0 + 1.6 + SINK_SECONDS;
        assert_eq!(f.reveal(10.0), 0.0);
        assert_eq!(f.reveal(landing), 1.0);
        assert_eq!(f.reveal(landing + 1.0), 1.0);
        assert_eq!(f.reveal(landing + ANSWER_SECONDS + 1e-6), 0.0);
    }

    #[test]
    fn without_a_wind_out_it_just_slips_in() {
        let f = Flight::new(0.0, 0.0);
        let first = f.sheet(0.0, &ROUTE).unwrap();
        assert_eq!(first.mouth, Some(MOUTH.y));
        assert!(f.has_landed(SINK_SECONDS));
    }
}
