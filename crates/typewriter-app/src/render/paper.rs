//! The sheet and everything struck or painted on it.

use eframe::egui::{Align2, Color32, CornerRadius, Painter, Pos2, Rect, Shadow, Vec2};
use typewriter_core::page::{Correction, Mark, Page};

use super::Metrics;

// Flat fallback until the maintainer-supplied paper texture is added (M3).
const PAPER: Color32 = Color32::from_rgb(0xF4, 0xF0, 0xE6);
const INK: Color32 = Color32::from_rgba_premultiplied(0x1C, 0x1A, 0x18, 0xEB);
const WHITE_OUT: Color32 = Color32::from_rgb(0xFB, 0xFA, 0xF5);
const CORRECTION_TAPE: Color32 = Color32::from_rgb(0xF8, 0xF6, 0xEF);

pub fn paint_sheet(painter: &Painter, metrics: &Metrics, page: &Page, origin: Pos2) {
    let sheet = Rect::from_min_size(origin, metrics.paper_size);
    let shadow = Shadow {
        offset: [0, 3],
        blur: 14,
        spread: 0,
        color: Color32::from_black_alpha(90),
    };
    painter.add(shadow.as_shape(sheet, CornerRadius::ZERO));
    painter.rect_filled(sheet, CornerRadius::ZERO, PAPER);

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
