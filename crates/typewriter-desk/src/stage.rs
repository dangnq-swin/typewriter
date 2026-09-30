//! The desk edition's hooks into the app: the room behind, the SM9 around
//! the sheet, its front panel for controls, the scale on its bail, the knobs
//! and return lever on its carriage.

use eframe::egui::{Painter, Rect, Ui};
use typewriter_app::Stage;
use typewriter_app::draw::{Controls, Knob, Metrics, PaperTable, Platen, Return, Scene, SheetWay};
use typewriter_core::Side;

use crate::machine::{self, Control, Panel, Throw};
use crate::room;

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

    /// The body and the sheets.
    fn depth(&self) -> bool {
        true
    }

    /// Sitting far back, the paper support's scale in view.
    fn zoom_min(&self) -> u16 {
        typewriter_app::settings::ZOOM_MIN
    }

    fn backdrop(&self) -> Option<fn(&Painter, Rect)> {
        Some(room::paint)
    }

    fn typing_line_height(&self, view: Rect, metrics: &Metrics, zoom_percent: u16) -> Option<f32> {
        Some(machine::typing_line_height(view, metrics, zoom_percent))
    }

    /// The machine behind the sheet. In depth: the platen and the cover
    /// hide what goes into it.
    fn paint_behind_sheets(&self, painter: &Painter, scene: &Scene) -> Option<f32> {
        let Scene {
            view,
            metrics,
            typing_y,
            carriage_x,
            ..
        } = *scene;
        machine::paint_behind(painter, view, metrics, typing_y, carriage_x);
        None
    }

    /// Round the platen from the printing point: up its front, and back
    /// round under it and up the paper support.
    fn sheet_way(&self, scene: &Scene) -> Option<SheetWay> {
        Some(machine::sheet_way(
            scene.view,
            scene.metrics,
            scene.typing_y,
        ))
    }

    /// Behind the platen: sheets go in over it.
    fn paper_table(&self, scene: &Scene) -> Option<PaperTable> {
        Some(machine::paper_table(
            scene.view,
            scene.metrics,
            scene.typing_y,
        ))
    }

    /// The machine in front of the sheet, and the paper bail over it.
    fn paint_over_sheets(&self, painter: &Painter, scene: &Scene) {
        let (view, metrics, typing_y) = (scene.view, scene.metrics, scene.typing_y);
        machine::paint_front(painter, view, metrics, typing_y);
        machine::paint_bail(painter, view, metrics, scene.carriage_x, typing_y);
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
            ends: ends(scene),
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
        let [left_end, right_end] = ends(scene);
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

/// The carriage's ends on screen, where the knobs turn.
fn ends(scene: &Scene) -> [f32; 2] {
    let Scene {
        view,
        metrics,
        typing_y,
        carriage_x,
        ..
    } = *scene;
    machine::platen_ends(view, metrics, typing_y, carriage_x)
}

/// The carriage's left end, where the lever is, and how far it is thrown.
fn lever(scene: &Scene) -> (f32, f32) {
    let [left, _] = ends(scene);
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
        // A page well begun: its lines over the platen and up the sheet.
        let page = "Call me Ishmael. Some years ago - never mind how long precisely -\n\
                    having little or no money in my purse, and nothing particular to\n\
                    interest me on shore, I thought I would sail about a little and see\n\
                    the watery part of the world.\n"
            .repeat(6);
        for (name, zoom_percent) in [("page", 100), ("page-close", 200)] {
            let shot = Shot {
                size: vec2(1600.0, 1400.0),
                zoom_percent,
                text: &page,
                after_seconds: 5.0,
            };
            let image = render(Box::new(Desk), &shot).unwrap();
            image.save(folder.join(format!("{name}.png"))).unwrap();
        }
        // A new sheet halfway round the platen, the last one filed.
        let feeding = Shot {
            size: vec2(1600.0, 1000.0),
            zoom_percent: 100,
            text: &format!("{}The last line.\n", "A line.\n".repeat(63)),
            after_seconds: 0.6,
        };
        let image = render(Box::new(Desk), &feeding).unwrap();
        image.save(folder.join("feeding.png")).unwrap();
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
