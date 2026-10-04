//! Normal mode's hooks into the app: the room behind, the SM9 around
//! the sheet, its knobs and front panel for controls, the scale on its bail.
//!
//! The scene keeps no state: the app's model holds what it draws from (the
//! last return), and the machine's clickable parts go back to the app as
//! `Part`s for it to sense. It reaches the app's drawing only through
//! `draw.rs`: share a helper there by re-exporting it, not by making an app
//! module public.
//!
//! Drawing is in depth — one wgpu callback a frame, a depth buffer, the
//! pass's camera projecting the machine's millimetres (`depth/`,
//! `machine/canvas.rs`). A part moved into depth needs its real shape, not
//! the order it was drawn in.

use eframe::egui::{Context, Painter, Rect, Ui};
use eframe::egui_wgpu::RenderState;
use typewriter_core::Side;
use typewriter_core::carriage::Carriage;
use typewriter_ui::Stage;
use typewriter_ui::draw::ruler::Scale;
use typewriter_ui::draw::{Controls, FlatSheet, Metrics, PaperTable, Part, Return, Scene, When};

use crate::depth;
use crate::machine::{self, Control, Panel, Throw};
use crate::room;

/// `typewriter`: the typewriter on a desk, seen from the chair. Normal
/// mode, the default.
pub struct Desk;

