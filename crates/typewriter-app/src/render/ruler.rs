//! The carriage scale (margin and tab marks) and the plates below it:
//! Spacing, Zoom, Correct, Goal and Autosave.

use std::f32::consts::PI;

use eframe::egui::{
    Align2, Color32, CornerRadius, FontId, Painter, Pos2, Rect, Shape, Stroke, pos2, vec2,
};
use typewriter_core::carriage::Carriage;
use typewriter_core::session::Progress;
use typewriter_core::{EraseMode, Goal, LineSpacing, Side};

use super::Metrics;
use crate::filing::{Keeping, WriteStatus};

pub const HEIGHT: f32 = 20.0;
/// Typing line bottom to scale.
const GAP: f32 = 10.0;
/// Scale to plates, and plate to plate.
const PLATE_GAP: f32 = 4.0;
const PLATE_PADDING: f32 = 6.0;
const PLATE_FONT_SIZE: f32 = 10.0;

const SCALE: Color32 = Color32::from_rgba_premultiplied(0xDC, 0xD8, 0xCE, 0xEE);
const SCALE_EDGE: Color32 = Color32::from_rgb(0x9A, 0x94, 0x88);
const TICKS: Color32 = Color32::from_rgb(0x3A, 0x36, 0x32);
const MARGIN: Color32 = Color32::from_rgb(0xA0, 0x2C, 0x1C);
const TAB: Color32 = Color32::from_rgb(0x24, 0x4C, 0x7A);
const SPACING: Color32 = Color32::from_rgb(0x24, 0x22, 0x20);
const SAVED: Color32 = Color32::from_rgb(0x3C, 0x8A, 0x3C);
const WRITING: Color32 = Color32::from_rgb(0xD0, 0x90, 0x20);
const FAILED: Color32 = Color32::from_rgb(0xB0, 0x30, 0x1C);
const DOT_RADIUS: f32 = 3.5;
const STOP_FOOT: f32 = 4.0;
const RELEASED_OPACITY: f32 = 0.35;
const RELEASED_LIFT: f32 = 3.0;

/// The scale's top edge for a strike point.
pub fn top(metrics: &Metrics, strike_point: Pos2) -> f32 {
    strike_point.y + metrics.cell_size().y + GAP
}

/// Where the scale lies. Laid out from the paper's left edge: it travels with
/// the carriage, like the SM9's.
#[derive(Debug, Clone, Copy)]
pub struct Scale {
    rect: Rect,
    /// Column 0's left edge.
    grid_left: f32,
    column_width: f32,
    columns: u16,
}

impl Scale {
    pub fn new(metrics: &Metrics, columns: u16, paper_left: f32, top: f32) -> Self {
        Self {
            rect: Rect::from_min_size(pos2(paper_left, top), vec2(metrics.paper_size.x, HEIGHT)),
            grid_left: paper_left + metrics.grid_origin.x,
            column_width: metrics.column_width,
            columns,
        }
    }

    fn column_left(&self, column: u16) -> f32 {
        self.grid_left + f32::from(column) * self.column_width
    }

    fn column_centre(&self, column: u16) -> f32 {
        self.column_left(column) + self.column_width / 2.0
    }

    /// A margin stop's grip: its bracket, a little wider.
    pub fn stop(&self, carriage: &Carriage, side: Side) -> Rect {
        let (column, foot) = match side {
            Side::Left => (carriage.left_margin, STOP_FOOT),
            Side::Right => (carriage.right_margin, -STOP_FOOT),
        };
        let x = self.column_left(column);
        let (left, right) = (x.min(x + foot), x.max(x + foot));
        Rect::from_x_y_ranges(left - 3.0..=right + 3.0, self.rect.y_range())
    }

    /// The column edge nearest `x`, on the paper.
    pub fn column_at(&self, x: f32) -> u16 {
        let column = ((x - self.grid_left) / self.column_width).round();
        // Safe cast: clamped to the paper's columns.
        column.clamp(0.0, f32::from(self.columns)) as u16
    }
}

