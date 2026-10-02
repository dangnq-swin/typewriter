//! What a mode draws around the sheet, through [`Stage`]'s hooks. Each
//! default is the plain app's: the chosen background behind, the sheet flat
//! on it, the scale and knobs hanging from the typing line, the plates below.
//! Whatever on the machine the pointer can grab, the hooks draw and hand
//! back as [`Part`]s: the app senses them, the scene keeps no state.

use eframe::egui::{Context, CursorIcon, Mesh, Painter, Rect, Response, Sense, Shape, Ui};
use eframe::egui_wgpu::RenderState;
use typewriter_core::carriage::Carriage;
use typewriter_core::session::Progress;
use typewriter_core::{EraseMode, Goal, LineSpacing, Side};

use crate::app::intent::Intent;
use crate::filing::{Keeping, WriteStatus};
use crate::render::ruler::Scale;
use crate::render::{self, Metrics, knob};

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
    /// Where the project is kept: the save control's reading.
    pub location: &'a str,
}

/// When a [`Part`] answers the pointer. The typing view senses every part
/// each frame; this is what must hold first.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum When {
    /// The machine's own: answers calm or not, feeding or not. The model
    /// turns away what a moving sheet must not take.
    Always,
    /// Chrome's: answers once fully shown, not mid-fade.
    Shown,
    /// Chrome's work: answers once fully shown and no sheet is feeding.
    Idle,
}

/// What a [`Part`] asks of the model this frame, given the pointer's
/// response to it.
type Grab = Box<dyn Fn(&Response) -> Vec<Intent>>;

/// One named thing on the machine a finger can grab: where it lies this
/// frame, and what it asks of the model taken and released. A hook draws
/// its parts and pushes them into `parts`; the app senses the pointer, so
/// the scene keeps no state of them.
pub struct Part {
    /// Its name, steady between frames: names it in egui's memory.
    pub name: String,
    /// Where it answers the pointer.
    pub rect: Rect,
    /// What it says under the pointer.
    pub tip: String,
    /// What it answers to: click, drag or both.
    pub sense: Sense,
    /// The cursor over it.
    pub cursor: CursorIcon,
    pub when: When,
    /// The wheel rolls the paper over a platen knob, not the view.
    pub takes_wheel: bool,
    /// What to ask of the model each frame the pointer holds it down.
    pub grab: Grab,
    /// What to ask of the model when the pointer lets go.
    pub release: Option<Intent>,
}

impl Part {
    /// The spacing control: click for the next notch.
    pub fn spacing(rect: Rect, when: When) -> Self {
        Self::click(
            "spacing",
            rect,
            when,
            "Line spacing (F1 / F2 / F3). Click for the next notch.",
            Intent::NextSpacing,
        )
    }

    /// The zoom control: double-click for 100 %.
    pub fn zoom(rect: Rect, when: When) -> Self {
        let grab = |response: &Response| {
            if response.double_clicked() {
                vec![Intent::ResetZoom]
            } else {
                Vec::new()
            }
        };
        Self::named(
            "zoom",
            rect,
            when,
            "Zoom (mouse wheel). Double-click for 100 %.".to_owned(),
            Box::new(grab),
        )
    }

    /// The correction control: click for the next way to take out type.
    pub fn correction(rect: Rect, delete_in_cycle: bool, when: When) -> Self {
        let tip = if delete_in_cycle {
            "Click (or F4) for the next way: correction paper, eraser, fluid or delete"
        } else {
            "Click (or F4) for the next way: correction paper, eraser or fluid"
        };
        Self::click("correction", rect, when, tip, Intent::NextCorrection)
    }

    /// The goal control: click for the next goal, or off.
    pub fn goal(rect: Rect, when: When) -> Self {
        Self::click(
            "goal",
            rect,
            when,
            "Click for the next: 250, 500 or 1000 words, 15, 25 or 50 minutes of typing, or off",
            Intent::NextGoal,
        )
    }

    /// The save control: click to save now. Reads `keeping`; `location` is
    /// where the project is kept.
    pub fn save(rect: Rect, keeping: &Keeping, location: &str, when: When) -> Self {
        let tip = match keeping {
            Keeping::Autosave(WriteStatus::Failed(err)) => {
                format!("Could not save: {err}. Click to try again.")
            }
            Keeping::Autosave(_) => format!("Saved to {location}. Click to save now."),
            Keeping::Draft => "Kept in the drafts folder. Click to save it as a file.".to_owned(),
            Keeping::Off { unsaved: true } => "Unsaved changes. Click to save.".to_owned(),
            Keeping::Off { unsaved: false } => "Saved. Click to save now.".to_owned(),
        };
        Self::click("autosave", rect, when, tip, Intent::Save)
    }

