//! The sheet and everything struck or painted on it.

use eframe::egui::{Align2, Color32, CornerRadius, Painter, Pos2, Rect, Stroke, Vec2};
use typewriter_core::carriage::Carriage;
use typewriter_core::page::{Correction, Mark, Page};

use super::{MM_PER_INCH, Metrics};

const INK: Color32 = Color32::from_rgba_premultiplied(0x1C, 0x1A, 0x18, 0xEB);
const WHITE_OUT: Color32 = Color32::from_rgb(0xFB, 0xFA, 0xF5);
const CORRECTION_TAPE: Color32 = Color32::from_rgb(0xF8, 0xF6, 0xEF);
const FRAME: Color32 = Color32::from_rgba_premultiplied(0x4A, 0x46, 0x40, 0x8C);
const FRAME_EXTENSION: Color32 = Color32::from_rgba_premultiplied(0x25, 0x23, 0x20, 0x46);
/// Clearance between the margins and the frame, so type never touches it.
const FRAME_PADDING_MM: f32 = 1.0;

/// The sheet has no fill of its own. Its extent is shown by a frame around
/// the writing area whose sides run out to the paper's edges. The profile
/// has no bottom margin (the SM9 types to the edge), so the frame mirrors
/// the top margin there.
pub fn paint_margin_frame(
    painter: &Painter,
    metrics: &Metrics,
    carriage: &Carriage,
    top_lines: u16,
    page: &Page,
    origin: Pos2,
) {
    let pad = FRAME_PADDING_MM / MM_PER_INCH * metrics.points_per_inch;
    let top_half_line = top_lines * 2;
    let bottom_half_line = page.half_lines().saturating_sub(top_half_line);
    let top_left = metrics.cell_offset(top_half_line, carriage.left_margin);
    let bottom_right = metrics.cell_offset(bottom_half_line, carriage.right_margin);
    let frame = Rect::from_min_max(origin + top_left, origin + bottom_right).expand(pad);
    let sheet = Rect::from_min_size(origin, metrics.paper_size);

    let extension = Stroke::new(1.0, FRAME_EXTENSION);
    for y in [frame.top(), frame.bottom()] {
        painter.hline(sheet.x_range(), y, extension);
    }
    for x in [frame.left(), frame.right()] {
        painter.vline(x, sheet.y_range(), extension);
    }
    painter.rect_stroke(
        frame,
        CornerRadius::ZERO,
        Stroke::new(1.0, FRAME),
        eframe::egui::StrokeKind::Middle,
    );
}

pub fn paint_sheet(painter: &Painter, metrics: &Metrics, page: &Page, origin: Pos2) {
    let clip = painter.clip_rect();
    for ((half_line, column), cell) in page.cells() {
        let cell_rect = Rect::from_min_size(
            origin + metrics.cell_offset(half_line, column),
            metrics.cell_size(),
        );
        if !clip.intersects(cell_rect) {
            continue;
        }
        // Marks are painted in the order they were made, so an opaque
        // correction hides whatever was struck before it.
        for mark in cell.marks() {
            match mark {
                Mark::Glyph(c) => {
                    painter.text(
                        cell_rect.min,
                        Align2::LEFT_TOP,
                        c,
                        metrics.font.clone(),
                        INK,
                    );
                }
                Mark::Correction(kind) => {
                    let color = match kind {
                        Correction::WhiteOut => WHITE_OUT,
                        Correction::CorrectionTape => CORRECTION_TAPE,
                    };
                    let patch = cell_rect.expand2(Vec2::new(0.5, -1.0));
                    painter.rect_filled(patch, CornerRadius::same(1), color);
                }
            }
        }
    }
}