pub fn paint_scale(painter: &Painter, scale: &Scale, carriage: &Carriage) {
    let rect = scale.rect;
    paint_plate(painter, rect);
    let bottom = rect.bottom() - 1.0;
    let label_font = FontId::proportional(8.0);

    for column in 0..scale.columns {
        let x = scale.column_centre(column);
        let length = match column % 10 {
            0 => 7.0,
            5 => 5.0,
            _ => 3.0,
        };
        painter.line_segment(
            [pos2(x, bottom - length), pos2(x, bottom)],
            Stroke::new(1.0, TICKS),
        );
        if column % 10 == 0 {
            painter.text(
                pos2(x, rect.top() + 1.0),
                Align2::CENTER_TOP,
                column,
                label_font.clone(),
                TICKS,
            );
        }
    }

    for stop in carriage.tab_stops() {
        paint_tab_mark(painter, scale.column_centre(stop), rect);
    }
    let released = carriage.margin_released;
    let left = scale.column_left(carriage.left_margin);
    let right = scale.column_left(carriage.right_margin);
    paint_margin_mark(painter, left, rect, STOP_FOOT, released);
    paint_margin_mark(painter, right, rect, -STOP_FOOT, released);
}

/// A bracket opening toward the writing area, `foot` long (negative: the
/// right stop). Released, it stands lifted and faded until the return.
fn paint_margin_mark(painter: &Painter, x: f32, rect: Rect, foot: f32, released: bool) {
    let (color, lift) = if released {
        (MARGIN.gamma_multiply(RELEASED_OPACITY), RELEASED_LIFT)
    } else {
        (MARGIN, 0.0)
    };
    let (top, bottom) = (rect.top() + 1.0 - lift, rect.bottom() - 1.0 - lift);
    painter.add(Shape::line(
        vec![
            pos2(x + foot, top),
            pos2(x, top),
            pos2(x, bottom),
            pos2(x + foot, bottom),
        ],
        Stroke::new(2.0, color),
    ));
}

/// A small pointer hanging from the scale's top.
fn paint_tab_mark(painter: &Painter, x: f32, rect: Rect) {
    let w = 3.5;
    let top = rect.top() + 1.0;
    painter.add(Shape::convex_polygon(
        vec![pos2(x - w, top), pos2(x + w, top), pos2(x, top + w * 1.6)],
        TAB,
        Stroke::NONE,
    ));
}

/// Below the scale, flush left. Two circles: first filled; second empty (1),
/// half (1.5) or full (2). Returns the plate's rect.
pub fn paint_spacing_indicator(
    painter: &Painter,
    spacing: LineSpacing,
    left: f32,
    top: f32,
) -> Rect {
    let radius = 5.0;
    let gap = 4.0;
    let label = painter.layout_no_wrap(
        "Spacing:".into(),
        FontId::proportional(PLATE_FONT_SIZE),
        TICKS,
    );
    let circles_width = radius * 4.0 + gap;
    let size = vec2(
        PLATE_PADDING + label.size().x + gap * 1.5 + circles_width + PLATE_PADDING,
        HEIGHT,
    );
    let rect = Rect::from_min_size(pos2(left, top + PLATE_GAP), size);
    paint_plate(painter, rect);
    let label_pos = pos2(
        rect.left() + PLATE_PADDING,
        rect.center().y - label.size().y / 2.0,
    );
    let label_width = label.size().x;
    painter.galley(label_pos, label, TICKS);

    let y = rect.center().y;
    let first = pos2(label_pos.x + label_width + gap * 1.5 + radius, y);
    let second = pos2(first.x + radius * 2.0 + gap, y);
    let outline = Stroke::new(1.2, SPACING);

    painter.circle_filled(first, radius, SPACING);
    match spacing {
        LineSpacing::Single => {}
        LineSpacing::OneAndHalf => {
            painter.add(Shape::convex_polygon(
                left_half_disc(second, radius),
                SPACING,
                Stroke::NONE,
            ));
        }
        LineSpacing::Double => {
            painter.circle_filled(second, radius, SPACING);
        }
    }
    painter.circle_stroke(second, radius, outline);
    rect
}

