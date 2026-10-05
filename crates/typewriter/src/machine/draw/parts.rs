//! The machine's parts built into the render crate's scene: the standing
//! solids, the carriage's travel, the knobs and the front, gathered through
//! [`convert`] instead of the old depth pass. T2 fills this in.
//!
//! Each of the old pass's groups is mirrored here: the standing parts are
//! added to a [`Frame`] by name with the shift that used to move them, so a
//! kept solid's travel is its placement's transform and its geometry is
//! converted once; what is rebuilt a frame (the paper support, the ribbon
//! scale, the key legends, and the stateful knobs, lever and basket) is
//! caught on a [`Canvas`] of its own and taken before the old pass sees it.

use eframe::egui::{Painter, Rect};
use glam::Vec3;
use typewriter_ui::draw::Metrics;

use crate::machine::canvas::Canvas;
use crate::machine::standing::Standing;
use crate::machine::{Eye, carriage, keyboard, knob, printing_point, support};

use super::convert::Frame;

/// Behind the sheet: the desk, the body's top, the carriage at `carriage_x`
/// with its bail and its return lever thrown `throw` (0..=1). The machine's
/// parts, into `frame`.
pub(crate) fn behind(
    frame: &mut Frame,
    painter: &Painter,
    view: Rect,
    metrics: &Metrics,
    typing_y: f32,
    carriage_x: f32,
    throw: f32,
) {
    let eye = Eye::new(view, metrics, typing_y);
    let standing = Standing::of(painter, &eye);
    let middle = (carriage_x - eye.origin.x) / eye.ppmm;
    frame.add("desk", &standing.desk, Vec3::ZERO);
    frame.add("deck", &standing.deck, Vec3::ZERO);
    capture(frame, painter, "support", |canvas| {
        support::paint(canvas, &eye, metrics, middle);
    });
    let by = Vec3::X * middle;
    frame.add("carriage", &standing.carriage, by);
    frame.add("bail", &standing.bail, by);
    let [left, _] = carriage::ends(middle);
    let platen = eye.about(carriage::platen_axis());
    frame.add("bracket", &standing.bracket, Vec3::X * left);
    capture(frame, painter, "lever", |canvas| {
        standing.gather_lever(canvas, &platen, throw, left);
    });
}

/// The platen knobs at the carriage's ends, for the sheet centred at
/// `carriage_x`, turned by `rolled` points of paper.
pub(crate) fn knobs(
    frame: &mut Frame,
    painter: &Painter,
    view: Rect,
    metrics: &Metrics,
    typing_y: f32,
    carriage_x: f32,
    rolled: f32,
) {
    let plain = Eye::new(view, metrics, typing_y);
    let middle = (carriage_x - plain.origin.x) / plain.ppmm;
    let eye = plain.about(carriage::platen_axis());
    let turned = rolled / eye.ppmm / knob::DISC.1;
    let standing = Standing::of(painter, &plain);
    frame.add("knob_bodies", &standing.knob_bodies, Vec3::X * middle);
    capture(frame, painter, "ribs", |canvas| {
        standing.gather_ribs(canvas, &eye, turned, middle);
    });
}

/// In front of the sheet, after it: the alignment guide, the ribbon cover,
/// the front panel and the keyboard.
pub(crate) fn front(
    frame: &mut Frame,
    painter: &Painter,
    view: Rect,
    metrics: &Metrics,
    typing_y: f32,
) {
    let eye = Eye::new(view, metrics, typing_y);
    let standing = Standing::of(painter, &eye);
    capture(frame, painter, "opening", |canvas| {
        standing.gather_opening(canvas, &eye);
    });
    frame.add("guide_plates", &standing.guide_plates, Vec3::ZERO);
    capture(frame, painter, "guide_scale", |canvas| {
        printing_point::paint_scale(canvas, &eye, metrics);
    });
    frame.add("guide_rest", &standing.guide_rest, Vec3::ZERO);
    frame.add("cover", &standing.cover, Vec3::ZERO);
    frame.add("panel", &standing.panel, Vec3::ZERO);
    frame.add("rod", &standing.rod, Vec3::ZERO);
    frame.add("well", &standing.well, Vec3::ZERO);
    frame.add("inner_walls", &standing.inner_walls, Vec3::ZERO);
    frame.add("knob_casters", &standing.knob_casters, Vec3::ZERO);
    frame.add("key_levers", &standing.key_levers, Vec3::ZERO);
    frame.add("side_controls", &standing.side_controls, Vec3::ZERO);
    frame.add("panel_edge", &standing.panel_edge, Vec3::ZERO);
    frame.add("caps", &standing.caps, Vec3::ZERO);
    capture(frame, painter, "key_legends", |canvas| {
        keyboard::paint_legends(canvas, &eye);
    });
    frame.add("case_frame", &standing.case_frame, Vec3::ZERO);
    frame.add("selector_marks", &standing.selector_marks, Vec3::ZERO);
}

