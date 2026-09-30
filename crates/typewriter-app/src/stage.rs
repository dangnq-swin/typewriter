//! What an edition draws around the sheet, through [`Stage`]'s hooks. Each
//! default is the plain app's: the chosen background behind, the sheet flat
//! on it, the scale and knobs hanging from the typing line, the plates below.

use eframe::egui::{Painter, Pos2, Rect, Ui};
use typewriter_core::session::Progress;
use typewriter_core::{EraseMode, Goal, LineSpacing};

use crate::filing::Keeping;
use crate::render::Metrics;

/// The plain app's furthest out: the page still reads.
const PLAIN_ZOOM_MIN: u16 = 50;

/// Where the machine stands this frame.
pub struct Scene<'a> {
    pub view: Rect,
    pub metrics: &'a Metrics,
    /// The typing line, on screen.
    pub typing_y: f32,
    /// The sheet's middle across: the carriage travels with it.
    pub carriage_x: f32,
    pub last_return: Return,
    pub now: f64,
}

/// The carriage's last return: when, and how far it glided home.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Return {
    pub at: f64,
    pub inches: f64,
}

impl Return {
    /// Before the first: long ago.
    pub const NONE: Self = Self {
        at: f64::NEG_INFINITY,
        inches: 0.0,
    };
}

/// The platen as drawn: its ends and its axis, on screen.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Platen {
    pub ends: [f32; 2],
    pub axis_y: f32,
}

/// Behind the platen, where a sheet goes in: the part not yet wound round
/// lies on it, its blank back to the writer.
pub struct PaperTable {
    /// Paper round the platen from where it goes in to the typing line.
    pub wrap_inches: f32,
    /// How far up the table from there a sheet shows: past that it has gone
    /// over the top, out of sight.
    pub seen_inches: f32,
    /// The platen's axis and radius on screen: the sheet wraps round it.
    pub platen_axis_y: f32,
    pub platen_radius: f32,
    /// Where a point of the sheet shows, `x` its place across on screen at
    /// the sheet's scale and `along` its inches up the table from where it
    /// goes in; and how lit it is there, 0..=1.
    pub place: Box<dyn Fn(f32, f32) -> (Pos2, f32)>,
}

/// A sheet's way through the machine, drawn in depth: up the platen's front
/// from the printing point to the sheet's top edge, and down round the
/// platen and up behind it to its bottom edge.
pub struct SheetWay {
    /// Where a point of the sheet is: `x` its place across on screen at the
    /// sheet's scale, `along` its inches from the printing point, up the
    /// front above zero, round the platen and up behind it below.
    pub place: Box<dyn Fn(f32, f32) -> Placed>,
}

/// A point of a sheet on its way.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Placed {
    pub pos: Pos2,
    /// Its depth, 0 at the eye to 1 far off; and a hair nearer, for what is
    /// printed on it.
    pub depth: f32,
    pub print_depth: f32,
    /// How lit it is, 0..=1.
    pub lit: f32,
    /// Its printed side is turned toward the eye.
    pub facing: bool,
}

/// What the spacing, zoom, correction, goal and save controls read.
pub struct Controls<'a> {
    pub spacing: LineSpacing,
    pub zoom_percent: u16,
    pub erase: EraseMode,
    pub slip_in: bool,
    pub delete_in_cycle: bool,
    pub goal: Option<Goal>,
    pub goals: &'a [Goal],
    pub progress: Option<Progress>,
    pub keeping: &'a Keeping,
}

/// An edition: its name, and what it draws around the sheet. The typing
/// view calls the hooks in its order: behind the sheets, over them, then,
/// fading in calm, the scale and around the knobs.
pub trait Stage {
    /// The command that opens it, also its app id.
    fn command(&self) -> &'static str;
    /// The window's title.
    fn title(&self) -> &'static str;
    /// What it is, in `--help`.
    fn about(&self) -> &'static str;

    /// The furthest out it zooms, one of the zoom's notches.
    fn zoom_min(&self) -> u16 {
        PLAIN_ZOOM_MIN
    }

    /// Draws in depth ([`crate::draw::depth`]): the window keeps a depth
    /// buffer.
    fn depth(&self) -> bool {
        false
    }

    /// Behind everything, in place of the chosen background, which then
    /// shows on the sheets only. `None`: the chosen background.
    fn backdrop(&self) -> Option<fn(&Painter, Rect)> {
        None
    }

    /// The typing line's height, as a share of `view`'s. `None`: the plain
    /// app's.
    fn typing_line_height(
        &self,
        _view: Rect,
        _metrics: &Metrics,
        _zoom_percent: u16,
    ) -> Option<f32> {
        None
    }

    /// Before the sheets. Returns where they go out of sight below; `None`:
    /// nowhere, they run off the window.
    fn paint_behind_sheets(&self, _painter: &Painter, _scene: &Scene) -> Option<f32> {
        None
    }

    /// Where sheets go in behind the platen. `None`: nowhere, they rise
    /// from below the window.
    fn paper_table(&self, _scene: &Scene) -> Option<PaperTable> {
        None
    }

    /// The sheets' way through the machine, drawing them in depth. `None`:
    /// they are drawn flat, in their order.
    fn sheet_way(&self, _scene: &Scene) -> Option<SheetWay> {
        None
    }

    /// A resting sheet's shadow strength, standing off what is behind it.
    /// `None`: it lies flat.
    fn sheet_lift(&self) -> Option<f32> {
        None
    }

    /// How far a resting sheet's top edge stays curled back toward the
    /// platen, 0..=1.
    fn sheet_curl(&self) -> f32 {
        0.0
    }

    /// After the sheets, calm or not: what stands in front of them.
    fn paint_over_sheets(&self, _painter: &Painter, _scene: &Scene) {}

    /// Its own spacing, zoom, correction, goal and save controls, calm or
    /// not, reading `controls`: where each is, to click. `None`: the plain
    /// app's plates, which fade in calm.
    fn controls(
        &self,
        _ui: &Ui,
        _painter: &Painter,
        _scene: &Scene,
        _controls: &Controls,
    ) -> Option<[Rect; 5]> {
        None
    }

    /// The scale's top, printed on the machine, which drew its plate.
    /// `None`: on its own plate, hanging from the typing line.
    fn scale_top(&self, _scene: &Scene) -> Option<f32> {
        None
    }

    /// The platen on screen, where the knobs turn. `None`: beside the
    /// paper, hanging from the scale.
    fn platen(&self, _scene: &Scene) -> Option<Platen> {
        None
    }

    /// Before the knobs: what they hide.
    fn paint_behind_knobs(&self, _painter: &Painter, _scene: &Scene) {}

    /// After the knobs: what passes in front of them.
    fn paint_over_knobs(&self, _painter: &Painter, _scene: &Scene) {}

    /// Still moving at `now` since `last_return`: draw again soon.
    fn is_animating(&self, _last_return: Return, _now: f64) -> bool {
        false
    }
}

/// The plain app, `typewriter`: the focused writing tool, every hook its
/// default.
pub struct Plain;

impl Stage for Plain {
    fn command(&self) -> &'static str {
        "typewriter"
    }

    fn title(&self) -> &'static str {
        "Typewriter"
    }

    fn about(&self) -> &'static str {
        "a typewriter simulator for focused writing"
    }
}
