//! The machine's standing solids: every part built once, in its
//! millimetres, and kept — the case, the cover, the panel, the keyboard —
//! so a frame gathers them where they stand instead of re-tessellating a
//! machine that has not changed.
//!
//! The carriage's slide, the bail's anchoring and the return lever's bracket
//! are travels: kept solids, [`Solids::shifted`] with the carriage. Three
//! groups carry their state in the colour of a face rather than in where it
//! stands, and rebuild only when that state changes — between changes their
//! travels are kept too:
//!
//! - the knob ribs, lit at the angle each stands at, which the platen's
//!   turn moves: keyed on the turn;
//! - the return lever, whose steel catches the lamp as it swings: keyed on
//!   the throw;
//! - the type basket under the cover, whose grain is sized in screen
//!   points: keyed on the zoom.
//!
//! None of them owes its screen positions to the eye it was built for:
//! `depth::end` redraws every vertex from its own millimetres, so zoom and
//! the typing line change only the projection. What is *laid* as text is
//! not standing either way: egui's font atlas may move a glyph between
//! frames, so legends, the bail's printed scale, the guide's tape and the
//! sheets' print are re-tessellated as always.

use std::sync::Arc;

use eframe::egui::{Id, Painter};
use glam::Vec3;

use super::{
    bail, body, canvas::Canvas, carriage, case, cover, eye::Eye, keyboard, knob, lever, panel,
    printing_point, side_controls,
};
use crate::depth::Solids;

/// Solids built in one state of a part that takes its lamp in its colour,
/// kept until that state comes again.
#[derive(Clone)]
struct Keyed {
    /// No turn, throw or zoom equals it: the first frame always builds.
    key: f32,
    solids: Solids,
}

/// The machine's standing solids, in the order `machine` gathers them.
pub(super) struct Standing {
    // Behind the sheet: the body's top, then the carriage's travelling trio.
    pub deck: Solids,
    pub carriage: Solids,
    pub bail: Solids,
    pub bracket: Solids,
    /// At the carriage's ends, at rest: the platen's turn moves only the
    /// ribs, which stay keyed; these never turn, they travel.
    pub knob_bodies: Solids,
    // In front of it: the basket under the opening (keyed), the guide split
    // about its tape (gathered a frame, screen-laid), then the case, panel
    // and keys.
    pub guide_plates: Solids,
    pub guide_rest: Solids,
    pub cover: Solids,
    pub panel: Solids,
    pub rod: Solids,
    pub well: Solids,
    pub inner_walls: Solids,
    pub key_shadows: Solids,
    pub key_levers: Solids,
    pub side_controls: Solids,
    pub panel_edge: Solids,
    pub caps: Solids,
    pub case_frame: Solids,
    pub selector_marks: Solids,
}

impl Keyed {
    /// `key` matches no turn, throw or zoom: the frame that finds it builds.
    fn waiting() -> Self {
        Self {
            key: f32::NAN,
            solids: Solids::default(),
        }
    }
}

fn standing_id() -> Id {
    Id::new("machine-standing")
}

impl Standing {
    /// The machine's standing solids: built whole on the first frame, kept
    /// in `painter`'s context data after.
    pub(super) fn of(painter: &Painter, eye: &Eye) -> Arc<Self> {
        let ctx = painter.ctx();
        if let Some(standing) = ctx.data_mut(|data| data.get_persisted::<Arc<Self>>(standing_id()))
        {
            return standing;
        }
        let standing = Arc::new(Self::build(painter, eye));
        ctx.data_mut(|data| data.insert_persisted(standing_id(), standing.clone()));
        standing
    }

