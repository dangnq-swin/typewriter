//! The desk edition's hooks into the app: the room behind, the SM9 around
//! the sheet, its front panel for controls, the scale on its bail, the knobs
//! and return lever on its carriage.

use eframe::egui::{Painter, Rect, Ui};
use typewriter_app::Stage;
use typewriter_app::draw::{Controls, Knob, Metrics, PaperTable, Platen, Return, Scene};
use typewriter_core::Side;

use crate::machine::{self, Control, Panel, Throw};
use crate::room;

/// The sheet in the machine: just off the wall behind.
const STANDING_LIFT: f32 = 0.25;
/// Its top edge keeps some of its wind round the platen, clearing the
/// platen's top.
const SHEET_CURL: f32 = 0.85;

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

    /// Behind the platen: sheets go in over it.
    fn paper_table(&self, scene: &Scene) -> Option<PaperTable> {
        Some(machine::paper_table(
            scene.view,
            scene.metrics,
            scene.typing_y,
        ))
    }

    fn sheet_lift(&self) -> Option<f32> {
        Some(STANDING_LIFT)
    }

    fn sheet_curl(&self) -> f32 {
        SHEET_CURL
    }

    /// The machine in front of the sheet, and the paper bail over it.
    fn paint_over_sheets(&self, painter: &Painter, scene: &Scene) {
        let (metrics, typing_y) = (scene.metrics, scene.typing_y);
        machine::paint_front(painter, scene.view, metrics, typing_y, scene.carriage_x);
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

    /// At the carriage's ends, travelling with it, on the platen's axis.
    fn platen(&self, scene: &Scene) -> Option<Platen> {
        Some(Platen {
            ends: machine::platen_ends(scene.carriage_x, scene.metrics),
            axis_y: machine::platen_axis_y(scene.view, scene.metrics, scene.typing_y),
        })
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

    /// The ribbon cover, where it stands in front of the knobs, then the
    /// return lever over the left knob.
    fn paint_over_knobs(&self, painter: &Painter, scene: &Scene) {
        let axis_y = machine::platen_axis_y(scene.view, scene.metrics, scene.typing_y);
        let [left_end, right_end] = machine::platen_ends(scene.carriage_x, scene.metrics);
        let knobs = [(Side::Left, left_end), (Side::Right, right_end)]
            .map(|(side, end)| Knob::on_axis(scene.metrics, side, end, axis_y).grip());
        machine::paint_cover_over_knobs(painter, scene.view, scene.metrics, scene.typing_y, knobs);
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

#[cfg(test)]
mod tests {
    use eframe::egui::vec2;
    use typewriter_app::snapshot::{Shot, render};

    use super::Desk;

    /// Draws the desk to PNGs in `$TYPEWRITER_SNAPSHOT`, to look at:
    /// `cargo test -p typewriter-desk --release -- --ignored snapshot`.
    #[test]
    #[ignore]
    fn snapshot() {
        let Some(folder) = std::env::var_os("TYPEWRITER_SNAPSHOT") else {
            return;
        };
        let folder = std::path::PathBuf::from(folder);
        let text = "The quick brown fox jumps over the lazy dog.\nA second line.";
        let fresh = Shot {
            size: vec2(1600.0, 1600.0),
            zoom_percent: 200,
            text: "",
            after_seconds: 5.0,
        };
        let image = render(Box::new(Desk), &fresh).unwrap();
        image.save(folder.join("fresh.png")).unwrap();
        // Wide enough for both of the carriage's ends.
        let lever = Shot {
            size: vec2(3000.0, 1400.0),
            zoom_percent: 200,
            text: "x",
            after_seconds: 5.0,
        };
        let image = render(Box::new(Desk), &lever).unwrap();
        image.save(folder.join("lever.png")).unwrap();
        // The left knob passing the cover's corner as the carriage moves.
        for typed in [20, 30, 40] {
            let along = Shot {
                size: vec2(3000.0, 1400.0),
                zoom_percent: 200,
                text: &"x".repeat(typed),
                after_seconds: 5.0,
            };
            let image = render(Box::new(Desk), &along).unwrap();
            image
                .save(folder.join(format!("along-{typed}.png")))
                .unwrap();
        }
        let shots = [
            ("desk", vec2(1600.0, 1000.0), 100),
            ("sitting-back", vec2(1600.0, 1000.0), 50),
            ("close", vec2(1600.0, 1000.0), 200),
            ("tall", vec2(1400.0, 2400.0), 100),
            ("platen", vec2(1600.0, 1600.0), 200),
            ("far-back", vec2(1536.0, 960.0), 25),
        ];
        for (name, size, zoom_percent) in shots {
            let shot = Shot {
                size,
                zoom_percent,
                text,
                after_seconds: 5.0,
            };
            let image = render(Box::new(Desk), &shot).unwrap();
            image.save(folder.join(format!("{name}.png"))).unwrap();
        }
    }
}
