//! The folder view, an open sheet, and the writing log up close.

use eframe::egui::{self, Rect};

use crate::app::TypewriterApp;
use crate::app::intent::Intent;
use crate::render::folder::{self, ProjectLabel};
use crate::render::{self, calendar};

impl TypewriterApp {
    pub(super) fn show_folder(&mut self, ui: &mut egui::Ui, view: Rect, intents: &mut Vec<Intent>) {
        let log = self.model.log_month();
        let model = &mut self.model;
        let project = &model.project;
        let (name, location) = (project.filing.name(), project.filing.location());
        let stats = calendar::log_line(project.machine.log());
        let contents = folder::Folder {
            sheets: project.machine.document().finished(),
            metrics: &self.metrics,
            margins: &project.machine.profile().margins,
            selected: model.selected,
            project: ProjectLabel {
                name: &name,
                location: &location,
                saved: project.filing.is_saved(),
                stats: &stats,
            },
            log: &log,
            covered: model.overlays.log_open,
        };
        let overlays = &mut model.overlays;
        let response = folder::show_folder(
            ui,
            view,
            &contents,
            overlays.renaming.as_mut(),
            overlays.renumbering.as_mut(),
        );
        self.pulled = response.pulled;
        if response.open_notebook {
            intents.push(Intent::OpenNotebook);
        }
        if response.open_log {
            intents.push(Intent::OpenLog);
        }
        if let Some(index) = response.opened {
            intents.push(Intent::OpenSheet(index));
        }
        intents.extend(response.action.map(Intent::Folder));
    }

    /// Finished sheet `index`, open. The model sends a sheet no longer there
    /// back to the folder.
    pub(super) fn show_sheet(
        &mut self,
        ui: &mut egui::Ui,
        view: Rect,
        index: usize,
        intents: &mut Vec<Intent>,
    ) {
        let model = &mut self.model;
        let machine = &model.project.machine;
        let sheets = machine.document().finished();
        let Some(page) = sheets.get(index) else {
            return;
        };
        let open = folder::OpenSheet {
            profile: machine.profile(),
            carriage: machine.carriage(),
            page,
            index,
            total: sheets.len(),
            max_points_per_inch: render::points_per_inch(model.zoom_percent),
            ink_realism: model.settings.look.ink_realism,
        };
        let response = folder::show_sheet(
            ui,
            view,
            &open,
            model.overlays.annotating.as_mut(),
            &mut self.print_open.borrow_mut(),
        );
        if response.start_note {
            intents.push(Intent::StartNote);
        }
        if let Some(note) = response.note_written {
            intents.push(Intent::NoteWritten { sheet: index, note });
        }
    }

    /// The writing log up close, or sliding away.
    pub(super) fn show_log(&mut self, ui: &mut egui::Ui, view: Rect, intents: &mut Vec<Intent>) {
        let open = self.model.overlays.log_open;
        let shown = ui.ctx().animate_bool_with_time(
            egui::Id::new("writing-log-up-close"),
            open,
            calendar::SLIDE_SECONDS,
        );
        if shown <= 0.0 {
            return;
        }
        let asked = calendar::show_up_close(ui, view, shown, &self.model.log_month(), open);
        intents.extend(asked.turn.map(Intent::TurnLog));
        if asked.close {
            intents.push(Intent::CloseLog);
        }
    }
}