/// Gathers what `draw` puts on a canvas of its own into `frame`, standing
/// where it was built: the parts rebuilt a frame, and the travelling ones
/// whose own gather already carries their shift.
fn capture(frame: &mut Frame, painter: &Painter, name: &str, draw: impl Fn(&Canvas)) {
    let canvas = Canvas::depth(painter);
    draw(&canvas);
    let solids = canvas.take_solids();
    frame.add(name, &solids, Vec3::ZERO);
}

#[cfg(test)]
mod tests {
    use super::*;
    use eframe::egui::{Pos2, vec2};

    fn metrics(zoom_percent: u16) -> Metrics {
        let sm9 = include_str!("../../../../../profiles/olympia-sm9.toml");
        let profile = typewriter_core::Profile::from_toml_str(sm9).unwrap();
        Metrics::new(&profile, typewriter_ui::draw::points_per_inch(zoom_percent))
    }

    /// The scene's placements, each mesh's name and the translation its
    /// transform stands at, read off the scene's `Debug`: the render crate
    /// keeps its placement list to itself, and this task may not touch the
    /// bridge that owns it.
    fn placements(frame: &Frame) -> Vec<(String, Vec3)> {
        let debug = format!("{:?}", frame.scene());
        debug
            .match_indices("Placed {")
            .map(|(start, tag)| {
                let rest = &debug[start + tag.len()..];
                let name = quoted(rest, "mesh: \"").expect("a placed mesh");
                let at = translation(rest).expect("a placement transform");
                (name, at)
            })
            .collect()
    }

    /// The text of `debug` between `marker` and the next quote.
    fn quoted(debug: &str, marker: &str) -> Option<String> {
        let start = debug.find(marker)? + marker.len();
        let end = debug[start..].find('"')? + start;
        Some(debug[start..end].to_owned())
    }

    /// The translation `debug`'s first `w_axis` carries, glam's `Mat4`
    /// debug layout: `w_axis: Vec4(x, y, z, w)`.
    fn translation(debug: &str) -> Option<Vec3> {
        let marker = "w_axis: Vec4(";
        let start = debug.find(marker)? + marker.len();
        let end = debug[start..].find(')')? + start;
        let mut parts = debug[start..end].split(',').map(str::trim);
        let x = parts.next()?.parse().ok()?;
        let y = parts.next()?.parse().ok()?;
        let z = parts.next()?.parse().ok()?;
        Some(Vec3::new(x, y, z))
    }

    /// The group a mesh name belongs to: its name up to the solid index.
    fn group(name: &str) -> &str {
        name.split('.').next().unwrap_or(name)
    }

    #[test]
    fn behind_gathers_the_desks_carriage_and_lever() {
        let ctx = eframe::egui::Context::default();
        let view = Rect::from_min_size(Pos2::ZERO, vec2(1600.0, 1000.0));
        let metrics = metrics(100);
        let typing_y = view.height() * 0.5;
        let mut frame = Frame::new();
        let mut output = ctx.run_ui(Default::default(), |ui| {
            behind(
                &mut frame,
                ui.painter(),
                view,
                &metrics,
                typing_y,
                800.0,
                0.0,
            );
        });
        output.textures_delta.clear();
        assert!(!frame.is_empty(), "the behind gathered nothing");
        let groups: std::collections::HashSet<_> = placements(&frame)
            .iter()
            .map(|(name, _)| group(name).to_owned())
            .collect();
        for want in [
            "desk", "deck", "support", "carriage", "bail", "bracket", "lever",
        ] {
            assert!(groups.contains(want), "missing group {want}: {groups:?}");
        }
    }

