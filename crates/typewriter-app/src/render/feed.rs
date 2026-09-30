//! Sheet feed motion. The finished sheet winds out steadily during the
//! clicks; the new one rises in step with the knob turns heard in its sound.

use eframe::egui::epaint::{Mesh, Shadow, Vertex};
use eframe::egui::{Color32, Painter, Pos2, Rect, Shape, Stroke, pos2};

use super::background::Background;
use super::smoothstep;

/// After landing: shadow out, pointer in, together over this; then the feed
/// ends. Landing is always at least this long before the sound ends.
pub const SETTLE_SECONDS: f64 = 0.25;
/// Loudness analysis window.
const WINDOW_SECONDS: f64 = 0.05;
/// dB below the loudest window at which the knob counts as still (0) or
/// fully turning (1).
const STILL_DB: f32 = 36.0;
const TURNING_DB: f32 = 20.0;
/// Leading-edge curl at full strength, inches.
const CURL_DEPTH_IN: f32 = 0.4;
const CURL_INSET_IN: f32 = 0.06;

/// A feed's timing: wind-out lasts as long as the clicks; wind-in moves only
/// while the knob is heard.
#[derive(Debug, Clone)]
pub struct FeedMotion {
    wind_out: f64,
    /// Wind-in progress (0..=1) at the end of each window.
    progress: Vec<f32>,
}

impl FeedMotion {
    pub fn from_sound(samples: &[f32], channels: usize, sample_rate: u32) -> Self {
        let frames = samples.len() / channels.max(1);
        let duration = frames as f64 / f64::from(sample_rate.max(1));
        let window = ((WINDOW_SECONDS * f64::from(sample_rate)) as usize * channels).max(1);
        let db: Vec<f32> = samples
            .chunks(window)
            .map(|w| {
                let power = w.iter().map(|s| s * s).sum::<f32>() / w.len() as f32;
                10.0 * (power + 1e-12).log10()
            })
            .collect();
        let loudest = db.iter().copied().fold(f32::MIN, f32::max);
        let turning: Vec<f32> = db
            .iter()
            .map(|d| ((d - loudest + STILL_DB) / (STILL_DB - TURNING_DB)).clamp(0.0, 1.0))
            .collect();
        // Average neighbours: eases each turn in and out.
        let smoothed: Vec<f32> = (0..turning.len())
            .map(|i| {
                let near = &turning[i.saturating_sub(1)..(i + 2).min(turning.len())];
                near.iter().sum::<f32>() / near.len() as f32
            })
            .collect();
        Self::from_speeds(duration, &smoothed)
    }

    /// Even wind-in, for when the sound can't be read.
    pub fn even(duration: f64) -> Self {
        let windows = (duration / WINDOW_SECONDS).ceil() as usize;
        Self::from_speeds(duration, &vec![1.0; windows])
    }

    fn from_speeds(duration: f64, speeds: &[f32]) -> Self {
        let last = ((duration - SETTLE_SECONDS).max(0.0) / WINDOW_SECONDS) as usize;
        let mut total = 0.0;
        let mut progress: Vec<f32> = speeds
            .iter()
            .enumerate()
            .map(|(i, &speed)| {
                if i < last {
                    total += speed;
                }
                total
            })
            .collect();
        if total > 0.0 {
            progress.iter_mut().for_each(|p| *p /= total);
        } else {
            progress.iter_mut().for_each(|p| *p = 1.0);
        }
        Self {
            wind_out: 0.0,
            progress,
        }
    }

    /// Winds the finished sheet out for `seconds` first.
    pub fn after_wind_out(self, seconds: f64) -> Self {
        Self {
            wind_out: seconds.max(0.0),
            ..self
        }
    }

    /// Seconds the finished sheet winds out for.
    pub fn wind_out(&self) -> f64 {
        self.wind_out
    }

    /// First click to pointer back. The sound's quiet tail may still play.
    pub fn duration(&self) -> f64 {
        self.settled_at() + SETTLE_SECONDS
    }

    /// Wind-out progress (0..=1, steady). `None` once gone.
    pub fn roll_out(&self, t: f64) -> Option<f32> {
        (t < self.wind_out).then(|| (t / self.wind_out) as f32)
    }