/// Right of `after`. Returns the plate's rect (so do the plates below).
pub fn paint_zoom_plate(painter: &Painter, percent: u16, after: Rect) -> Rect {
    paint_text_plate(painter, format!("{percent} %"), after)
}

/// The correction method, right of `after`.
pub fn paint_correction_plate(
    painter: &Painter,
    mode: EraseMode,
    slip_in: bool,
    after: Rect,
) -> Rect {
    let method = match mode {
        EraseMode::Off => "Off",
        EraseMode::Paper if slip_in => "Paper (slip in)",
        EraseMode::Paper => "Paper",
        EraseMode::Eraser => "Eraser",
        EraseMode::Fluid => "Fluid",
    };
    paint_text_plate(painter, format!("Correct: {method}"), after)
}

/// The session goal's progress, right of `after`.
pub fn paint_goal_plate(painter: &Painter, progress: Option<Progress>, after: Rect) -> Rect {
    paint_text_plate(painter, goal_text(progress), after)
}

fn goal_text(progress: Option<Progress>) -> String {
    let Some(Progress {
        goal,
        words,
        minutes,
        reached,
    }) = progress
    else {
        return "Goal: Off".to_owned();
    };
    let target = match goal {
        Goal::Words(n) => format!("{words} / {n} words"),
        Goal::Minutes(n) => format!("{minutes} / {n} min"),
        Goal::WordsOrMinutes {
            words: w,
            minutes: m,
        } => format!("{words} / {w} words or {minutes} / {m} min"),
    };
    let check = if reached { " \u{2714}" } else { "" };
    format!("Goal: {target}{check}")
}

/// Flush with the scale's right end (`right`), in `left_row`'s row; drops a
/// row if the left plates reach `row_end`.
pub fn paint_autosave_plate(
    painter: &Painter,
    keeping: &Keeping,
    right: f32,
    left_row: Rect,
    row_end: f32,
) -> Rect {
    let (text, dot) = autosave_label(keeping);
    let galley = painter.layout_no_wrap(
        text.to_owned(),
        FontId::proportional(PLATE_FONT_SIZE),
        TICKS,
    );
    let dot_room = if dot.is_some() {
        DOT_RADIUS * 2.0 + PLATE_PADDING
    } else {
        0.0
    };
    let size = vec2(
        galley.size().x + PLATE_PADDING * 2.0 + dot_room,
        left_row.height(),
    );
    let mut rect = Rect::from_min_size(pos2(right - size.x, left_row.top()), size);
    if rect.left() < row_end + PLATE_GAP {
        rect = rect.translate(vec2(0.0, left_row.height() + PLATE_GAP));
    }
    paint_plate(painter, rect);
    let text_left = rect.left() + PLATE_PADDING;
    painter.galley(
        pos2(text_left, rect.center().y - galley.size().y / 2.0),
        galley,
        TICKS,
    );
    if let Some(color) = dot {
        let centre = pos2(rect.right() - PLATE_PADDING - DOT_RADIUS, rect.center().y);
        painter.circle(centre, DOT_RADIUS, color, Stroke::new(0.8, SCALE_EDGE));
    }
    rect
}

fn autosave_label(keeping: &Keeping) -> (&'static str, Option<Color32>) {
    match keeping {
        Keeping::Autosave(WriteStatus::Saved) => ("Autosave: On", Some(SAVED)),
        Keeping::Autosave(WriteStatus::Writing) => ("Autosave: On", Some(WRITING)),
        Keeping::Autosave(WriteStatus::Failed(_)) => ("Autosave: On", Some(FAILED)),
        Keeping::Draft => ("Autosave: Draft", None),
        Keeping::Off { .. } => ("Autosave: Off", None),
    }
}