    #[test]
    fn a_carriage_shift_travels_the_standing_parts_by_that_much() {
        let ctx = eframe::egui::Context::default();
        let view = Rect::from_min_size(Pos2::ZERO, vec2(1600.0, 1000.0));
        let metrics = metrics(100);
        let typing_y = view.height() * 0.5;
        let draw = |carriage_x: f32| {
            let mut frame = Frame::new();
            let mut output = ctx.run_ui(Default::default(), |ui| {
                behind(
                    &mut frame,
                    ui.painter(),
                    view,
                    &metrics,
                    typing_y,
                    carriage_x,
                    0.0,
                );
            });
            output.textures_delta.clear();
            frame
        };
        let home = draw(800.0);
        let shifted = draw(800.0 + 12.7 * metrics.points_per_mm());
        let home: std::collections::HashMap<_, _> = placements(&home).into_iter().collect();
        let shifted: std::collections::HashMap<_, _> = placements(&shifted).into_iter().collect();
        assert_eq!(home.len(), shifted.len());
        // The carriage and its bail are kept at rest and placed by the
        // carriage's travel: their transforms move exactly 12.7 mm across,
        // and the body that stands still does not.
        let mut moved = 0;
        for (name, at) in &home {
            let to = shifted.get(name).expect("the same meshes");
            match group(name) {
                "carriage" | "bail" => {
                    let delta = *to - *at;
                    assert!(
                        (delta - Vec3::new(12.7, 0.0, 0.0)).length() < 1e-4,
                        "{name}: {at:?} -> {to:?}"
                    );
                    moved += 1;
                }
                "desk" | "deck" => assert_eq!(at, to, "{name} stayed"),
                _ => {}
            }
        }
        assert!(moved > 0, "the carriage travelled");
    }

    #[test]
    fn front_gathers_the_opening_the_panel_and_the_keyboard() {
        let ctx = eframe::egui::Context::default();
        let view = Rect::from_min_size(Pos2::ZERO, vec2(1600.0, 1000.0));
        let metrics = metrics(100);
        let typing_y = view.height() * 0.5;
        let mut frame = Frame::new();
        let mut output = ctx.run_ui(Default::default(), |ui| {
            front(&mut frame, ui.painter(), view, &metrics, typing_y);
        });
        output.textures_delta.clear();
        assert!(!frame.is_empty(), "the front gathered nothing");
        let groups: std::collections::HashSet<_> = placements(&frame)
            .iter()
            .map(|(name, _)| group(name).to_owned())
            .collect();
        for want in [
            "opening",
            "guide_plates",
            "guide_scale",
            "guide_rest",
            "cover",
            "panel",
            "rod",
            "well",
            "inner_walls",
            "knob_casters",
            "key_levers",
            "side_controls",
            "panel_edge",
            "caps",
            "key_legends",
            "case_frame",
            "selector_marks",
        ] {
            assert!(groups.contains(want), "missing group {want}: {groups:?}");
        }
    }

    #[test]
    fn knobs_gather_the_bodies_and_the_turned_ribs() {
        let ctx = eframe::egui::Context::default();
        let view = Rect::from_min_size(Pos2::ZERO, vec2(1600.0, 1000.0));
        let metrics = metrics(100);
        let typing_y = view.height() * 0.5;
        let mut frame = Frame::new();
        let mut output = ctx.run_ui(Default::default(), |ui| {
            knobs(
                &mut frame,
                ui.painter(),
                view,
                &metrics,
                typing_y,
                800.0,
                0.0,
            );
        });
        output.textures_delta.clear();
        assert!(!frame.is_empty(), "the knobs gathered nothing");
        let groups: std::collections::HashSet<_> = placements(&frame)
            .iter()
            .map(|(name, _)| group(name).to_owned())
            .collect();
        for want in ["knob_bodies", "ribs"] {
            assert!(groups.contains(want), "missing group {want}: {groups:?}");
        }
    }
}