    /// Wind-in progress (0..=1).
    pub fn progress(&self, t: f64) -> f32 {
        let at = (t - self.wind_out) / WINDOW_SECONDS;
        if at <= 0.0 {
            return 0.0;
        }
        let i = at.floor() as usize;
        let Some(&end) = self.progress.get(i) else {
            return 1.0;
        };
        let start = i.checked_sub(1).map_or(0.0, |j| self.progress[j]);
        start + (end - start) * (at - at.floor()) as f32
    }

    /// When the new sheet stops for good.
    fn settled_at(&self) -> f64 {
        let windows = self.progress.iter().take_while(|&&p| p < 1.0).count() + 1;
        self.wind_out + windows as f64 * WINDOW_SECONDS
    }

    /// Shadow strength: full while moving, fading as the pointer fades in.
    pub fn lift(&self, t: f64) -> f32 {
        1.0 - self.pointer_opacity(t)
    }

    /// Leading-edge curl: curled while winding, flat on landing.
    pub fn curl(&self, t: f64) -> f32 {
        1.0 - smoothstep((self.progress(t) - 0.7) / 0.3)
    }

    pub fn pointer_opacity(&self, t: f64) -> f32 {
        smoothstep(((t - self.settled_at()) / SETTLE_SECONDS) as f32)
    }
}

/// A sheet held off the desk: shadow, then a body of the background paper,
/// so only edges, curl and shadow show.
pub fn paint_lifted_sheet(
    painter: &Painter,
    background: &Background,
    view: Rect,
    sheet: Rect,
    points_per_inch: f32,
    curl: f32,
    lift: f32,
) {
    if lift > 0.0 {
        let shadow = Shadow {
            offset: [0, (4.0 * lift).round() as i8],
            blur: 16,
            spread: 0,
            color: Color32::from_black_alpha((70.0 * lift) as u8),
        };
        painter.add(shadow.as_shape(sheet, 0));
    }

    // Curled back toward the platen, the edge foreshortens and narrows.
    let depth = curl * CURL_DEPTH_IN * points_per_inch;
    let inset = curl * CURL_INSET_IN * points_per_inch;
    let lip = [
        pos2(sheet.left() + inset, sheet.top()),
        pos2(sheet.right() - inset, sheet.top()),
    ];
    let fold = [
        pos2(sheet.left(), sheet.top() + depth),
        pos2(sheet.right(), sheet.top() + depth),
    ];
    background.paint_polygon(
        painter,
        view,
        &[
            lip[0],
            lip[1],
            fold[1],
            sheet.right_bottom(),
            sheet.left_bottom(),
            fold[0],
        ],
    );
    if curl > 0.0 {
        let shade = Color32::from_black_alpha((40.0 * curl) as u8);
        let mut band = Mesh::default();
        for (pos, color) in [
            (lip[0], shade),
            (lip[1], shade),
            (fold[1], Color32::TRANSPARENT),
            (fold[0], Color32::TRANSPARENT),
        ] {
            band.colored_vertex(pos, color);
        }
        band.add_triangle(0, 1, 2);
        band.add_triangle(0, 2, 3);
        painter.add(Shape::mesh(band));
        let highlight = Color32::from_white_alpha((150.0 * curl) as u8);
        painter.line_segment(lip, Stroke::new(1.0, highlight));
    }
}

/// A triangle fan over a convex polygon.
pub fn convex_mesh(points: &[Pos2], mut vertex: impl FnMut(Pos2) -> Vertex) -> Mesh {
    let mut mesh = Mesh::default();
    mesh.vertices.extend(points.iter().map(|&p| vertex(p)));
    for i in 1..points.len().saturating_sub(1) as u32 {
        mesh.add_triangle(0, i, i + 1);
    }
    mesh
}

#[cfg(test)]
mod tests {
    use super::*;

    const RATE: u32 = 1000;

    /// Silence, a knob turn from 1 s to 2 s, silence until 4 s.
    fn one_turn() -> FeedMotion {
        let samples: Vec<f32> = (0..4 * RATE)
            .map(|i| {
                if (RATE..2 * RATE).contains(&i) {
                    0.5
                } else {
                    0.0
                }
            })
            .collect();
        FeedMotion::from_sound(&samples, 1, RATE)
    }

