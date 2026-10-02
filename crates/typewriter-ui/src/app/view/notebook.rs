//! The notebook open, written in place, or sliding away.

use eframe::egui::{self, Rect};

use crate::app::TypewriterApp;
use crate::app::intent::Intent;
use crate::render::pad;

impl TypewriterApp {
    pub(super) fn show_notebook(
        &mut self,
        ui: &mut egui::Ui,
        view: Rect,
        intents: &mut Vec<Intent>,
    ) {
        let model = &mut self.model;
        let shown = ui.ctx().animate_bool_with_time(
            egui::Id::new("notebook"),
            model.overlays.notebook.is_some(),
            pad::SLIDE_SECONDS,
        );
        if shown <= 0.0 {
            return;
        }
        let points_per_inch = model.points_per_inch();
        let book = model.project.machine.notebook_mut();
        let mut pad = pad::Pad::rising(view, points_per_inch, shown, book.spread());
        let mut changed = false;
        if let Some(writing) = &mut model.overlays.notebook
            && let Some(step) = pad.turn_asked(ui)
            && writing.turn(book, step)
        {
            changed = true;
            pad = pad::Pad::rising(view, points_per_inch, shown, book.spread());
        }
        let painter = ui.painter_at(view);
        pad.paint(&painter, ui.input(|i| i.pointer.hover_pos()));
        match &mut model.overlays.notebook {
            Some(writing) => {
                let outcome = pad::write(ui, &pad, book, writing);
                changed |= outcome.changed;
                if outcome.done {
                    intents.push(Intent::CloseNotebook);
                }
            }
            None => pad.paint_text(&painter, book),
        }
        if changed {
            intents.push(Intent::NotebookWritten);
        }
    }
}