    /// A platen knob's grip at `rect`: dragged or scrolled, it rolls the
    /// paper a half-line per notch; let go, the turn stops.
    pub fn platen_knob(side: Side, rect: Rect, metrics: &Metrics) -> Self {
        let per_notch = knob::DRAG_POINTS_PER_NOTCH * metrics.points_per_inch / 96.0;
        let grab = move |response: &Response| {
            // The front face up rolls on, as on a platen.
            if response.dragged() {
                vec![Intent::DragKnob {
                    points: -response.drag_delta().y,
                    per_notch,
                }]
            } else {
                Vec::new()
            }
        };
        let name = match side {
            Side::Left => "platen-knob-left",
            Side::Right => "platen-knob-right",
        };
        Self {
            name: name.to_owned(),
            rect,
            tip: "Platen knob (Up / Down): drag or scroll to roll a half-line".to_owned(),
            sense: Sense::DRAG,
            cursor: CursorIcon::ResizeVertical,
            when: When::Idle,
            takes_wheel: true,
            grab: Box::new(grab),
            release: Some(Intent::ReleaseKnob),
        }
    }

    /// A margin stop's grip on `scale`: drag moves the margin to the column
    /// under the pointer, click releases both margins.
    pub fn margin_stop(scale: &Scale, carriage: &Carriage, side: Side) -> Self {
        let scale = *scale;
        let (name, key) = match side {
            Side::Left => ("Left", "Home"),
            Side::Right => ("Right", "End"),
        };
        let (margin, other) = match side {
            Side::Left => (carriage.left_margin, carriage.right_margin),
            Side::Right => (carriage.right_margin, carriage.left_margin),
        };
        let grab = move |response: &Response| {
            let mut asks = Vec::new();
            if response.clicked() {
                asks.push(Intent::ReleaseMargins);
            }
            let Some(pointer) = response
                .interact_pointer_pos()
                .filter(|_| response.dragged())
            else {
                return asks;
            };
            // Stop short of the other stop: no jolt per frame.
            let column = match side {
                Side::Left => scale.column_at(pointer.x).min(other - 1),
                Side::Right => scale.column_at(pointer.x).max(other + 1),
            };
            if column != margin {
                asks.push(Intent::MoveMargin { side, column });
            }
            asks
        };
        Self {
            name: format!("margin-stop-{name}"),
            rect: scale.stop(carriage, side),
            tip: format!(
                "{name} margin (Shift+{key}). Drag to move; click to release the margins (Home)."
            ),
            sense: render::CLICK_AND_DRAG,
            cursor: CursorIcon::ResizeHorizontal,
            when: When::Idle,
            takes_wheel: false,
            grab: Box::new(grab),
            release: None,
        }
    }

    /// Click-only, the pointing hand: `intent` each time clicked.
    fn click(name: &str, rect: Rect, when: When, tip: impl Into<String>, intent: Intent) -> Self {
        let grab = move |response: &Response| {
            if response.clicked() {
                vec![intent.clone()]
            } else {
                Vec::new()
            }
        };
        Self::named(name, rect, when, tip.into(), Box::new(grab))
    }

    /// A named thing with a tooltip and nothing to release: the plates and
    /// the machine's panel buttons alike.
    fn named(name: &str, rect: Rect, when: When, tip: String, grab: Grab) -> Self {
        Self {
            name: name.to_owned(),
            rect,
            tip,
            sense: render::CLICK,
            cursor: CursorIcon::PointingHand,
            when,
            takes_wheel: false,
            grab,
            release: None,
        }
    }
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
    /// the one under the pointer lit if `active`: draws them and pushes their
    /// grips into `parts`, to drag. Pushes nothing: the plain app's knobs,
    /// which fade in calm.
    fn knobs(
        &self,
        _ui: &Ui,
        _painter: &Painter,
        _scene: &Scene,
        _rolled: f32,
        _active: bool,
        _parts: &mut Vec<Part>,
    ) {
    }

    /// After the sheets, calm or not: what stands in front of them.
    fn paint_over_sheets(&self, _painter: &Painter, _scene: &Scene) {}

    /// Its own spacing, zoom, correction, goal and save controls, calm or
    /// not, reading `controls`: draws them and pushes each into `parts`, to
    /// click. Pushes nothing: the plain app's plates, which fade in calm.
    fn controls(
        &self,
        _ui: &Ui,
        _painter: &Painter,
        _scene: &Scene,
        _controls: &Controls,
        _parts: &mut Vec<Part>,
    ) {
    }

    /// Prints `scale`, laid out at the sheet's scale, and `carriage`'s stops
    /// on the machine itself, calm or not: draws them and pushes the stops'
    /// grips into `parts`. Pushes nothing: the plain app's plate, hanging
    /// from the typing line, which fades in calm.
    fn scale(
        &self,
        _painter: &Painter,
        _scene: &Scene,
        _scale: &Scale,
        _carriage: &Carriage,
        _parts: &mut Vec<Part>,
    ) {
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
