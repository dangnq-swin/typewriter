//! The scratchpad open, written in place, or sliding away.

use eframe::egui::{self, Rect};

use crate::app::TypewriterApp;
use crate::app::intent::Intent;
use crate::render::pad;

impl TypewriterApp {
    pub(super) fn show_scratchpad(
        &mut self,
        ui: &mut egui::Ui,
        view: Rect,
        intents: &mut Vec<Intent>,
    ) {
        let desk = &mut self.desk;
        let shown = ui.ctx().animate_bool_with_time(
            egui::Id::new("scratchpad"),
            desk.overlays.scratchpad.is_some(),
            pad::SLIDE_SECONDS,
        );
        if shown <= 0.0 {
            return;
        }
        let points_per_inch = desk.points_per_inch();
        let book = desk.project.machine.scratchpad_mut();
        let mut pad = pad::Pad::rising(view, points_per_inch, shown, book.spread());
        let mut changed = false;
        if let Some(writing) = &mut desk.overlays.scratchpad
            && let Some(step) = pad.turn_asked(ui)
            && writing.turn(book, step)
        {
            changed = true;
            pad = pad::Pad::rising(view, points_per_inch, shown, book.spread());
        }
        let painter = ui.painter_at(view);
        pad.paint(&painter, ui.input(|i| i.pointer.hover_pos()));
        match &mut desk.overlays.scratchpad {
            Some(writing) => {
                let outcome = pad::write(ui, &pad, book, writing);
                changed |= outcome.changed;
                if outcome.done {
                    intents.push(Intent::CloseScratchpad);
                }
            }
            None => pad.paint_text(&painter, book),
        }
        if changed {
            intents.push(Intent::ScratchpadWritten);
        }
    }
}
