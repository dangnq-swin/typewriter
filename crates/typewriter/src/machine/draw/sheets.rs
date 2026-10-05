//! The sheets on their way through the machine, built into the render
//! crate's scene: the paper's matte faces and the print laid on them as
//! decals, gathered through [`convert`] instead of the old depth pass.
//!
//! The geometry is the old pass's own: [`gather`] asks
//! [`solids_sheets`](super::super::sheet::solids_sheets) for the same
//! [`Solids`](crate::depth::Solids) the old path draws and hands them to the
//! frame. Nothing is re-tessellated between the passes.
//!
//! **Texture limitation (wave 2).** The bridge maps every solid to
//! `typewriter_render::device::Texture::WHITE`, because the render crate's
//! white texel is the only handle a converted solid can name: the old
//! [`Solid`](crate::depth::Solid) carries egui's `TextureId`, which is not
//! the device's own handle. So the paper's fill texture — the uvs the old
//! pass samples — is not yet read; the paper shows the vertex colours alone.
//! A faithful port needs the bridge to carry a `Texture` per solid (a
//! resolver the app uploads the paper's texels into), which is a change in
//! [`convert`], not here.

use eframe::egui::{Painter, Rect};
use glam::Vec3;
use typewriter_ui::draw::{FlatSheet, Metrics};

use super::convert::Frame;

/// `sheets`, on their way through the machine for the typing line at
/// `typing_y`: the paper and its print, into `frame` as `sheet`-named
/// meshes standing where they were built.
pub(crate) fn gather(
    frame: &mut Frame,
    painter: &Painter,
    view: Rect,
    metrics: &Metrics,
    typing_y: f32,
    sheets: Vec<FlatSheet>,
) {
    let solids = super::super::sheet::solids_sheets(painter, view, metrics, typing_y, sheets);
    frame.add("sheet", &solids, Vec3::ZERO);
}

#[cfg(test)]
mod tests {
    use eframe::egui::{Color32, Context, Mesh, Pos2, Shape, Stroke, pos2, vec2};
    use typewriter_core::Profile;
    use typewriter_ui::draw::{Metrics, points_per_inch};

    use super::*;

    /// The sheets as the plain app lays them: a paper quad, its print on it.
    fn flat_sheet() -> FlatSheet {
        let mut paper = Mesh::default();
        for (x, y) in [(10.0, 20.0), (110.0, 20.0), (110.0, 220.0), (10.0, 220.0)] {
            paper.colored_vertex(pos2(x, y), Color32::WHITE);
        }
        paper.add_triangle(0, 1, 2);
        paper.add_triangle(0, 2, 3);
        let print = vec![Shape::line_segment(
            [pos2(20.0, 40.0), pos2(100.0, 40.0)],
            Stroke::new(1.0, Color32::BLACK),
        )];
        FlatSheet { paper, print }
    }

    fn metrics(zoom_percent: u16) -> Metrics {
        let sm9 = include_str!("../../../../../profiles/olympia-sm9.toml");
        let profile = Profile::from_toml_str(sm9).unwrap();
        Metrics::new(&profile, points_per_inch(zoom_percent))
    }

    fn view() -> Rect {
        Rect::from_min_size(Pos2::ZERO, vec2(1600.0, 1000.0))
    }

    #[test]
    fn the_sheets_solids_carry_the_paper_and_its_print() {
        let ctx = Context::default();
        let metrics = metrics(100);
        let view = view();
        let typing_y = view.height() * 0.62;
        let mut output = ctx.run_ui(Default::default(), |ui| {
            let painter = ui.painter();
            let solids = crate::machine::sheet::solids_sheets(
                painter,
                view,
                &metrics,
                typing_y,
                vec![flat_sheet()],
            );
            let [(_, opaque), (_, decals)] = solids.layers();
            assert!(!opaque.is_empty(), "the paper draws opaque");
            assert!(!decals.is_empty(), "the print draws as a decal");
        });
        output.textures_delta.clear();
    }

    #[test]
    fn gathering_fills_the_frame_with_the_paper_and_its_print() {
        let ctx = Context::default();
        let metrics = metrics(100);
        let view = view();
        let typing_y = view.height() * 0.62;
        let mut frame = Frame::new();
        let mut output = ctx.run_ui(Default::default(), |ui| {
            gather(
                &mut frame,
                ui.painter(),
                view,
                &metrics,
                typing_y,
                vec![flat_sheet()],
            );
        });
        output.textures_delta.clear();
        assert!(!frame.is_empty(), "the sheets reached the frame");
        // The frame registers both the paper (opaque) and the print (decal):
        // its scene's placements name each layer.
        let scene = format!("{:?}", frame.scene());
        assert!(scene.contains("layer: Opaque"), "the paper is opaque");
        assert!(scene.contains("layer: Decal"), "the print is a decal");
    }
}
