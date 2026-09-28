//! The carriage scale below the typing line, with margin and tab stop marks,
//! the line spacing indicator, the zoom plate, the correction plate and the
//! goal plate.

use std::f32::consts::PI;

use eframe::egui::{
    Align2, Color32, CornerRadius, FontId, Painter, Pos2, Rect, Shape, Stroke, pos2, vec2,
};
use typewriter_core::carriage::Carriage;
use typewriter_core::session::Progress;
use typewriter_core::{EraseMode, Goal, LineSpacing};

use super::Metrics;

pub const HEIGHT: f32 = 20.0;
/// Space between the bottom of the typing line and the scale.
const GAP: f32 = 10.0;
/// Space between the scale and the plates below it, and between plates.
const PLATE_GAP: f32 = 4.0;
const PLATE_PADDING: f32 = 6.0;
const PLATE_FONT_SIZE: f32 = 10.0;

const SCALE: Color32 = Color32::from_rgba_premultiplied(0xDC, 0xD8, 0xCE, 0xEE);
const SCALE_EDGE: Color32 = Color32::from_rgb(0x9A, 0x94, 0x88);
const TICKS: Color32 = Color32::from_rgb(0x3A, 0x36, 0x32);
const MARGIN: Color32 = Color32::from_rgb(0xA0, 0x2C, 0x1C);
const TAB: Color32 = Color32::from_rgb(0x24, 0x4C, 0x7A);
const SPACING: Color32 = Color32::from_rgb(0x24, 0x22, 0x20);

/// Top edge of the scale for a given strike point.
pub fn top(metrics: &Metrics, strike_point: Pos2) -> f32 {
    strike_point.y + metrics.cell_size().y + GAP
}

/// The scale travels with the carriage, so it is laid out from the paper's
/// left edge, like the one on the SM9's paper bail.
pub fn paint_scale(
    painter: &Painter,
    metrics: &Metrics,
    carriage: &Carriage,
    columns: u16,
    paper_left: f32,
    top: f32,
) {
    let rect = Rect::from_min_size(pos2(paper_left, top), vec2(metrics.paper_size.x, HEIGHT));
    painter.rect(
        rect,
        CornerRadius::same(2),
        SCALE,
        Stroke::new(1.0, SCALE_EDGE),
        eframe::egui::StrokeKind::Inside,
    );

    let column_left =
        |column: u16| paper_left + metrics.grid_origin.x + f32::from(column) * metrics.column_width;
    let column_centre = |column: u16| column_left(column) + metrics.column_width / 2.0;
    let bottom = rect.bottom() - 1.0;
    let label_font = FontId::proportional(8.0);

    for column in 0..columns {
        let x = column_centre(column);
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
        paint_tab_mark(painter, column_centre(stop), rect);
    }
    paint_margin_mark(painter, column_left(carriage.left_margin), rect, 1.0);
    paint_margin_mark(painter, column_left(carriage.right_margin), rect, -1.0);
}

/// A bracket opening towards the writing area: `direction` is 1 for the left
/// margin and -1 for the right.
fn paint_margin_mark(painter: &Painter, x: f32, rect: Rect, direction: f32) {
    let stroke = Stroke::new(2.0, MARGIN);
    let foot = 4.0 * direction;
    painter.add(Shape::line(
        vec![
            pos2(x + foot, rect.top() + 1.0),
            pos2(x, rect.top() + 1.0),
            pos2(x, rect.bottom() - 1.0),
            pos2(x + foot, rect.bottom() - 1.0),
        ],
        stroke,
    ));
}

/// A small downward pointer hanging from the top of the scale.
fn paint_tab_mark(painter: &Painter, x: f32, rect: Rect) {
    let w = 3.5;
    let top = rect.top() + 1.0;
    painter.add(Shape::convex_polygon(
        vec![pos2(x - w, top), pos2(x + w, top), pos2(x, top + w * 1.6)],
        TAB,
        Stroke::NONE,
    ));
}

/// Labelled plate just below the scale, flush with its left end. Two
/// circles: the first is always filled, the second is empty for single
/// spacing, filled on its left half for 1.5 and full for double.
/// Returns the plate's outline, which can be clicked to change the spacing.
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

/// The zoom level on a plate right of `after` (the spacing plate). Returns
/// the plate's outline.
pub fn paint_zoom_plate(painter: &Painter, percent: u16, after: Rect) -> Rect {
    paint_text_plate(painter, format!("{percent} %"), after)
}

/// How mistakes are fixed, on a plate right of `after` (the zoom plate).
/// Returns the plate's outline.
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

/// The session goal and how far along it is, on a plate right of `after`
/// (the correction plate). Returns the plate's outline.
pub fn paint_goal_plate(painter: &Painter, progress: Option<Progress>, after: Rect) -> Rect {
    paint_text_plate(painter, goal_text(progress), after)
}

fn goal_text(progress: Option<Progress>) -> String {
    let Some(Progress {
        goal,
        done,
        reached,
    }) = progress
    else {
        return "Goal: Off".to_owned();
    };
    let (target, unit) = match goal {
        Goal::Words(n) => (n, "words"),
        Goal::Minutes(n) => (n, "min"),
    };
    let check = if reached { " \u{2714}" } else { "" };
    format!("Goal: {done} / {target} {unit}{check}")
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
            // From the top, round the left side, to the bottom.
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
    use super::*;

    #[test]
    fn the_goal_plate_reads_progress() {
        assert_eq!(goal_text(None), "Goal: Off");
        let progress = |goal, done, reached| {
            Some(Progress {
                goal,
                done,
                reached,
            })
        };
        assert_eq!(
            goal_text(progress(Goal::Words(500), 312, false)),
            "Goal: 312 / 500 words"
        );
        assert_eq!(
            goal_text(progress(Goal::Minutes(25), 25, true)),
            "Goal: 25 / 25 min \u{2714}"
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
}
