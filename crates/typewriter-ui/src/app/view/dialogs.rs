//! The questions: scrunching up a sheet, and leaving unsaved work.

use eframe::egui;

use crate::app::intent::Intent;
use crate::app::model::{Answer, Model};

/// The "Scrunch up sheet N?" modal, until answered.
pub fn confirm_scrunch(ctx: &egui::Context, model: &Model) -> Option<Intent> {
    let index = model.overlays.confirm_scrunch?;
    let modal = egui::Modal::new(egui::Id::new("confirm-scrunch")).show(ctx, |ui| {
        ui.set_width(300.0);
        ui.heading(format!("Scrunch up sheet {}?", index + 1));
        ui.label("It can't be smoothed out again.");
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            let scrunch = ui.button("Scrunch up").clicked();
            let keep = ui.button("Keep it").clicked();
            (scrunch, keep)
        })
        .inner
    });
    let (scrunch, keep) = modal.inner;
    if scrunch {
        Some(Intent::ScrunchUp(index))
    } else if keep || modal.should_close() {
        Some(Intent::KeepSheet)
    } else {
        None
    }
}

/// The leave dialog: what to do with a draft or unsaved changes.
pub fn leaving(ctx: &egui::Context, model: &Model) -> Option<Intent> {
    model.leaving.as_ref()?;
    let filing = &model.project.filing;
    let draft = !filing.is_saved();
    let name = filing.name();
    let modal = egui::Modal::new(egui::Id::new("leaving")).show(ctx, |ui| {
        ui.set_width(340.0);
        if draft {
            ui.heading("Keep this draft?");
            ui.label("It has not been saved under a name.");
        } else {
            ui.heading(format!("Save changes to \u{201c}{name}\u{201d}?"));
        }
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            let buttons: &[(&str, Answer)] = if draft {
                &[
                    ("Save As\u{2026}", Answer::SaveAs),
                    ("Keep as draft", Answer::Keep),
                    ("Discard", Answer::Discard),
                    ("Cancel", Answer::Cancel),
                ]
            } else {
                &[
                    ("Save", Answer::Save),
                    ("Don't save", Answer::DontSave),
                    ("Cancel", Answer::Cancel),
                ]
            };
            let mut chosen = None;
            for &(label, answer) in buttons {
                if ui.button(label).clicked() {
                    chosen = Some(answer);
                }
            }
            chosen
        })
        .inner
    });
    let answer = modal
        .inner
        .or_else(|| modal.should_close().then_some(Answer::Cancel));
    answer.map(Intent::Leave)
}
