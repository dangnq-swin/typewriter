//! Drawing the desk: the view that fills the window, what lies over it, and
//! the dialogs. What the user asks comes back as [`Intent`]s. Only egui's
//! own editors change the desk directly: text fields, the notebook, the
//! copy holder's guide and the settings card.

mod dialogs;
mod folder;
mod notebook;
mod typing;

use eframe::egui::{self, Rect};

use super::TypewriterApp;
use super::desk::View;
use super::intent::Intent;
use crate::render::{self, scrunch};

impl TypewriterApp {
    /// Draws the frame. Returns what the user asked, in order.
    pub(super) fn show(&mut self, ui: &mut egui::Ui, now: f64) -> Vec<Intent> {
        let mut intents = Vec::new();
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE)
            .show(ui, |ui| {
                let view = ui.max_rect();
                self.background.paint(&ui.painter_at(view), view);
                match self.desk.view {
                    View::Typing => {
                        self.show_typing(ui, now, &mut intents);
                        // Last: above the paper and the knob beside it,
                        // for clicks too.
                        self.show_holder(ui, view, &mut intents);
                    }
                    View::Folder => self.show_folder(ui, view, &mut intents),
                    View::Sheet(index) => self.show_sheet(ui, view, index, &mut intents),
                    View::Settings => self.show_settings(ui, view, &mut intents),
                }
                self.show_notebook(ui, view, &mut intents);
                if self.desk.view == View::Folder {
                    self.show_log(ui, view, &mut intents);
                }
                self.paint_scrunching(ui, now);
                self.desk.notice.paint(ui.ctx(), view, now);
            });
        let ctx = ui.ctx();
        intents.extend(dialogs::confirm_scrunch(ctx, &self.desk));
        intents.extend(dialogs::leaving(ctx, &self.desk));
        intents
    }

    /// The settings card, edited in place.
    fn show_settings(&mut self, ui: &mut egui::Ui, view: Rect, intents: &mut Vec<Intent>) {
        let desk = &mut self.desk;
        let before = desk.settings.clone();
        // No background row over a backdrop: the look's isn't used.
        let problem = self
            .stage
            .backdrop()
            .is_none()
            .then(|| self.background.problem());
        let card =
            render::settings::show_settings(ui, view, &mut desk.settings, &desk.machines, problem);
        if card.choose_texture {
            intents.push(Intent::ChooseTexture);
        }
        if render::settings::gear_icon(ui, view, 1.0) || card.close {
            intents.push(Intent::CloseSettings);
        }
        if desk.settings != before {
            intents.push(Intent::SettingsEdited(Box::new(before)));
        }
    }

    /// A scrunched sheet balling up where it lay.
    fn paint_scrunching(&self, ui: &egui::Ui, now: f64) {
        let Some(scrunching) = &self.scrunching else {
            return;
        };
        let t = now - scrunching.started;
        if t < scrunch::SECONDS {
            let painter = ui.ctx().layer_painter(egui::LayerId::new(
                egui::Order::Foreground,
                egui::Id::new("scrunch"),
            ));
            scrunch::paint(&painter, scrunching.outline, t, scrunching.seed);
        }
    }
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::rc::Rc;

    use eframe::egui::Painter;
    use typewriter_core::Command;

    use super::*;
    use crate::app::desk::testing::{press, type_text};
    use crate::app::desk::{Answer, Leaving};
    use crate::app::fonts;
    use crate::draw::{Controls, Metrics, Scene};
    use crate::input::Action;
    use crate::render::folder::FolderAction;
    use crate::render::ruler::Scale;
    use crate::stage::Stage;
    use typewriter_core::carriage::Carriage;

    /// One frame at `now`: what the user asked.
    fn frame(app: &mut TypewriterApp, ctx: &egui::Context, now: f64) -> Vec<Intent> {
        let mut intents = Vec::new();
        let mut output = ctx.run_ui(egui::RawInput::default(), |ui| {
            intents = app.show(ui, now);
        });
        output.textures_delta.clear();
        intents
    }

    #[test]
    fn every_view_draws_with_everything_open_over_it() {
        let ctx = egui::Context::default();
        fonts::install(&ctx);
        let mut app = TypewriterApp::for_tests(&ctx);
        let desk = &mut app.desk;
        type_text(desk, "a finished sheet", 10.0);
        press(desk, &[Action::Machine(Command::FeedSheet)], 20.0);
        type_text(desk, "on the next", 20.5);

        // Feeding, a sheet on the holder, the notebook open.
        desk.update(Intent::Folder(FolderAction::PutOnHolder), 21.0);
        desk.update(Intent::OpenNotebook, 21.0);
        assert!(app.desk.feed.feeding.is_some());
        frame(&mut app, &ctx, 21.0);

        let desk = &mut app.desk;
        desk.update(Intent::CloseNotebook, 30.0);
        desk.update(Intent::OpenFolder, 30.0);
        desk.update(Intent::Folder(FolderAction::Rename), 30.0);
        desk.update(Intent::OpenLog, 30.0);
        frame(&mut app, &ctx, 30.0);

        let desk = &mut app.desk;
        desk.overlays = Default::default();
        desk.update(Intent::OpenSheet(0), 31.0);
        desk.update(Intent::StartNote, 31.0);
        desk.overlays.confirm_scrunch = Some(0);
        frame(&mut app, &ctx, 31.0);

        let desk = &mut app.desk;
        desk.overlays = Default::default();
        desk.update(Intent::OpenSettings, 32.0);
        desk.leaving = Some(Leaving::New);
        frame(&mut app, &ctx, 32.0);
        app.desk.update(Intent::Leave(Answer::Cancel), 33.0);
        assert_eq!(app.desk.view, View::Settings);
    }

    /// A stage noting which hooks the typing view calls, in order, each
    /// answering as the plain app.
    struct Noting(Rc<RefCell<Vec<&'static str>>>);

    impl Noting {
        fn note(&self, hook: &'static str) {
            self.0.borrow_mut().push(hook);
        }
    }

    impl Stage for Noting {
        fn command(&self) -> &'static str {
            "typewriter-noting"
        }

        fn title(&self) -> &'static str {
            "Noting"
        }

        fn about(&self) -> &'static str {
            "notes the hooks"
        }

        fn typing_line_height(&self, _: Rect, _: &Metrics, _: u16) -> Option<f32> {
            self.note("typing line");
            None
        }

        fn paint_behind_sheets(&self, _: &Painter, _: &Scene) -> Option<f32> {
            self.note("behind the sheets");
            None
        }

        fn knobs(
            &self,
            _: &egui::Ui,
            _: &Painter,
            _: &Scene,
            _: f32,
            _: bool,
        ) -> Option<[Rect; 2]> {
            self.note("knobs");
            None
        }

        fn scale(&self, _: &Painter, _: &Scene, _: &Scale, _: &Carriage) -> Option<Scale> {
            self.note("scale");
            None
        }

        fn paint_over_sheets(&self, _: &Painter, _: &Scene) {
            self.note("over the sheets");
        }

        fn controls(
            &self,
            _: &egui::Ui,
            _: &Painter,
            _: &Scene,
            _: &Controls,
        ) -> Option<[Rect; 5]> {
            self.note("controls");
            None
        }
    }

    #[test]
    fn the_typing_view_calls_the_stage_back_to_front() {
        let ctx = egui::Context::default();
        fonts::install(&ctx);
        let mut app = TypewriterApp::for_tests(&ctx);
        let noted = Rc::new(RefCell::new(Vec::new()));
        app.stage = Box::new(Noting(Rc::clone(&noted)));
        frame(&mut app, &ctx, 10.0);
        assert_eq!(
            *noted.borrow(),
            [
                "typing line",
                "behind the sheets",
                "knobs",
                "scale",
                "over the sheets",
                "controls",
            ]
        );
    }
}