fn paint_text_plate(painter: &Painter, text: String, after: Rect) -> Rect {
    let text = painter.layout_no_wrap(text, FontId::proportional(PLATE_FONT_SIZE), TICKS);
    let size = vec2(text.size().x + PLATE_PADDING * 2.0, after.height());
    let rect = Rect::from_min_size(after.right_top() + vec2(PLATE_GAP, 0.0), size);
    paint_plate(painter, rect);
    painter.galley(rect.center() - text.size() / 2.0, text, TICKS);
    rect
}

fn paint_plate(painter: &Painter, rect: Rect) {
    painter.rect(
        rect,
        CornerRadius::same(2),
        SCALE,
        Stroke::new(1.0, SCALE_EDGE),
        eframe::egui::StrokeKind::Inside,
    );
}

fn left_half_disc(centre: Pos2, radius: f32) -> Vec<Pos2> {
    const STEPS: u16 = 16;
    (0..=STEPS)
        .map(|i| {
            // Top, round the left, to the bottom.
            let angle = PI / 2.0 + PI * f32::from(i) / f32::from(STEPS);
            pos2(
                centre.x + radius * angle.cos(),
                centre.y - radius * angle.sin(),
            )
        })
        .collect()
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use typewriter_core::Profile;

    use super::*;

    #[test]
    fn the_goal_plate_reads_progress() {
        assert_eq!(goal_text(None), "Goal: Off");
        let progress = |goal, reached| {
            Some(Progress {
                goal,
                words: 312,
                minutes: 12,
                reached,
            })
        };
        assert_eq!(
            goal_text(progress(Goal::Words(500), false)),
            "Goal: 312 / 500 words"
        );
        assert_eq!(
            goal_text(progress(Goal::Minutes(12), true)),
            "Goal: 12 / 12 min \u{2714}"
        );
        let either = Goal::WordsOrMinutes {
            words: 750,
            minutes: 30,
        };
        assert_eq!(
            goal_text(progress(either, false)),
            "Goal: 312 / 750 words or 12 / 30 min"
        );
    }

    #[test]
    fn the_autosave_plate_names_how_the_project_is_kept() {
        let failed = Keeping::Autosave(WriteStatus::Failed("disk full".into()));
        assert_eq!(autosave_label(&failed), ("Autosave: On", Some(FAILED)));
        assert_eq!(autosave_label(&Keeping::Draft).0, "Autosave: Draft");
        assert_eq!(
            autosave_label(&Keeping::Off { unsaved: true }),
            ("Autosave: Off", None)
        );
    }

    #[test]
    fn half_disc_covers_only_the_left_side() {
        let centre = pos2(10.0, 10.0);
        let points = left_half_disc(centre, 5.0);
        assert!(points.iter().all(|p| p.x <= centre.x + 1e-4));
        assert!((points[0].y - 5.0).abs() < 1e-4);
        assert!((points[points.len() - 1].y - 15.0).abs() < 1e-4);
    }

    #[test]
    fn a_stop_grips_where_it_stands_and_drags_by_whole_columns() {
        let profile =
            Profile::from_toml_str(include_str!("../../../../profiles/olympia-sm9.toml")).unwrap();
        let metrics = Metrics::new(&profile, 96.0);
        let columns = profile.columns();
        let scale = Scale::new(&metrics, columns, 100.0, 50.0);
        let carriage = Carriage::new(10, 72, 12);
        for (side, column) in [(Side::Left, 10), (Side::Right, 72)] {
            let grip = scale.stop(&carriage, side);
            let x = scale.column_left(column);
            assert!(grip.x_range().contains(x));
            assert_eq!(scale.column_at(grip.center().x), column);
        }
        assert_eq!(scale.column_at(-1000.0), 0);
        assert_eq!(scale.column_at(1e6), columns);
    }
}
