//! What a mode draws around the sheet, through [`Stage`]'s hooks. Each
//! default is the plain app's: the chosen background behind, the sheet flat
//! on it, the scale and knobs hanging from the typing line, the plates below.

use eframe::egui::{Context, Mesh, Painter, Rect, Shape, Ui};
use eframe::egui_wgpu::RenderState;
use typewriter_core::carriage::Carriage;
use typewriter_core::session::Progress;
use typewriter_core::{EraseMode, Goal, LineSpacing};

use crate::filing::Keeping;
use crate::render::Metrics;
use crate::render::ruler::Scale;

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
    /// Millimetres.
    pub mm: f64,
}

impl Return {
    /// Before the first: long ago.
    pub const NONE: Self = Self {
        at: f64::NEG_INFINITY,
        mm: 0.0,
    };
}

/// Behind the platen, where a sheet goes in: for its timing as it is slid
/// down the table and wound round.
pub struct PaperTable {
    /// Paper round the platen from where it goes in to the typing line,
    /// millimetres.
    pub wrap_mm: f32,
    /// How far up the table from there a sheet shows, millimetres: past that
    /// it has gone over the top, out of sight.
    pub seen_mm: f32,
}

/// A sheet as the plain app lays it flat on screen, for a stage that draws
/// the sheets itself ([`Stage::paint_sheets`]).
pub struct FlatSheet {
    /// The paper in its fill (the texture, or a flat colour): a quad, its
    /// corners clockwise from the top left.
    pub paper: Mesh,
    /// What is on it: the margin frame and every mark of its page.
    pub print: Vec<Shape>,
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

/// A mode: its name, and what it draws around the sheet. The typing
/// view calls the hooks in its order: behind the sheets, the knobs and the
/// scale, then over the sheets.
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

    /// The window's depth buffer, in bits: none for a flat stage.
    fn depth_buffer(&self) -> u8 {
        0
    }

    /// Once the window is open: readies what it draws with, e.g. its own
    /// pipelines on eframe's wgpu renderer (`None` without one).
    fn start(&self, _ctx: &Context, _render_state: Option<&RenderState>) {}

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

    /// Draws the sheets itself, in [`Stage::paint_sheets`]. False: the
    /// typing view draws them flat, in their order.
    fn draws_sheets(&self) -> bool {
        false
    }

    /// The sheets of this frame, the outgoing one first, if it
    /// [`draws_sheets`](Stage::draws_sheets).
    fn paint_sheets(&self, _painter: &Painter, _scene: &Scene, _sheets: Vec<FlatSheet>) {}

    /// Its own platen knobs, calm or not, turned by `rolled` points of paper,
    /// the one under the pointer lit if `active`: where each grip is, left
    /// then right, to drag. `None`: the plain app's, which fade in calm.
    fn knobs(
        &self,
        _ui: &Ui,
        _painter: &Painter,
        _scene: &Scene,
        _rolled: f32,
        _active: bool,
    ) -> Option<[Rect; 2]> {
        None
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

    /// Prints `scale`, laid out at the sheet's scale, and `carriage`'s stops
    /// on the machine itself, calm or not: where it shows, for its stops.
    /// `None`: the plain app's plate, hanging from the typing line, which
    /// fades in calm.
    fn scale(
        &self,
        _painter: &Painter,
        _scene: &Scene,
        _scale: &Scale,
        _carriage: &Carriage,
    ) -> Option<Scale> {
        None
    }

    /// Still moving at `now` since `last_return`: draw again soon.
    fn is_animating(&self, _last_return: Return, _now: f64) -> bool {
        false
    }

    /// Fills a paint callback of its own, inside `clip`, in a snapshot.
    #[cfg(feature = "snapshot")]
    fn snapshot_callback(
        &self,
        _callback: &eframe::egui::PaintCallback,
        _clip: Rect,
        _raster: &mut crate::snapshot::Raster,
    ) {
    }
}

/// The plain app, `typewriter-plain`: the focused writing tool, every hook
/// its default.
pub struct Plain;

impl Stage for Plain {
    fn command(&self) -> &'static str {
        "typewriter-plain"
    }

    fn title(&self) -> &'static str {
        "Typewriter"
    }

    fn about(&self) -> &'static str {
        "a typewriter simulator for focused writing"
    }
}
