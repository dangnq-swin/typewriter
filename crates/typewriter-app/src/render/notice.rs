//! A short note at the bottom of the window, fading out: saves, failures,
//! jammed typebars.

use eframe::egui::{Color32, Context, CornerRadius, FontId, Id, LayerId, Order, Rect, vec2};

use super::LABEL;

const SECONDS: f64 = 4.0;
const FADE_SECONDS: f64 = 0.5;

#[derive(Debug, Default)]
pub struct Notice {
    /// What it says, and since when.
    shown: Option<(String, f64)>,
}

impl Notice {
    pub fn show(&mut self, text: impl Into<String>, now: f64) {
        self.shown = Some((text.into(), now));
    }

    /// Takes it down early, if it still says `text`.
    pub fn withdraw(&mut self, text: &str) {
        if self.shown.as_ref().is_some_and(|(shown, _)| shown == text) {
            self.shown = None;
        }
    }

    pub fn is_animating(&self, now: f64) -> bool {
        self.shown
            .as_ref()
            .is_some_and(|(_, at)| now - at < SECONDS)
    }

    pub fn paint(&self, ctx: &Context, view: Rect, now: f64) {
        let Some((text, at)) = &self.shown else {
            return;
        };
        let age = now - at;
        if age >= SECONDS {
            return;
        }
        let opacity = ((SECONDS - age) / FADE_SECONDS).min(1.0) as f32;
        let painter = ctx.layer_painter(LayerId::new(Order::Foreground, Id::new("notice")));
        let galley = painter.layout(
            text.clone(),
            FontId::proportional(13.0),
            LABEL.gamma_multiply(opacity),
            view.width() - 80.0,
        );
        let size = galley.size() + vec2(24.0, 14.0);
        let rect =
            Rect::from_center_size(view.center_bottom() - vec2(0.0, 90.0 + size.y / 2.0), size);
        painter.rect_filled(
            rect,
            CornerRadius::same(4),
            Color32::from_rgba_unmultiplied(0x2A, 0x26, 0x22, 0xE0).gamma_multiply(opacity),
        );
        painter.galley(rect.min + vec2(12.0, 7.0), galley, Color32::WHITE);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_notice_is_withdrawn_only_while_it_still_says_so() {
        let mut notice = Notice::default();
        notice.show("Jammed", 0.0);
        notice.withdraw("Saved");
        assert!(notice.shown.is_some());
        notice.withdraw("Jammed");
        assert!(notice.shown.is_none());
    }
}
