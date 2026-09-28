//! The sheet and everything struck or painted on it.

use eframe::egui::{Align2, Color32, CornerRadius, Painter, Pos2, Rect, Stroke, Vec2, pos2, vec2};
use typewriter_core::carriage::Carriage;
use typewriter_core::page::{Correction, Mark, Page};

use super::{MM_PER_INCH, Metrics};

pub const INK: Color32 = Color32::from_rgba_premultiplied(0x1C, 0x1A, 0x18, 0xEB);
const WHITE_OUT: Color32 = Color32::from_rgb(0xFB, 0xFA, 0xF5);
const CORRECTION_TAPE: Color32 = Color32::from_rgb(0xF8, 0xF6, 0xEF);
const FRAME: Color32 = Color32::from_rgba_premultiplied(0x4A, 0x46, 0x40, 0x8C);
const FRAME_EXTENSION: Color32 = Color32::from_rgba_premultiplied(0x25, 0x23, 0x20, 0x46);
/// Clearance between the margins and the frame, so type never touches it.
const FRAME_PADDING_MM: f32 = 1.0;
/// Ink realism: how far a strike may land off its cell (in points at 96 per
/// inch) and how much lighter it may print.
const INK_MAX_OFFSET: f32 = 0.4;
const INK_DENSITY_VARIANCE: f32 = 0.1;

/// The sheet has no fill of its own. Its extent is shown by a frame around
/// the writing area whose sides run out to the paper's edges. The profile
/// has no bottom margin (the SM9 types down to the last line), so the frame
/// runs to the bottom of the sheet.
pub fn paint_margin_frame(
    painter: &Painter,
    metrics: &Metrics,
    carriage: &Carriage,
    top_lines: u16,
    origin: Pos2,
) {
    let pad = FRAME_PADDING_MM / MM_PER_INCH * metrics.points_per_inch;
    let sheet = Rect::from_min_size(origin, metrics.paper_size);
    let top_left =
        origin + metrics.cell_offset(top_lines * 2, carriage.left_margin) - Vec2::splat(pad);
    let right = origin.x + metrics.cell_offset(0, carriage.right_margin).x + pad;
    let frame = Rect::from_min_max(top_left, pos2(right, sheet.bottom()));

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

/// `ink_realism` varies each strike slightly, like uneven key pressure and
/// type slugs that do not land exactly in place.
pub fn paint_sheet(
    painter: &Painter,
    metrics: &Metrics,
    page: &Page,
    origin: Pos2,
    ink_realism: bool,
) {
    let scale = metrics.points_per_inch / 96.0;
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
        for (index, mark) in cell.marks().iter().enumerate() {
            match mark {
                Mark::Glyph(c) => {
                    let (offset, density) = if ink_realism {
                        ink_variation(half_line, column, index)
                    } else {
                        (Vec2::ZERO, 1.0)
                    };
                    painter.text(
                        cell_rect.min + offset * scale,
                        Align2::LEFT_TOP,
                        c,
                        metrics.font.clone(),
                        INK.gamma_multiply(density),
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

/// Offset and density for one strike. Derived from its place on the page, so
/// it never changes between redraws.
fn ink_variation(half_line: u16, column: u16, index: usize) -> (Vec2, f32) {
    let seed = (u64::from(half_line) << 40) ^ (u64::from(column) << 20) ^ index as u64;
    let bits = splitmix64(seed);
    let unit = |shift: u32| ((bits >> shift) & 0xFFFF) as f32 / 65535.0;
    let offset = vec2(unit(0) - 0.5, unit(16) - 0.5) * (2.0 * INK_MAX_OFFSET);
    let density = 1.0 - INK_DENSITY_VARIANCE * unit(32);
    (offset, density)
}

/// A well-mixed hash, so neighbouring cells do not vary in step.
pub fn splitmix64(mut x: u64) -> u64 {
    x = x.wrapping_add(0x9E37_79B9_7F4A_7C15);
    x = (x ^ (x >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    x = (x ^ (x >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    x ^ (x >> 31)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ink_variation_is_stable_and_subtle() {
        assert_eq!(ink_variation(12, 10, 0), ink_variation(12, 10, 0));
        assert_ne!(ink_variation(12, 10, 0), ink_variation(12, 11, 0));
        for column in 0..200 {
            let (offset, density) = ink_variation(12, column, 0);
            assert!(offset.x.abs() <= INK_MAX_OFFSET && offset.y.abs() <= INK_MAX_OFFSET);
            assert!((1.0 - INK_DENSITY_VARIANCE..=1.0).contains(&density));
        }
    }
}
