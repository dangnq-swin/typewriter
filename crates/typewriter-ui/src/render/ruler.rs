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
/// Ticks up from the scale's foot, the tens' through the band.
const SHORT_TICK: f32 = 3.0;
const FIVE_TICK: f32 = 4.0;
const LABEL_SIZE: f32 = 8.0;
/// Either side of a ten's tick, to its number's digits.
const DIGIT_GAP: f32 = 1.5;
/// The middle, as on the SM9: a ▼ for its number, half this wide.
const CENTRE_COLUMN: u16 = 40;
const CENTRE_HALF: f32 = 3.0;
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
pub const SAVED: Color32 = Color32::from_rgb(0x3C, 0x8A, 0x3C);
const WRITING: Color32 = Color32::from_rgb(0xD0, 0x90, 0x20);
const FAILED: Color32 = Color32::from_rgb(0xB0, 0x30, 0x1C);
const DOT_RADIUS: f32 = 3.5;
/// The reached goal's check mark, drawn.
const CHECK_WIDTH: f32 = 9.0;
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

    /// The same scale printed nearer or farther: stretched `by` across about
    /// `x`, its band from `top`, `height` high.
    pub fn stretched(self, x: f32, by: f32, top: f32, height: f32) -> Self {
        let across = |at: f32| x + (at - x) * by;
        Self {
            rect: Rect::from_x_y_ranges(
                across(self.rect.left())..=across(self.rect.right()),
                top..=top + height,
            ),
            grid_left: across(self.grid_left),
            column_width: self.column_width * by,
            columns: self.columns,
        }
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
    paint_plate(painter, scale.rect);
    painter.extend(scale_marks(painter, scale, carriage));
}

/// The scale's ticks, numbers and stops, without its plate: for printing
/// on something else, such as the machine's paper bail. Sized to the
/// scale's band, [`HEIGHT`] as on the plate; `painter` lays out the numbers.
pub fn scale_marks(painter: &Painter, scale: &Scale, carriage: &Carriage) -> Vec<Shape> {
    let rect = scale.rect;
    let k = rect.height() / HEIGHT;
    let (top, bottom, middle) = (rect.top() + k, rect.bottom() - k, rect.center().y);
    let stroke = Stroke::new(k, TICKS);
    let font = FontId::proportional(LABEL_SIZE * k);
    let mut shapes = Vec::new();
    for column in 0..scale.columns {
        let x = scale.column_centre(column);
        let tick = |from: f32| Shape::line_segment([pos2(x, from), pos2(x, bottom)], stroke);
        if column == CENTRE_COLUMN {
            shapes.push(tick(bottom - SHORT_TICK * k));
            let half = CENTRE_HALF * k;
            shapes.push(Shape::convex_polygon(
                vec![
                    pos2(x - half, middle - half),
                    pos2(x + half, middle - half),
                    pos2(x, middle + half),
                ],
                TICKS,
                Stroke::NONE,
            ));
        } else if column % 10 == 0 {
            // Through the number, between its last digit and the rest.
            shapes.push(tick(top));
            let digits = column.to_string();
            let (rest, last) = digits.split_at(digits.len() - 1);
            for (text, anchor, at) in [
                (rest, Align2::RIGHT_CENTER, x - DIGIT_GAP * k),
                (last, Align2::LEFT_CENTER, x + DIGIT_GAP * k),
            ] {
                let galley = painter.layout_no_wrap(text.to_owned(), font.clone(), TICKS);
                let at = anchor.anchor_size(pos2(at, middle), galley.size()).min;
                shapes.push(Shape::galley(at, galley, TICKS));
            }
        } else {
            let length = if column % 5 == 0 {
                FIVE_TICK
            } else {
                SHORT_TICK
            };
            shapes.push(tick(bottom - length * k));
        }
    }
    for stop in carriage.tab_stops() {
        shapes.push(tab_mark(scale.column_centre(stop), rect, k));
    }
    let released = carriage.margin_released;
    let left = scale.column_left(carriage.left_margin);
    let right = scale.column_left(carriage.right_margin);
    shapes.push(margin_mark(left, rect, STOP_FOOT * k, released, k));
    shapes.push(margin_mark(right, rect, -STOP_FOOT * k, released, k));
    shapes
}

/// A bracket opening toward the writing area, `foot` long (negative: the
/// right stop), sized `k` times the plate's. Released, it stands lifted and
/// faded until the return.
fn margin_mark(x: f32, rect: Rect, foot: f32, released: bool, k: f32) -> Shape {
    let (color, lift) = if released {
        (MARGIN.gamma_multiply(RELEASED_OPACITY), RELEASED_LIFT * k)
    } else {
        (MARGIN, 0.0)
    };
    let (top, bottom) = (rect.top() + k - lift, rect.bottom() - k - lift);
    Shape::line(
        vec![
            pos2(x + foot, top),
            pos2(x, top),
            pos2(x, bottom),
            pos2(x + foot, bottom),
        ],
        Stroke::new(2.0 * k, color),
    )
}

