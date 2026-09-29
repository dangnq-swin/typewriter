//! Calm mode: chrome fades out, lines dim with distance from the typing line.

use eframe::egui::{self, CornerRadius, CursorIcon, Id, Rect, Sense, Stroke, Ui, vec2};

use super::paper::INK;
use super::{HIGHLIGHT, SHEET, SHEET_EDGE, smoothstep};

/// Chrome and dimming fade together over this.
pub const FADE_SECONDS: f32 = 0.25;
/// Strong by default: only a line or two around the typing line stay legible.
pub const DEFAULT_FALLOFF_LINES: u8 = 4;
pub const DEFAULT_MINIMUM_PERCENT: u8 = 10;

/// How much ink each line keeps.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Dimming {
    /// The typing line, in half-lines.
    pub half_line: u16,
    /// 0 off, 1 on, between while fading.
    pub amount: f32,
    /// Lines away where ink is faintest.
    pub falloff_lines: f32,
    /// Faintest ink, 0..=1.
    pub minimum: f32,
}

impl Dimming {
    pub const NONE: Self = Self {
        half_line: 0,
        amount: 0.0,
        falloff_lines: 1.0,
        minimum: 1.0,
    };

    /// Dimming around `half_line`, `amount` of the way on.
    pub fn calm(half_line: u16, amount: f32, falloff_lines: u8, minimum_percent: u8) -> Self {
        Self {
            half_line,
            amount,
            falloff_lines: f32::from(falloff_lines.max(1)),
            minimum: f32::from(minimum_percent.min(100)) / 100.0,
        }
    }

    /// Measured on the sheet: wider spacing dims the previous line more.
    pub fn opacity(&self, half_line: u16) -> f32 {
        if self.amount <= 0.0 {
            return 1.0;
        }
        let lines = f32::from(half_line.abs_diff(self.half_line)) / 2.0;
        let eased = smoothstep(lines / self.falloff_lines);
        1.0 - self.amount * (1.0 - self.minimum) * eased
    }
}

/// A small sheet whose lines fade from the middle, right of the folder
/// icon. Stays shown in calm mode: it is the way out.
pub fn calm_icon(ui: &mut Ui, view: Rect) -> bool {
    let sheet = Rect::from_min_size(view.left_bottom() + vec2(76.0, -46.0), vec2(23.0, 30.0));
    let response = ui
        .interact(sheet, Id::new("calm-icon"), Sense::click())
        .on_hover_text("Calm mode (Esc)");
    let hovered = response.hovered();
    let painter = ui.painter_at(view);
    painter.rect(
        sheet,
        CornerRadius::same(2),
        SHEET,
        Stroke::new(1.0, if hovered { HIGHLIGHT } else { SHEET_EDGE }),
        egui::StrokeKind::Inside,
    );
    let lines = Dimming::calm(4, 1.0, DEFAULT_FALLOFF_LINES, DEFAULT_MINIMUM_PERCENT);
    for line in 0..5_u16 {
        let y = sheet.top() + 7.0 + f32::from(line) * 4.0;
        painter.hline(
            sheet.left() + 5.0..=sheet.right() - 5.0,
            y,
            Stroke::new(1.5, INK.gamma_multiply(lines.opacity(line * 2))),
        );
    }
    if hovered {
        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
    }
    response.clicked()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn off_keeps_every_line_at_full_ink() {
        for half_line in [0, 10, 139] {
            assert_eq!(Dimming::NONE.opacity(half_line), 1.0);
        }
    }

    const MIN_OPACITY: f32 = DEFAULT_MINIMUM_PERCENT as f32 / 100.0;

    fn calm(half_line: u16, amount: f32) -> Dimming {
        Dimming::calm(
            half_line,
            amount,
            DEFAULT_FALLOFF_LINES,
            DEFAULT_MINIMUM_PERCENT,
        )
    }

    #[test]
    fn lines_fade_strongly_with_distance() {
        let d = calm(20, 1.0);
        assert_eq!(d.opacity(20), 1.0);
        // Symmetric, steadily fading, at the minimum by the falloff.
        assert_eq!(d.opacity(18), d.opacity(22));
        let fading: Vec<f32> = (0..=4).map(|lines| d.opacity(20 - lines * 2)).collect();
        assert!(fading.windows(2).all(|w| w[1] < w[0]), "{fading:?}");
        assert!(fading[1] > 0.8, "the previous line stays legible");
        assert!((fading[4] - MIN_OPACITY).abs() < 1e-6);
        assert!((d.opacity(0) - MIN_OPACITY).abs() < 1e-6);
    }

    #[test]
    fn fading_calm_mode_dims_part_way() {
        let half = calm(20, 0.5);
        assert!((half.opacity(0) - (1.0 + MIN_OPACITY) / 2.0).abs() < 1e-6);
    }

    #[test]
    fn a_gentler_setting_keeps_more_ink() {
        let gentle = Dimming::calm(20, 1.0, 8, 40);
        assert!(gentle.opacity(12) > calm(20, 1.0).opacity(12));
        assert!((gentle.opacity(0) - 0.4).abs() < 1e-6);
    }
}