    #[test]
    fn moves_only_while_the_knob_turns() {
        let m = one_turn();
        assert!(m.progress(0.9) < 0.01);
        assert!((m.progress(1.5) - 0.5).abs() < 0.1);
        assert!(m.progress(2.1) > 0.99);
        assert_eq!(m.progress(3.0), 1.0);
    }

    #[test]
    fn progress_never_goes_back() {
        let m = one_turn();
        let mut last = 0.0;
        for i in 0..=400 {
            let p = m.progress(f64::from(i) / 100.0);
            assert!(p >= last);
            last = p;
        }
    }

    #[test]
    fn in_place_before_the_pointer_fades_in() {
        // Knob turning right to the end of the sound.
        let m = FeedMotion::from_sound(&vec![0.5; 3 * RATE as usize], 1, RATE);
        assert_eq!(m.progress(3.0 - SETTLE_SECONDS), 1.0);
        assert_eq!(m.pointer_opacity(3.0 - SETTLE_SECONDS), 0.0);
        assert_eq!(m.pointer_opacity(2.0), 0.0);
        assert_eq!(m.pointer_opacity(3.0), 1.0);
    }

    #[test]
    fn even_motion_when_the_sound_is_missing() {
        let m = FeedMotion::even(2.5);
        let moving = 2.5 - SETTLE_SECONDS;
        assert!((m.progress(moving / 2.0) - 0.5).abs() < 0.03);
        assert_eq!(m.progress(moving), 1.0);
    }

    #[test]
    fn curl_and_shadow_settle() {
        let m = one_turn();
        assert_eq!(m.curl(0.0), 1.0);
        assert_eq!(m.curl(3.0), 0.0);
        assert_eq!(m.lift(1.5), 1.0);
        assert_eq!(m.lift(3.5), 0.0);
    }

    #[test]
    fn shadow_fades_out_as_the_pointer_fades_in_then_the_feed_ends() {
        let m = one_turn();
        let end = m.duration();
        // The turn ends at 2 s; the feed does not wait for the quiet 2 s after.
        assert!(end > 2.0 && end < 2.5, "{end}");
        let landed = end - SETTLE_SECONDS;
        assert_eq!(m.progress(landed), 1.0);
        assert_eq!((m.lift(landed), m.pointer_opacity(landed)), (1.0, 0.0));
        assert_eq!((m.lift(end), m.pointer_opacity(end)), (0.0, 1.0));
        for i in 0..=10 {
            let t = landed + SETTLE_SECONDS * f64::from(i) / 10.0;
            assert!((m.lift(t) + m.pointer_opacity(t) - 1.0).abs() < 1e-6);
        }
    }

    #[test]
    fn old_sheet_winds_out_steadily_before_the_new_one_moves() {
        let m = one_turn().after_wind_out(1.0);
        assert!((m.duration() - (one_turn().duration() + 1.0)).abs() < 1e-9);
        assert_eq!(m.roll_out(0.0), Some(0.0));
        assert_eq!(m.roll_out(0.25), Some(0.25));
        assert_eq!(m.roll_out(0.5), Some(0.5));
        assert_eq!(m.roll_out(1.0), None);
        // The new sheet's own timing moves back by the wind-out.
        assert_eq!(m.progress(1.9), 0.0);
        assert!((m.progress(2.5) - 0.5).abs() < 0.1);
        assert_eq!(m.progress(4.0), 1.0);
        assert_eq!(m.pointer_opacity(2.9), 0.0);
        assert_eq!(m.pointer_opacity(m.duration()), 1.0);
    }

    #[test]
    fn first_sheet_only_winds_in() {
        let m = one_turn().after_wind_out(0.0);
        assert_eq!(m.roll_out(0.0), None);
        assert!((m.progress(1.5) - one_turn().progress(1.5)).abs() < 1e-6);
        assert!((m.duration() - one_turn().duration()).abs() < 1e-9);
    }

    #[test]
    fn convex_mesh_fans_out() {
        let square = [
            pos2(0.0, 0.0),
            pos2(1.0, 0.0),
            pos2(1.0, 1.0),
            pos2(0.0, 1.0),
        ];
        let mesh = convex_mesh(&square, |pos| Vertex {
            pos,
            uv: pos,
            color: Color32::WHITE,
        });
        assert_eq!(mesh.vertices.len(), 4);
        assert_eq!(mesh.indices, [0, 1, 2, 0, 2, 3]);
    }
}