    fn build(painter: &Painter, eye: &Eye) -> Self {
        let platen = eye.about(carriage::platen_axis());
        let at_rest = carriage::ends(0.0);
        let levers = keyboard::key_levers();
        Self {
            deck: collect(painter, eye, body::paint_deck),
            carriage: collect(painter, eye, |c, e| carriage::paint(c, e, 0.0)),
            bail: collect(painter, eye, |c, e| bail::paint(c, e, 0.0)),
            bracket: collect(painter, &platen, |c, e| lever::paint_bracket(c, e, 0.0)),
            knob_bodies: collect(painter, &platen, |c, e| knob::paint_bodies(c, e, at_rest)),
            guide_plates: collect(painter, eye, printing_point::paint_plates),
            guide_rest: collect(painter, eye, printing_point::paint_rest),
            cover: collect(painter, eye, cover::paint),
            panel: collect(painter, eye, panel::paint_face),
            rod: collect(painter, eye, |c, e| keyboard::paint_rod(c, e, &levers)),
            well: collect(painter, eye, case::paint_well),
            inner_walls: collect(painter, eye, case::paint_inner_walls),
            key_shadows: collect(painter, eye, keyboard::paint_shadows),
            key_levers: collect(painter, eye, |c, e| keyboard::paint_levers(c, e, &levers)),
            side_controls: collect(painter, eye, side_controls::paint),
            panel_edge: collect(painter, eye, case::paint_panel_edge),
            caps: collect(painter, eye, keyboard::paint_caps),
            case_frame: collect(painter, eye, case::paint_frame),
            selector_marks: collect(painter, eye, side_controls::paint_marks),
        }
    }

    /// The knob ribs at the platen `turned`, `eye` about the platen's axis:
    /// kept while the turn holds, and travelling with the carriage.
    pub(super) fn gather_ribs(&self, canvas: &Canvas, eye: &Eye, turned: f32, middle: f32) {
        gather(
            canvas,
            eye,
            "ribs",
            turned,
            Vec3::X * middle,
            |c, e, turned| knob::paint_ribs(c, e, carriage::ends(0.0), turned),
        );
    }

    /// The return lever at the carriage's left end `thrown`, `eye` about the
    /// platen's axis: kept while the throw holds, and travelling with it.
    pub(super) fn gather_lever(&self, canvas: &Canvas, eye: &Eye, thrown: f32, left: f32) {
        gather(
            canvas,
            eye,
            "lever",
            thrown,
            Vec3::X * left,
            |c, e, thrown| lever::paint_moving(c, e, 0.0, thrown),
        );
    }

    /// The type basket under the cover's opening: kept while the zoom
    /// holds, whose screen points size its grain.
    pub(super) fn gather_opening(&self, canvas: &Canvas, eye: &Eye) {
        gather(canvas, eye, "opening", eye.ppmm, Vec3::ZERO, |c, e, _| {
            cover::paint_opening(c, e)
        });
    }
}

/// The solids `draw` puts on a canvas of its own: a standing part, built
/// once and never handed to a frame's pass.
fn collect(painter: &Painter, eye: &Eye, draw: impl Fn(&Canvas, &Eye)) -> Solids {
    let canvas = Canvas::depth(painter);
    draw(&canvas, eye);
    canvas.take_solids()
}

/// Adds `what`'s solids to `canvas`, `by` millimetres along: rebuilt first
/// if `key` — the turn, throw or zoom they were built for — came changed.
/// They live in `canvas`'s context data, beside the standing machine's.
fn gather(
    canvas: &Canvas,
    eye: &Eye,
    what: &'static str,
    key: f32,
    by: Vec3,
    draw: impl Fn(&Canvas, &Eye, f32),
) {
    let painter = canvas.painter();
    let shifted = painter.ctx().data_mut(|data| {
        let id = Id::new(("machine-standing", what));
        let built = data.get_persisted_mut_or_insert_with::<Keyed>(id, Keyed::waiting);
        if built.key != key {
            *built = Keyed {
                key,
                solids: collect(painter, eye, |c, e| draw(c, e, key)),
            };
        }
        built.solids.shifted(by)
    });
    canvas.add_solids(shifted);
}
