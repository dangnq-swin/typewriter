//! Calm mode: the chrome fades away and the typed lines dim with their
//! distance from the line being typed.

use eframe::egui::{self, Color32, CornerRadius, CursorIcon, Id, Rect, Sense, Stroke, Ui, vec2};

use super::paper::INK;

/// Chrome and dimming fade in and out together over this, like the pointer
/// after a sheet feed.
pub const FADE_SECONDS: f32 = 0.25;
/// Strong falloff: only the line or two around the typing line stay legible.
const FALLOFF_LINES: f32 = 4.0;
const MIN_OPACITY: f32 = 0.1;

const ICON_SHEET: Color32 = Color32::from_rgb(0xF7, 0xF4, 0xEC);
const ICON_EDGE: Color32 = Color32::from_rgb(0xA8, 0xA0, 0x92);
const ICON_HIGHLIGHT: Color32 = Color32::from_rgb(0x80, 0x30, 0x20);

/// How much ink each line of a sheet keeps.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Dimming {
    /// The line being typed, in half-lines.
    pub half_line: u16,
    /// 0 with calm mode off, 1 with it on, in between while it fades.
    pub amount: f32,
}

impl Dimming {
    pub const NONE: Self = Self {
        half_line: 0,
        amount: 0.0,
    };

    /// Distance is measured on the sheet, so wider line spacing dims the
    /// previous line more.
    pub fn opacity(&self, half_line: u16) -> f32 {
        if self.amount <= 0.0 {
            return 1.0;
        }
        let lines = f32::from(half_line.abs_diff(self.half_line)) / 2.0;
        let t = (lines / FALLOFF_LINES).min(1.0);
        let eased = t * t * (3.0 - 2.0 * t);
        1.0 - self.amount * (1.0 - MIN_OPACITY) * eased
    }
}

/// A small sheet right of the folder icon whose lines fade away from the
/// middle one. It stays in calm mode, so clicking it turns calm mode on and
/// off.
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
        ICON_SHEET,
        Stroke::new(1.0, if hovered { ICON_HIGHLIGHT } else { ICON_EDGE }),
        egui::StrokeKind::Inside,
    );
    let lines = Dimming {
        half_line: 4,
        amount: 1.0,
    };
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

    #[test]
    fn lines_fade_strongly_with_distance() {
        let d = Dimming {
            half_line: 20,
            amount: 1.0,
        };
        assert_eq!(d.opacity(20), 1.0);
        // Equal on both sides, fading steadily, down to the minimum by
        // FALLOFF_LINES.
        assert_eq!(d.opacity(18), d.opacity(22));
        let fading: Vec<f32> = (0..=4).map(|lines| d.opacity(20 - lines * 2)).collect();
        assert!(fading.windows(2).all(|w| w[1] < w[0]), "{fading:?}");
        assert!(fading[1] > 0.8, "the previous line stays legible");
        assert!((fading[4] - MIN_OPACITY).abs() < 1e-6);
        assert!((d.opacity(0) - MIN_OPACITY).abs() < 1e-6);
    }

    #[test]
    fn fading_calm_mode_dims_part_way() {
        let half = Dimming {
            half_line: 20,
            amount: 0.5,
        };
        assert!((half.opacity(0) - (1.0 + MIN_OPACITY) / 2.0).abs() < 1e-6);
    }
}