impl Stage for Desk {
    fn command(&self) -> &'static str {
        "typewriter"
    }

    fn title(&self) -> &'static str {
        "Typewriter Desk"
    }

    fn about(&self) -> &'static str {
        "the typewriter on a desk"
    }

    /// The body and the sheets are drawn in depth.
    fn depth_buffer(&self) -> u8 {
        depth::DEPTH_BITS
    }

    fn start(&self, ctx: &Context, render_state: Option<&RenderState>) {
        if let Some(render_state) = render_state {
            depth::install(ctx, render_state);
        }
    }

    /// Sitting far back, the paper support's scale in view.
    fn zoom_min(&self) -> u16 {
        typewriter_ui::settings::ZOOM_MIN
    }

    fn backdrop(&self) -> Option<fn(&Painter, Rect, &Metrics, u16)> {
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
            last_return: Return { at, mm },
            now,
        } = *scene;
        let throw = Throw::new(at, mm).amount(now);
        machine::paint_behind(painter, view, metrics, typing_y, carriage_x, throw);
        None
    }

    /// Round the platen from the printing point, in depth: up its front,
    /// and back round under it and up the paper support.
    fn draws_sheets(&self) -> bool {
        true
    }

    fn paint_sheets(&self, painter: &Painter, scene: &Scene, sheets: Vec<FlatSheet>) {
        let (view, metrics, typing_y) = (scene.view, scene.metrics, scene.typing_y);
        machine::paint_sheets(painter, view, metrics, typing_y, sheets);
    }

    /// The sheet bends off the platen toward the seated eye as it rises.
    fn print_magnify(&self, along_mm: f32) -> f32 {
        machine::print_magnify(along_mm)
    }

    /// Behind the platen: sheets go in over it.
    fn paper_table(&self, scene: &Scene) -> Option<PaperTable> {
        Some(machine::paper_table(
            scene.view,
            scene.metrics,
            scene.typing_y,
        ))
    }

    /// On the carriage's ends, in depth with the machine: always there, as
    /// they are the machine's. Their grips drag the paper, wheel or no.
    fn knobs(
        &self,
        ui: &Ui,
        painter: &Painter,
        scene: &Scene,
        rolled: f32,
        active: bool,
        parts: &mut Vec<Part>,
    ) {
        let (view, metrics, typing_y) = (scene.view, scene.metrics, scene.typing_y);
        let turning = (scene.carriage_x, rolled);
        let hovered = |grip| active && ui.rect_contains_pointer(grip);
        let grips = machine::paint_knobs(painter, view, metrics, typing_y, turning, hovered);
        parts.extend(
            [Side::Left, Side::Right]
                .into_iter()
                .zip(grips)
                .map(|(side, grip)| Part::platen_knob(side, grip, metrics)),
        );
    }

    /// The machine in front of the sheet.
    fn paint_over_sheets(&self, painter: &Painter, scene: &Scene) {
        let (view, metrics, typing_y) = (scene.view, scene.metrics, scene.typing_y);
        machine::paint_front(painter, view, metrics, typing_y);
    }

    /// On the front panel. Always shown, calm or not: they are the machine's,
    /// and so they answer at once.
    fn controls(
        &self,
        ui: &Ui,
        painter: &Painter,
        scene: &Scene,
        controls: &Controls,
        parts: &mut Vec<Part>,
    ) {
        let panel = Panel::new(scene.view, scene.metrics, scene.typing_y);
        let rects = Control::ALL.map(|control| panel.rect(control));
        let hovered = Control::ALL
            .into_iter()
            .zip(rects)
            .find(|(_, rect)| ui.rect_contains_pointer(*rect))
            .map(|(control, _)| control);
        panel.paint(painter, controls, hovered);
        let [spacing, zoom, correct, goal, save] = rects;
        let when = When::Always;
        parts.extend([
            Part::spacing(spacing, when),
            Part::zoom(zoom, when),
            Part::correction(correct, controls.delete_in_cycle, when),
            Part::goal(goal, when),
            Part::save(save, controls.keeping, controls.location, when),
        ]);
    }

    /// On the paper bail above the typing line, as on the SM9, in depth with
    /// the machine: its stops, as printed, to set the margins.
    fn scale(
        &self,
        painter: &Painter,
        scene: &Scene,
        scale: &Scale,
        carriage: &Carriage,
        parts: &mut Vec<Part>,
    ) {
        let (view, metrics, typing_y) = (scene.view, scene.metrics, scene.typing_y);
        let printed = machine::print_scale(painter, view, metrics, typing_y, scale, carriage);
        parts.extend(
            [Side::Left, Side::Right].map(|side| Part::margin_stop(&printed, carriage, side)),
        );
    }

    /// The lever, springing back after a return.
    fn is_animating(&self, last_return: Return, now: f64) -> bool {
        Throw::new(last_return.at, last_return.mm).is_moving(now)
    }

    /// What it draws in depth.
    #[cfg(test)]
    fn snapshot_callback(
        &self,
        callback: &eframe::egui::PaintCallback,
        clip: Rect,
        raster: &mut typewriter_ui::snapshot::Raster,
    ) {
        depth::rasterize(callback, clip, raster);
    }
}

#[cfg(test)]
mod tests {
    use eframe::egui::{self, vec2};
    use typewriter_ui::Stage;
    use typewriter_ui::snapshot::{self, BACKSPACE, ERASE, Shot, render};

    use super::Desk;

    /// A full page's worth of type: what the `snapshot` test's `page`
    /// shots carry.
    fn page() -> String {
        "Call me Ishmael. Some years ago - never mind how long precisely -\n\
                    having little or no money in my purse, and nothing particular to\n\
                    interest me on shore, I thought I would sail about a little and see\n\
                    the watery part of the world.\n"
            .repeat(6)
    }

