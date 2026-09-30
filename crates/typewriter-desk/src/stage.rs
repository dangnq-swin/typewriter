//! The desk edition's hooks into the app: the room behind, the SM9 around
//! the sheet, its front panel for controls, the scale on its bail, the knobs
//! and return lever on its carriage.

use eframe::egui::{Painter, Rect, Ui};
use typewriter_app::Stage;
use typewriter_app::draw::{Controls, Metrics, Return, Scene};

use crate::machine::{self, Control, Panel, Throw};
use crate::room;

/// The sheet in the machine: just off the wall behind.
const STANDING_LIFT: f32 = 0.25;

/// `typewriter-desk`: the typewriter on a desk, seen from the chair.
pub struct Desk;

impl Stage for Desk {
    fn command(&self) -> &'static str {
        "typewriter-desk"
    }

    fn title(&self) -> &'static str {
        "Typewriter Desk"
    }

    fn about(&self) -> &'static str {
        "the typewriter on a desk"
    }

    fn backdrop(&self) -> Option<fn(&Painter, Rect)> {
        Some(room::paint)
    }

    fn typing_line_height(&self, view: Rect, metrics: &Metrics, zoom_percent: u16) -> Option<f32> {
        Some(machine::typing_line_height(view, metrics, zoom_percent))
    }

    /// The machine behind the sheet; the sheet goes out of sight into it.
    fn paint_behind_sheets(&self, painter: &Painter, scene: &Scene) -> Option<f32> {
        let Scene {
            view,
            metrics,
            typing_y,
            carriage_x,
            ..
        } = *scene;
        machine::paint_behind(painter, view, metrics, typing_y, carriage_x);
        Some(machine::sheet_bottom(view, metrics, typing_y))
    }

    fn sheet_lift(&self) -> Option<f32> {
        Some(STANDING_LIFT)
    }

    /// The machine in front of the sheet, and the paper bail over it.
    fn paint_over_sheets(&self, painter: &Painter, scene: &Scene) {
        let (metrics, typing_y) = (scene.metrics, scene.typing_y);
        machine::paint_front(painter, scene.view, metrics, typing_y);
        machine::paint_bail(painter, metrics, scene.carriage_x, typing_y);
    }

    /// On the front panel. Always shown, calm or not: they are the machine's.
    fn controls(
        &self,
        ui: &Ui,
        painter: &Painter,
        scene: &Scene,
        controls: &Controls,
    ) -> Option<[Rect; 5]> {
        let panel = Panel::new(scene.view, scene.metrics, scene.typing_y);
        let rects = Control::ALL.map(|control| panel.rect(control));
        let hovered = Control::ALL
            .into_iter()
            .zip(rects)
            .find(|(_, rect)| ui.rect_contains_pointer(*rect))
            .map(|(control, _)| control);
        panel.paint(painter, controls, hovered);
        Some(rects)
    }

    /// On the paper bail above the typing line, as on the SM9.
    fn scale_top(&self, scene: &Scene) -> Option<f32> {
        Some(machine::bail_scale_top(scene.metrics, scene.typing_y))
    }

    /// At the carriage's ends, travelling with it.
    fn platen_ends(&self, scene: &Scene) -> Option<[f32; 2]> {
        Some(machine::platen_ends(scene.carriage_x, scene.metrics))
    }

    /// The return lever's bracket, behind the left knob.
    fn paint_behind_knobs(&self, painter: &Painter, scene: &Scene) {
        let (left, throw) = lever(scene);
        machine::paint_lever_base(
            painter,
            scene.view,
            scene.metrics,
            scene.typing_y,
            left,
            throw,
        );
    }

    /// The return lever, over the left knob.
    fn paint_over_knobs(&self, painter: &Painter, scene: &Scene) {
        let (left, throw) = lever(scene);
        machine::paint_lever(
            painter,
            scene.view,
            scene.metrics,
            scene.typing_y,
            left,
            throw,
        );
    }

    /// The lever, springing back after a return.
    fn is_animating(&self, last_return: Return, now: f64) -> bool {
        Throw::new(last_return.at, last_return.inches).is_moving(now)
    }
}

/// The carriage's left end, where the lever is, and how far it is thrown.
fn lever(scene: &Scene) -> (f32, f32) {
    let [left, _] = machine::platen_ends(scene.carriage_x, scene.metrics);
    let Return { at, inches } = scene.last_return;
    (left, Throw::new(at, inches).amount(scene.now))
}