/// A small pointer hanging from the scale's top, sized `k` times the
/// plate's.
fn tab_mark(x: f32, rect: Rect, k: f32) -> Shape {
    let w = 3.5 * k;
    let top = rect.top() + k;
    Shape::convex_polygon(
        vec![pos2(x - w, top), pos2(x + w, top), pos2(x, top + w * 1.6)],
        TAB,
        Stroke::NONE,
    )
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
    let method = correction_method(mode, slip_in);
    paint_text_plate(painter, format!("Correct: {method}"), after)
}

pub fn correction_method(mode: EraseMode, slip_in: bool) -> &'static str {
    match mode {
        EraseMode::Delete => "Delete",
        EraseMode::Paper if slip_in => "Paper (slip in)",
        EraseMode::Paper => "Paper",
        EraseMode::Eraser => "Eraser",
        EraseMode::Fluid => "Fluid",
    }
}

/// The session goal's progress, right of `after`.
/// A reached goal gets a check mark, drawn: the plate font has no ✔.
pub fn paint_goal_plate(painter: &Painter, progress: Option<Progress>, after: Rect) -> Rect {
    let reached = progress.is_some_and(|p| p.reached);
    let text = painter.layout_no_wrap(
        goal_text(progress),
        FontId::proportional(PLATE_FONT_SIZE),
        TICKS,
    );
    let check = if reached {
        CHECK_WIDTH + PLATE_PADDING / 2.0
    } else {
        0.0
    };
    let size = vec2(text.size().x + PLATE_PADDING * 2.0 + check, after.height());
    let rect = Rect::from_min_size(after.right_top() + vec2(PLATE_GAP, 0.0), size);
    paint_plate(painter, rect);
    let text_left = rect.left() + PLATE_PADDING;
    let text_right = text_left + text.size().x;
    painter.galley(
        pos2(text_left, rect.center().y - text.size().y / 2.0),
        text,
        TICKS,
    );
    if reached {
        let h = 0.55 * rect.height();
        let at = |x: f32, y: f32| {
            pos2(
                text_right + PLATE_PADDING / 2.0 + x * CHECK_WIDTH,
                rect.center().y + (y - 0.5) * h,
            )
        };
        painter.add(Shape::line(
            vec![at(0.0, 0.55), at(0.35, 0.9), at(1.0, 0.1)],
            Stroke::new(1.4, SAVED),
        ));
    }
    rect
}

fn goal_text(progress: Option<Progress>) -> String {
    format!("Goal: {}", goal_reading(progress))
}

/// Progress toward the goal, or "Off".
pub fn goal_reading(progress: Option<Progress>) -> String {
    let Some(Progress {
        goal,
        words,
        minutes,
        ..
    }) = progress
    else {
        return "Off".to_owned();
    };
    match goal {
        Goal::Words(n) => format!("{words} / {n} words"),
        Goal::Minutes(n) => format!("{minutes} / {n} min"),
        Goal::WordsOrMinutes {
            words: w,
            minutes: m,
        } => format!("{words} / {w} words or {minutes} / {m} min"),
    }
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
    let galley = painter.layout_no_wrap(text, FontId::proportional(PLATE_FONT_SIZE), TICKS);
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

fn autosave_label(keeping: &Keeping) -> (String, Option<Color32>) {
    let (state, lamp) = autosave_state(keeping);
    (format!("Autosave: {state}"), lamp)
}

/// On, Draft or Off, and the lamp's colour if it is lit.
pub fn autosave_state(keeping: &Keeping) -> (&'static str, Option<Color32>) {
    match keeping {
        Keeping::Autosave(WriteStatus::Saved) => ("On", Some(SAVED)),
        Keeping::Autosave(WriteStatus::Writing) => ("On", Some(WRITING)),
        Keeping::Autosave(WriteStatus::Failed(_)) => ("On", Some(FAILED)),
        Keeping::Draft => ("Draft", None),
        Keeping::Off { .. } => ("Off", None),
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
            "Goal: 12 / 12 min"
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
        assert_eq!(
            autosave_label(&failed),
            ("Autosave: On".to_owned(), Some(FAILED))
        );
        assert_eq!(autosave_label(&Keeping::Draft).0, "Autosave: Draft");
        assert_eq!(
            autosave_label(&Keeping::Off { unsaved: true }),
            ("Autosave: Off".to_owned(), None)
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
