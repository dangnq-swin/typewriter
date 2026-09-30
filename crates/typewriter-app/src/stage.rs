//! What an edition draws around the sheet, through [`Stage`]'s hooks. Each
//! default is the plain app's: the chosen background behind, the sheet flat
//! on it, the scale and knobs hanging from the typing line, the plates below.

use eframe::egui::{Painter, Rect, Ui};
use typewriter_core::session::Progress;
use typewriter_core::{EraseMode, Goal, LineSpacing};

use crate::filing::Keeping;
use crate::render::Metrics;

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

    /// A resting sheet's shadow strength, standing off what is behind it.
    /// `None`: it lies flat.
    fn sheet_lift(&self) -> Option<f32> {
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

    /// The scale's top, printed on the machine, which drew its plate.
    /// `None`: on its own plate, hanging from the typing line.
    fn scale_top(&self, _scene: &Scene) -> Option<f32> {
        None
    }

    /// The platen's ends on screen, where the knobs turn on the typing line.
    /// `None`: beside the paper, hanging from the scale.
    fn platen_ends(&self, _scene: &Scene) -> Option<[f32; 2]> {
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
