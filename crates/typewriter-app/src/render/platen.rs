//! Keeps the typing point still and moves the paper around it.

use std::f32::consts::TAU;

use eframe::egui::{
    Color32, CornerRadius, Painter, Pos2, Rect, Stroke, StrokeKind, Vec2, pos2, vec2,
};

use super::Metrics;

/// Fraction of the view height where the typing line sits.
const TYPING_LINE_HEIGHT: f32 = 0.62;
const JOLT_SECONDS: f64 = 0.18;
const JOLT_AMPLITUDE: f32 = 3.0;
const JOLT_HZ: f32 = 28.0;

#[derive(Debug)]
pub struct PlatenView {
    /// Slide the paper sideways with the carriage. When off, the paper stays
    /// centred and the strike-point marker moves instead.
    pub carriage_travel: bool,
    glide: Glide,
    jolt_started: Option<f64>,
}

pub struct Layout {
    pub paper_origin: Pos2,
    pub strike_point: Pos2,
}

impl PlatenView {
    pub fn new(carriage_travel: bool) -> Self {
        Self {
            carriage_travel,
            glide: Glide::default(),
            jolt_started: None,
        }
    }

    /// Jumps straight to the carriage position, e.g. after a zoom change,
    /// instead of gliding there.
    pub fn snap(&mut self) {
        self.glide = Glide::default();
    }

    pub fn jolt(&mut self, now: f64) {
        self.jolt_started = Some(now);
    }

    pub fn is_animating(&self, now: f64) -> bool {
        self.glide.is_moving(now)
            || self
                .jolt_started
                .is_some_and(|start| now - start < JOLT_SECONDS)
    }

    /// `cell` is the carriage's cell offset on the paper.
    pub fn layout(&mut self, view: Rect, metrics: &Metrics, cell: Vec2, now: f64) -> Layout {
        let carriage = self.glide.follow(cell, metrics, now);
        let fixed = pos2(
            view.center().x,
            view.top() + view.height() * TYPING_LINE_HEIGHT,
        );
        let x = if self.carriage_travel {
            fixed.x - carriage.x - metrics.column_width / 2.0
        } else {
            view.center().x - metrics.paper_size.x / 2.0
        };
        let paper_origin = pos2(x + self.jolt_offset(now), fixed.y - carriage.y);
        let strike_point = pos2(
            paper_origin.x + carriage.x + metrics.column_width / 2.0,
            fixed.y,
        );
        Layout {
            paper_origin,
            strike_point,
        }
    }

    fn jolt_offset(&self, now: f64) -> f32 {
        let Some(start) = self.jolt_started else {
            return 0.0;
        };
        let t = (now - start) as f32;
        if f64::from(t) >= JOLT_SECONDS {
            return 0.0;
        }
        let decay = 1.0 - t / JOLT_SECONDS as f32;
        JOLT_AMPLITUDE * decay * (TAU * JOLT_HZ * t).sin()
    }
}

/// A small pointer just below the typing line, like the type guide. Only
/// below, so it never overlaps the line typed before.
pub fn paint_strike_marker(painter: &Painter, metrics: &Metrics, strike_point: Pos2, opacity: f32) {
    if opacity <= 0.0 {
        return;
    }
    let color = Color32::from_rgba_unmultiplied(0x80, 0x30, 0x20, 0xB0).gamma_multiply(opacity);
    let w = metrics.column_width * 0.45;
    let line = metrics.cell_size().y;
    let gap = 2.0;
    let bottom = strike_point.y + line + gap;
    painter.add(eframe::egui::Shape::convex_polygon(
        vec![
            pos2(strike_point.x, bottom),
            pos2(strike_point.x + w, bottom + w),
            pos2(strike_point.x - w, bottom + w),
        ],
        color,
        Stroke::NONE,
    ));
}

/// Eased movement between carriage positions. Long moves (a carriage return)
/// take longer than a single escapement step.
#[derive(Debug, Default)]
struct Glide {
    from: Vec2,
    to: Vec2,
    start: f64,
    duration: f64,
    initialised: bool,
}

impl Glide {
    fn follow(&mut self, target: Vec2, metrics: &Metrics, now: f64) -> Vec2 {
        if !self.initialised {
            *self = Self {
                from: target,
                to: target,
                start: now,
                duration: 0.0,
                initialised: true,
            };
        } else if target != self.to {
            let inches = f64::from((target - self.value(now)).length() / metrics.points_per_inch);
            self.from = self.value(now);
            self.to = target;
            self.start = now;
            self.duration = (0.05 + inches * 0.05).clamp(0.05, 0.35);
        }
        self.value(now)
    }

    fn is_moving(&self, now: f64) -> bool {
        now - self.start < self.duration
    }

    fn value(&self, now: f64) -> Vec2 {
        if !self.is_moving(now) {
            return self.to;
        }
        let t = ((now - self.start) / self.duration) as f32;
        let eased = 1.0 - (1.0 - t).powi(3);
        self.from + (self.to - self.from) * eased
    }
}

/// The correction slip, held between the ribbon and the paper over the
/// typing point. Translucent enough to see what is struck through it.
pub fn paint_slip(painter: &Painter, metrics: &Metrics, strike_point: Pos2) {
    let line = metrics.cell_size().y;
    let slip = Rect::from_center_size(
        pos2(strike_point.x, strike_point.y + line * 0.5),
        vec2(metrics.column_width * 5.0, line * 1.6),
    );
    painter.rect_filled(
        slip.translate(vec2(1.0, 1.5)),
        CornerRadius::same(1),
        Color32::from_black_alpha(0x18),
    );
    painter.rect(
        slip,
        CornerRadius::same(1),
        Color32::from_rgba_unmultiplied(0xF6, 0xF5, 0xF0, 0xC8),
        Stroke::new(1.0, Color32::from_rgba_unmultiplied(0x9A, 0x94, 0x88, 0x90)),
        StrokeKind::Inside,
    );
}