    /// Draws the desk scene to PNGs in `$TYPEWRITER_SNAPSHOT`, to look at:
    /// `cargo test -p typewriter --release -- --ignored snapshot`.
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
        // Near the line's end: the right knob and side plate beside the guide.
        let line_end = Shot {
            size: vec2(3000.0, 1400.0),
            zoom_percent: 200,
            text: &"x".repeat(72),
            after_seconds: 5.0,
        };
        let image = render(Box::new(Desk), &line_end).unwrap();
        image.save(folder.join("line-end.png")).unwrap();
        // The plain app's scale, its middle marked.
        let plain = Shot {
            size: vec2(1600.0, 1000.0),
            zoom_percent: 200,
            text: &"x".repeat(45),
            after_seconds: 5.0,
        };
        let image = render(Box::new(typewriter_ui::Plain), &plain).unwrap();
        image.save(folder.join("plain.png")).unwrap();
        // A page well begun: its lines over the platen and up the sheet.
        let page = page();
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
        // A slip over a wrong letter, then the right one: the chalk hides it.
        let corrected = format!("Typewritet{BACKSPACE}{ERASE}t{BACKSPACE}{ERASE}r");
        let shot = Shot {
            size: vec2(1600.0, 1000.0),
            zoom_percent: 200,
            text: &corrected,
            after_seconds: 5.0,
        };
        let image = render(Box::new(Desk), &shot).unwrap();
        image.save(folder.join("corrected.png")).unwrap();
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

    /// Times the typing view's frame — `run_ui` plus `ctx.tessellate` —
    /// normal against plain, empty page and full, at three window sizes:
    /// the desk's 1600 × 1000 at 100 %, a full-HD window, and the wide
    /// 3000 × 1400 at 200 %. Run it in debug and release, e.g.
    /// `TYPEWRITER_BENCH=50 cargo test -p typewriter --release -- --ignored bench --nocapture`.
    /// The same env gate runs the depth pass's GPU bench
    /// (`depth::gpu::tests::gpu_bench`) under the same filter; that wants a
    /// device and reports per-pass times beside these CPU ones. Add
    /// `--test-threads=1` so the two benches do not run, and share cores
    /// with, each other.
    #[test]
    #[ignore]
    fn bench() {
        let Some(frames) = std::env::var("TYPEWRITER_BENCH")
            .ok()
            .and_then(|said| said.parse::<usize>().ok())
        else {
            return;
        };
        let full = page();
        let build = if cfg!(debug_assertions) {
            "debug"
        } else {
            "release"
        };
        let shots = [
            ("1600×1000@100", vec2(1600.0, 1000.0), 100),
            ("1920×1080@100", vec2(1920.0, 1080.0), 100),
            ("3000×1400@200", vec2(3000.0, 1400.0), 200),
        ];
        println!("{frames} timed frames, {build}: the desk scene's solids and vertices per frame.");
        println!(
            "{:<17}{:>21}{:>9}{:>9}{:>9}{:>11}{:>9}{:>9}{:>8}{:>10}",
            "frame",
            "shot",
            "ms",
            "median",
            "run_ui",
            "tessellate",
            "fastest",
            "slowest",
            "solids",
            "vertices"
        );
        for (what, text) in [("empty", ""), ("full", full.as_str())] {
            for (shot_name, size, zoom_percent) in shots {
                let shot = Shot {
                    size,
                    zoom_percent,
                    text,
                    after_seconds: 5.0,
                };
                for (name, plain) in [("Desk", false), ("Plain", true)] {
                    let ctx = egui::Context::default();
                    let stage: Box<dyn Stage> = if plain {
                        Box::new(typewriter_ui::Plain)
                    } else {
                        Box::new(Desk)
                    };
                    let time = snapshot::bench(&ctx, stage, &shot, frames).unwrap();
                    // The plain app draws no depth pass: its counts are `-`.
                    let (solids, vertices) = match crate::depth::counts(&ctx) {
                        Some((solids, vertices)) => (solids.to_string(), vertices.to_string()),
                        None => ("-".to_string(), "-".to_string()),
                    };
                    println!(
                        "{:<17}{:>21}{:>9.1}{:>9.1}{:>9.1}{:>11.1}{:>9.1}{:>9.1}{:>8}{:>10}",
                        format!("{name}, {what}"),
                        shot_name,
                        time.mean_ms,
                        time.median_ms,
                        time.run_ms,
                        time.tessellate_ms,
                        time.fastest_ms,
                        time.slowest_ms,
                        solids,
                        vertices,
                    );
                }
            }
        }
    }
}
