//! The desk edition's typewriter, an Olympia SM9 seen from the chair.
//!
//! The body is a model in inches, projected from a seated eye: `x` right of
//! the machine's centre, `y` toward the writer, `z` up, the origin at the
//! printing point. There the projection's scale is the sheet's, so the flat
//! sheet and the carriage (platen, knobs, bail), which travel with it, line
//! up with the still body around them.

use std::f32::consts::{FRAC_PI_2, PI};
use std::ops::RangeInclusive;

use eframe::egui::epaint::{Vertex, WHITE_UV};
use eframe::egui::{
    Align2, Color32, CornerRadius, FontId, Mesh, Painter, Pos2, Rect, Shape, Stroke, StrokeKind,
    Vec2, pos2, vec2,
};
use typewriter_core::session::Progress;
use typewriter_core::{EraseMode, Goal, LineSpacing};

use super::feed::convex_mesh;
use super::perspective::warp;
use super::{HIGHLIGHT, Metrics, ruler, splitmix64, unit};
use crate::filing::Keeping;

/// The eye from the printing point, and how far it looks down.
const EYE_INCHES: f32 = 24.0;
const EYE_TILT_DEGREES: f32 = 35.0;
/// The typing line's height in the view: leaning over the page at 100 %
/// zoom and above, sitting back to see the whole machine at 50 %. Never
/// lower than keeps the panel's controls in view, nor higher than the least.
const TYPING_LINE_LEANING: f32 = 0.62;
const TYPING_LINE_SITTING: f32 = 0.25;
const TYPING_LINE_LEAST: f32 = 0.2;
/// Between the panel's readings and the window's bottom edge.
const CONTROLS_ROOM: f32 = 12.0;

/// Centre to the carriage's ends, where the knobs are.
const PLATEN_HALF_INCHES: f32 = 5.9;
const PLATEN_DIAMETER_INCHES: f32 = 1.3;
/// The metal rings at the platen's ends.
const PLATEN_END_INCHES: f32 = 0.08;
/// Bands of shading round a roller: enough for a smooth curve.
const ROLLER_BANDS: u16 = 14;
/// Light from above and in front: angle round a roller from its top.
const LIGHT_DEGREES: f32 = 55.0;
/// The light is also to the writer's left: shadows fall right, a little
/// back. The machine's on the desk, in inches `(x, y)`; the knobs', in
/// degrees clockwise from up the panel.
const CAST_SHADOW: (f32, f32) = (0.9, -0.5);
const KNOB_SHADOW_TURN: f32 = 55.0;
/// The carriage's side plates, inside its ends; its back behind the platen,
/// above the typing line; how far the side plates reach below it.
const SIDE_PLATE_INCHES: f32 = 0.2;
const CARRIAGE_BACK: (f32, f32) = (1.05, 0.6);
const SIDE_PLATE_BELOW: f32 = 0.55;
/// The paper bail above the typing line: its scale's centre, the bar's
/// height past the scale's, its rollers either side of the sheet's centre,
/// and the arms holding it to the side plates, pivoting just above the line.
const BAIL_ABOVE_INCHES: f32 = 0.6;
const BAIL_EXTRA_INCHES: f32 = 0.08;
const BAIL_ROLLER_INCHES: f32 = 1.65;
const BAIL_ARM_INCHES: f32 = 0.14;
const BAIL_PIVOT_ABOVE: f32 = 0.1;
/// The aligning scale on the card holder: columns marked either side of the
/// type guide, from the plates' inner ends.
const ALIGNING_COLUMNS: i32 = 12;
/// The metal drawing notches at the plates' inner ends: width and height.
const NOTCH_INCHES: (f32, f32) = (0.17, 0.2);
/// The type guide's arch over the ribbon vibrator: half its width, and the
/// vibrator's half width.
const TYPE_GUIDE_HALF: f32 = 0.2;
const VIBRATOR_HALF: f32 = 0.3;

/// The deck under the carriage, and the rail in it: `y` ranges, and `z`.
const DECK: (f32, f32) = (-1.8, 0.1);
const RAIL: (f32, f32) = (-1.25, -0.75);
const DECK_Z: f32 = -0.55;
const DECK_HALF: f32 = 6.2;
/// The ribbon cover, sloping toward the writer: back and front `(y, z)`,
/// its half widths there, and the opening onto the type bars `(y, half)`.
/// The opening runs out at the back between the two plates' tips.
const COVER_BACK: (f32, f32) = (0.95, -0.5);
const COVER_FRONT: (f32, f32) = (4.0, -1.25);
const COVER_HALF: (f32, f32) = (5.95, 6.35);
const OPENING_BACK: (f32, f32) = (0.95, 1.9);
const OPENING_FRONT: (f32, f32) = (3.6, 4.1);
/// The plates' tips, rounded thick; the opening's front corners; the
/// plates' thickness, showing where they drop into the opening.
const PLATE_TIP: f32 = 0.35;
const OPENING_CORNER: f32 = 0.4;
const PLATE_THICKNESS: f32 = 0.3;
/// The type segment's radii: its teal core, bronze band and silver rim;
/// its height; how far its type bars reach from its centre, past the
/// opening's edges.
const SEGMENT_CORE: f32 = 1.0;
const SEGMENT_BAND: f32 = 1.5;
const SEGMENT_RIM: f32 = 1.6;
const SEGMENT_Z: f32 = -0.85;
const TYPE_BAR_REACH: f32 = 4.6;
/// How far the cover's shadow reaches inside the opening.
const OPENING_SHADE_INCHES: f32 = 0.3;
/// The dark throat between platen and cover, behind the card holder.
const THROAT_TOP: (f32, f32) = (0.05, -0.3);
/// The card holder's translucent plates under the platen, either side of
/// the type guide: `(y, z)` back and front, and their inner and outer `x`.
/// The sheet shows through them, and goes out of sight below them.
const TABLE_BACK: (f32, f32) = (0.18, -0.25);
const TABLE_FRONT: (f32, f32) = (0.85, -0.45);
const TABLE_X: (f32, f32) = (0.55, 4.35);
/// The front panel, falling steeply to the keyboard's opening.
const PANEL_BOTTOM: (f32, f32) = (4.7, -2.35);
const PANEL_HALF_BOTTOM: f32 = 6.45;
/// The controls where the maker's badge would be: across the panel at `y`,
/// their names and readings below.
const CONTROLS_Y: f32 = 4.35;
/// Knob to its name and reading beside it; their lines, above and below
/// the knob's centre.
const LABEL_GAP: f32 = 0.5;
const LABEL_LINES: (f32, f32) = (-0.07, 0.07);
const KNOB_RADIUS: f32 = 0.3;
const KNOB_HEIGHT: f32 = 0.22;
const BUTTON_HEIGHT: f32 = 0.1;
/// Past the knob, where its index marks are engraved: inner and outer.
const INDEX_MARKS: (f32, f32) = (1.25, 1.5);
const LAMP_RADIUS: f32 = 0.08;
/// The keyboard's well: the thin side walls either side of it, running the
/// case's full depth; the panel's thickness over it; the key bed.
const OPENING_HALF: f32 = 5.85;
const PANEL_EDGE_INCHES: f32 = 0.45;
const KEY_BED_Z: f32 = -4.25;
/// The walls' tops: easing from the panel's foot to `WALL_EASE_Z`, then
/// swooping down over `WALL_SWOOP` (`y`) to the case's front corners.
const WALL_EASE_Z: f32 = -2.55;
const WALL_SWOOP: (f32, f32) = (7.0, SHELF.1);
/// The shelf between the walls, flush with the space bar and reaching just
/// past it: its back `y`, the case's front `y`, its top `z`. The notch in
/// it that the space bar and the keys beside it sit in: half its width, its
/// front `y`.
const SHELF: (f32, f32, f32) = (8.45, 9.15, -3.9);
const NOTCH: (f32, f32) = (4.5, 9.02);
/// Where the shelf rounds off into the well beside the notch; the notch's
/// front corners; the case's front corners.
const SHELF_LIP: f32 = 0.3;
const NOTCH_ROUNDING: f32 = 0.15;
const FRAME_ROUNDING: f32 = 0.5;
/// The case's front face down to the plinth.
const PLINTH_TOP: f32 = -4.2;
const BASE_FRONT: f32 = SHELF.1;
/// The body's back on the desk, under the carriage.
const BODY_BACK: f32 = -2.6;
const DESK_Z: f32 = -4.35;

/// Keys: the far row's centre `(y, z)` of its tops, one row to the next.
const KEY_ROW: (f32, f32) = (5.45, -2.65);
const KEY_ROW_STEP: (f32, f32) = (0.8, -0.28);
/// The space bar's centre `y` and top `z`: standing above the shelf, its
/// foot down in the notch.
const SPACE_ROW: (f32, f32) = (8.75, -3.6);
/// Keys reach almost wall to wall, as on the real machine.
const KEY_PITCH: f32 = 0.86;
/// A key cap's top, its height, and how much wider its foot is each side.
const KEY_CAP: (f32, f32) = (0.64, 0.56);
const KEY_FRONT: f32 = 0.4;
const KEY_FLARE: f32 = 0.03;
/// The keyboard's middle, in key pitches from the far row's first key.
const KEYBOARD_MIDDLE: f32 = 5.65;
/// A key's post drops from under its cap; its lever runs back from the post
/// under the rows behind, into the machine under the panel.
const STEM_DROP: f32 = 0.3;
/// The key levers draw together toward the type basket.
const LEVER_CONVERGE: f32 = 0.94;
/// The rod the levers rest on: how far behind the far row, its radius, and
/// the gap between its top and the lowest lever over it.
const ROD_BEHIND: f32 = 0.35;
const ROD_RADIUS: f32 = 0.09;
const ROD_GAP: f32 = 0.02;
/// How far below a key's top its shadow falls, cast by the light.
const KEY_SHADOW_DROP: f32 = 0.6;

/// As on a US SM9 De Luxe, far row first: positions in key pitches from
/// the far row's first key, measured from one.
const KEYS: [&[Key]; 4] = [
    &[
        Key::new("=", "+", 0.0),
        Key::new("2", "\"", 1.0),
        Key::new("3", "#", 2.0),
        Key::new("4", "$", 3.0),
        Key::new("5", "%", 4.0),
        Key::new("6", "_", 5.0),
        Key::new("7", "&", 6.0),
        Key::new("8", "'", 7.0),
        Key::new("9", "(", 8.0),
        Key::new("0", ")", 9.0),
        Key::new("-", "*", 10.0),
        Key::new("¾", "!", 11.0),
        Key::new("TAB", "", 12.0),
    ],
    &[
        Key::new("MR", "", -0.71),
        Key::new("Q", "", 0.38),
        Key::new("W", "", 1.38),
        Key::new("E", "", 2.38),
        Key::new("R", "", 3.38),
        Key::new("T", "", 4.38),
        Key::new("Y", "", 5.38),
        Key::new("U", "", 6.38),
        Key::new("I", "", 7.38),
        Key::new("O", "", 8.38),
        Key::new("P", "", 9.38),
        Key::new("½", "¼", 10.38),
        Key::new(BACKSPACE, "", 11.6),
    ],
    &[
        Key::new("", "", -0.38),
        Key::new("A", "", 0.64),
        Key::new("S", "", 1.64),
        Key::new("D", "", 2.64),
        Key::new("F", "", 3.64),
        Key::new("G", "", 4.64),
        Key::new("H", "", 5.64),
        Key::new("J", "", 6.64),
        Key::new("K", "", 7.64),
        Key::new("L", "", 8.64),
        Key::new(";", ":", 9.64),
        Key::new("¢", "@", 10.64),
    ],
    &[
        Key::shift(-0.31),
        Key::new("Z", "", 1.22),
        Key::new("X", "", 2.22),
        Key::new("C", "", 3.22),
        Key::new("V", "", 4.22),
        Key::new("B", "", 5.22),
        Key::new("N", "", 6.22),
        Key::new("M", "", 7.22),
        Key::new(",", ",", 8.22),
        Key::new(".", ".", 9.22),
        Key::new("/", "?", 10.22),
        Key::shift(11.53),
    ],
];
/// The side controls in the gaps between the keys and the walls, `x` from
/// the middle: the carriage lock by the far row, its green cap's half width
/// and half depth; the touch control and ribbon selectors by the third row,
/// how far each bends toward the writer from its post, and its chrome
/// paddle's half width and height.
const LOCK_X: f32 = 5.5;
const LOCK_CAP: (f32, f32) = (0.09, 0.12);
/// The lock's cap stands this far above the far row's tops, and this far
/// behind them.
const LOCK_RISE: f32 = 0.25;
const LOCK_BACK: f32 = 0.12;
/// The selectors' paddles reach up to just under the walls' tops, level with
/// their marks.
const PADDLE_BELOW_WALL: f32 = 0.05;
/// How far the lock's post runs on below its cap, and the selectors' below
/// their bends, before their rods run back into the machine: the selectors'
/// rods well under the lock's.
const LOCK_DROP: f32 = 0.4;
const SELECTOR_DROP: f32 = 0.9;
const SELECTOR_X: f32 = 5.66;
const SELECTOR_REACH: f32 = 0.25;
const PADDLE: (f32, f32) = (0.08, 0.35);
/// Text laid flat on the machine is set at this many points an inch, then
/// bent onto its surface.
const FLAT_TEXT: f32 = 160.0;
/// The backspace key's legend: drawn as an arrow.
const BACKSPACE: &str = "<-";
/// The tab clear key, the space bar and the tab set key: `x`, and legends.
const SPACE_BAR: [(f32, f32, &str); 3] = [
    (-4.4, -3.55, "clear"),
    (-3.45, 3.35, ""),
    (3.45, 4.3, "set"),
];

const RUBBER: Color32 = Color32::from_rgb(0x16, 0x15, 0x14);
const RUBBER_SHINE: Color32 = Color32::from_rgb(0x55, 0x53, 0x50);
const METAL: Color32 = Color32::from_rgb(0x8C, 0x8E, 0x8C);
const METAL_SHINE: Color32 = Color32::from_rgb(0xDC, 0xDE, 0xDA);
const CHROME: Color32 = Color32::from_rgb(0xC8, 0xCA, 0xC6);
const IVORY_LIT: Color32 = Color32::from_rgb(0xEE, 0xEC, 0xE2);
const IVORY: Color32 = Color32::from_rgb(0xE2, 0xDF, 0xD2);
const IVORY_SHADE: Color32 = Color32::from_rgb(0xC6, 0xC2, 0xB3);
const GLASS: Color32 = Color32::from_rgba_unmultiplied_const(0xF8, 0xF8, 0xF4, 0x46);
const GLASS_EDGE: Color32 = Color32::from_rgba_unmultiplied_const(0x70, 0x70, 0x6A, 0xB0);
const BAIL_TOP: Color32 = Color32::from_rgb(0xD4, 0xD6, 0xD2);
const BAIL_MID: Color32 = Color32::from_rgb(0xEE, 0xEF, 0xEC);
const BAIL_LOW: Color32 = Color32::from_rgb(0xAE, 0xB0, 0xAC);
const EDGE: Color32 = Color32::from_rgb(0x9C, 0x97, 0x88);
const DARK: Color32 = Color32::from_rgb(0x1A, 0x1A, 0x19);
const INSIDE: Color32 = Color32::from_rgb(0x16, 0x12, 0x0E);
const INSIDE_FRONT: Color32 = Color32::from_rgb(0x2A, 0x2A, 0x28);
/// The dark outline round small raised marks.
const MARK_EDGE: Color32 = Color32::from_rgb(0x30, 0x2C, 0x26);
/// Dark enough to read as the machine's insides, light enough for the keys'
/// shadows to show on.
const KEY_BED: Color32 = Color32::from_rgb(0x2A, 0x29, 0x26);
const KEY_BED_FRONT: Color32 = Color32::from_rgb(0x40, 0x3F, 0x3A);
/// The rod the key levers rest on: steel.
const ROD: Color32 = Color32::from_rgb(0x6A, 0x6D, 0x6A);
const TYPE_BAR: Color32 = Color32::from_rgb(0x2E, 0x2B, 0x26);
const BRONZE: Color32 = Color32::from_rgb(0x7C, 0x66, 0x42);
const BRONZE_DARK: Color32 = Color32::from_rgb(0x4C, 0x3D, 0x28);
const TEAL: Color32 = Color32::from_rgb(0x3A, 0x68, 0x64);
const TEAL_LIGHT: Color32 = Color32::from_rgb(0x5A, 0x8A, 0x84);
const PALE_RING: Color32 = Color32::from_rgb(0xCC, 0xC6, 0xB2);
/// The ribbon colour selector's marks, back to front: blue (the ribbon's
/// black half), white (stencil), red; and where each is from the third
/// row's middle.
const RIBBON_MARKS: [Color32; 3] = [
    Color32::from_rgb(0x2A, 0x4F, 0x9A),
    Color32::from_rgb(0xF2, 0xF2, 0xEE),
    Color32::from_rgb(0xB0, 0x30, 0x2A),
];
const RIBBON_ALONG: [f32; 3] = [-0.25, 0.0, 0.25];
/// Typing in black: the selector at blue.
const RIBBON_SET: usize = 0;
/// A ribbon mark's half size, rounding, and how far it stands proud.
const RIBBON_MARK: (f32, f32, f32) = (0.055, 0.02, 0.02);
/// The touch control's less and more marks: where from the third row's
/// middle, their half length and stroke.
const TOUCH_MARKS: (f32, f32, f32) = (0.3, 0.1, 0.035);
const PLINTH: Color32 = Color32::from_rgb(0x8C, 0x8A, 0x86);
const CAP: Color32 = Color32::from_rgb(0xF0, 0xED, 0xE0);
const CAP_FRONT: Color32 = Color32::from_rgb(0xD2, 0xCD, 0xBC);
const SHIFT_CAP: Color32 = Color32::from_rgb(0x5E, 0xB2, 0x9C);
const SHIFT_FRONT: Color32 = Color32::from_rgb(0x3F, 0x8C, 0x78);
const LEGEND: Color32 = Color32::from_rgb(0x1E, 0x1E, 0x1E);
const STEM: Color32 = Color32::from_rgb(0x4C, 0x4C, 0x48);
const STEM_SHINE: Color32 = Color32::from_rgb(0xE8, 0xE8, 0xE2);
const SPRING_LIT: Color32 = Color32::from_rgb(0xD6, 0xD6, 0xCE);
const SPRING_DARK: Color32 = Color32::from_rgb(0x5E, 0x5E, 0x58);
const KNOB_TOP: Color32 = Color32::from_rgb(0xEC, 0xE7, 0xD6);
const KNOB_SIDE: Color32 = Color32::from_rgb(0xCC, 0xC5, 0xAF);
const KNOB_RIB: Color32 = Color32::from_rgb(0xAE, 0xA6, 0x8E);
const ENGRAVED: Color32 = Color32::from_rgb(0x6E, 0x6A, 0x5E);
const READING: Color32 = Color32::from_rgb(0x2A, 0x28, 0x25);
const LAMP_OFF: Color32 = Color32::from_rgb(0x4A, 0x4A, 0x46);

/// A key on the keyboard.
struct Key {
    legend: &'static str,
    /// Above the legend; empty for none.
    shifted: &'static str,
    /// In key pitches from the far row's first key.
    at: f32,
    /// In key caps.
    width: f32,
    shift: bool,
}

impl Key {
    const fn new(legend: &'static str, shifted: &'static str, at: f32) -> Self {
        Self {
            legend,
            shifted,
            at,
            width: 1.0,
            shift: false,
        }
    }

    const fn shift(at: f32) -> Self {
        Self {
            legend: "",
            shifted: "",
            at,
            width: 1.8,
            shift: true,
        }
    }

    fn x(&self) -> f32 {
        (self.at - KEYBOARD_MIDDLE) * KEY_PITCH
    }
}

/// The typing line's share of `view`'s height at `zoom_percent`, keeping
/// the panel's controls in view where it can.
pub fn typing_line_height(view: Rect, metrics: &Metrics, zoom_percent: u16) -> f32 {
    let leaning = ((f32::from(zoom_percent) - 50.0) / 50.0).clamp(0.0, 1.0);
    let wanted = TYPING_LINE_SITTING + (TYPING_LINE_LEANING - TYPING_LINE_SITTING) * leaning;
    let eye = Eye::new(view, metrics, 0.0);
    let reach = KNOB_RADIUS * INDEX_MARKS.1 + 0.1;
    let readings = eye
        .at(panel_offset(on_panel(0.0, CONTROLS_Y), reach, 180.0))
        .y;
    let height = view.height().max(1.0);
    let in_view = (height - readings - CONTROLS_ROOM) / height;
    wanted.min(in_view).max(TYPING_LINE_LEAST)
}

/// Where the sheet goes out of sight into the machine, for the typing line
/// at `typing_y`: nothing of it shows below.
pub fn sheet_bottom(view: Rect, metrics: &Metrics, typing_y: f32) -> f32 {
    let eye = Eye::new(view, metrics, typing_y);
    eye.at([0.0, TABLE_FRONT.0, TABLE_FRONT.1]).y
}

/// The carriage's ends, where the knobs are, for a sheet centred at
/// `centre_x`.
pub fn platen_ends(centre_x: f32, metrics: &Metrics) -> [f32; 2] {
    let half = PLATEN_HALF_INCHES * metrics.points_per_inch;
    [centre_x - half, centre_x + half]
}

/// The seated eye, anchored at the printing point.
struct Eye {
    origin: Pos2,
    ppi: f32,
}

impl Eye {
    fn new(view: Rect, metrics: &Metrics, typing_y: f32) -> Self {
        Self {
            origin: pos2(view.center().x, typing_y),
            ppi: metrics.points_per_inch,
        }
    }

    /// Whether a face at `p` facing `normal` is turned toward the eye.
    fn sees(p: [f32; 3], normal: [f32; 3]) -> bool {
        let (sin, cos) = EYE_TILT_DEGREES.to_radians().sin_cos();
        let eye = [0.0, EYE_INCHES * cos, EYE_INCHES * sin];
        dot(sub(eye, p), normal) > 0.0
    }

    /// Screen points per inch at `p`: `ppi` at the printing point.
    fn scale(&self, [_, y, z]: [f32; 3]) -> f32 {
        let (sin, cos) = EYE_TILT_DEGREES.to_radians().sin_cos();
        let depth = (EYE_INCHES - y * cos - z * sin).max(1.0);
        self.ppi * EYE_INCHES / depth
    }

    fn at(&self, p: [f32; 3]) -> Pos2 {
        let [x, y, z] = p;
        let (sin, cos) = EYE_TILT_DEGREES.to_radians().sin_cos();
        let up = z * cos - y * sin;
        self.origin + self.scale(p) * vec2(x, -up)
    }

    fn polygon(&self, points: &[[f32; 3]]) -> Vec<Pos2> {
        points.iter().map(|&p| self.at(p)).collect()
    }

    /// Fills convex `points`, each vertex coloured by `colour`.
    fn fill(&self, painter: &Painter, points: &[[f32; 3]], colour: impl Fn([f32; 3]) -> Color32) {
        let mut colours = points.iter().map(|&p| colour(p));
        let mesh = convex_mesh(&self.polygon(points), |pos| Vertex {
            pos,
            uv: WHITE_UV,
            color: colours.next().unwrap_or_default(),
        });
        painter.add(Shape::mesh(mesh));
    }

    /// A line through `points`, `width_inches` thick where it starts.
    fn line(&self, painter: &Painter, points: &[[f32; 3]], width_inches: f32, colour: Color32) {
        let width = width_inches * self.scale(points[0]);
        painter.add(Shape::line(
            self.polygon(points),
            Stroke::new(width, colour),
        ));
    }

    fn outline(&self, painter: &Painter, points: &[[f32; 3]]) {
        painter.add(Shape::closed_line(
            self.polygon(points),
            Stroke::new(1.0, EDGE),
        ));
    }
}

/// Behind the sheet, before it: the machine's shadow, the deck the carriage
/// rides on, and the carriage for the sheet centred at `carriage_x`.
pub fn paint_behind(
    painter: &Painter,
    view: Rect,
    metrics: &Metrics,
    typing_y: f32,
    carriage_x: f32,
) {
    let eye = Eye::new(view, metrics, typing_y);
    paint_shadow(painter, &eye);
    let across = |y: (f32, f32)| {
        [
            [-DECK_HALF, y.0, DECK_Z],
            [DECK_HALF, y.0, DECK_Z],
            [DECK_HALF, y.1, DECK_Z],
            [-DECK_HALF, y.1, DECK_Z],
        ]
    };
    eye.fill(painter, &across(DECK), |_| INSIDE_FRONT);
    eye.fill(painter, &across(RAIL), |_| DARK);
    let rail_lip = [[-DECK_HALF, RAIL.1, DECK_Z], [DECK_HALF, RAIL.1, DECK_Z]];
    eye.line(painter, &rail_lip, 0.03, CHROME);

    let ppi = metrics.points_per_inch;
    let [left, right] = platen_ends(carriage_x, metrics);
    // The carriage's back, behind the platen.
    let back = (
        typing_y - CARRIAGE_BACK.0 * ppi,
        typing_y - CARRIAGE_BACK.1 * ppi,
    );
    let mut mesh = Mesh::default();
    let back_radius = (back.1 - back.0) / 2.0;
    roller(
        &mut mesh,
        left..=right,
        back.0 + back_radius,
        back_radius,
        [METAL, CHROME],
    );
    let ring = PLATEN_END_INCHES * ppi;
    let plate = SIDE_PLATE_INCHES * ppi;
    let radius = PLATEN_DIAMETER_INCHES * ppi / 2.0;
    let (inner_left, inner_right) = (left + plate, right - plate);
    let rubber = inner_left + ring..=inner_right - ring;
    roller(&mut mesh, rubber, typing_y, radius, [RUBBER, RUBBER_SHINE]);
    for x in [
        inner_left..=inner_left + ring,
        inner_right - ring..=inner_right,
    ] {
        roller(&mut mesh, x, typing_y, radius, [METAL, METAL_SHINE]);
    }
    painter.add(Shape::mesh(mesh));
    // The side plates, from the back down to the deck.
    for x in [left..=inner_left, inner_right..=right] {
        let side = Rect::from_x_y_ranges(x, back.0..=typing_y + SIDE_PLATE_BELOW * ppi);
        painter.rect(
            side,
            CornerRadius::same(3),
            CHROME,
            Stroke::new(1.0, EDGE),
            StrokeKind::Inside,
        );
    }
    // The dark throat between platen and cover, still, behind the sheet:
    // what shows of it is where no sheet is, and below the card holder.
    let (y, z) = THROAT_TOP;
    let throat = [
        [-6.0, y, z],
        [6.0, y, z],
        [6.0, 1.0, DECK_Z],
        [-6.0, 1.0, DECK_Z],
    ];
    eye.fill(painter, &throat, |_| DARK);
}

/// A horizontal cylinder from the front, lit from above: `[shade, shine]`.
fn roller(
    mesh: &mut Mesh,
    x: RangeInclusive<f32>,
    centre_y: f32,
    radius: f32,
    colours: [Color32; 2],
) {
    let light = LIGHT_DEGREES.to_radians();
    let row = |i: u16| {
        // From the top (0) round the front to the bottom (π).
        let around = PI * f32::from(i) / f32::from(ROLLER_BANDS);
        let lit = (around - light).cos().max(0.0).powi(3);
        let colour = colours[0].lerp_to_gamma(colours[1], lit);
        (centre_y - radius * around.cos(), colour)
    };
    for i in 0..ROLLER_BANDS {
        let ((top, top_colour), (bottom, bottom_colour)) = (row(i), row(i + 1));
        add_quad(
            mesh,
            [
                (pos2(*x.start(), top), top_colour),
                (pos2(*x.end(), top), top_colour),
                (pos2(*x.end(), bottom), bottom_colour),
                (pos2(*x.start(), bottom), bottom_colour),
            ],
        );
    }
}

/// Its shadows on the desk: cast away from the light, and the contact
/// shadow round its base.
fn paint_shadow(painter: &Painter, eye: &Eye) {
    let (cast, contact) = (footprint(eye, CAST_SHADOW), footprint(eye, (0.0, 0.0)));
    let (cast_blur, contact_blur) = (0.9 * eye.ppi, 0.15 * eye.ppi);
    painter.add(Shape::mesh(soft(
        &cast,
        cast_blur,
        Color32::from_black_alpha(80),
    )));
    painter.add(Shape::mesh(soft(
        &contact,
        contact_blur,
        Color32::from_black_alpha(110),
    )));
}

/// Where the body stands on the desk, moved `(x, y)` inches, on screen: its
/// front corners rounded as the case's are.
fn footprint(eye: &Eye, (dx, dy): (f32, f32)) -> Vec<Pos2> {
    let (half, back, front) = (PANEL_HALF_BOTTOM, BODY_BACK, BASE_FRONT);
    eye.polygon(&rounded(&[
        ([-half + dx, back + dy, DESK_Z], 0.0),
        ([half + dx, back + dy, DESK_Z], 0.0),
        ([half + dx, front + dy, DESK_Z], FRAME_ROUNDING),
        ([-half + dx, front + dy, DESK_Z], FRAME_ROUNDING),
    ]))
}

/// Convex `inner` in `colour`, fading to nothing `blur` points outside it.
fn soft(inner: &[Pos2], blur: f32, colour: Color32) -> Mesh {
    let n = inner.len() as u32;
    let centre = inner.iter().fold(Vec2::ZERO, |sum, p| sum + p.to_vec2()) / n as f32;
    let outward = |a: Pos2, b: Pos2| {
        let along = (b - a).normalized();
        let normal = vec2(along.y, -along.x);
        if (a.to_vec2() - centre).dot(normal) < 0.0 {
            -normal
        } else {
            normal
        }
    };
    let mut mesh = Mesh::default();
    for &p in inner {
        mesh.colored_vertex(p, colour);
    }
    for i in 1..n.saturating_sub(1) {
        mesh.add_triangle(0, i, i + 1);
    }
    let len = inner.len();
    for (i, &p) in inner.iter().enumerate() {
        let (before, after) = (inner[(i + len - 1) % len], inner[(i + 1) % len]);
        let normal = (outward(before, p) + outward(p, after)).normalized();
        mesh.colored_vertex(p + normal * blur, Color32::TRANSPARENT);
    }
    for i in 0..n {
        let j = (i + 1) % n;
        mesh.add_triangle(i, j, n + j);
        mesh.add_triangle(i, n + j, n + i);
    }
    mesh
}

/// In front of the sheet, after it: the card holder at the printing point,
/// the ribbon cover over the type bars, the front panel and the keyboard.
pub fn paint_front(painter: &Painter, view: Rect, metrics: &Metrics, typing_y: f32) {
    let eye = Eye::new(view, metrics, typing_y);
    paint_card_holder(painter, &eye, metrics);
    paint_ribbon_cover(painter, &eye);
    paint_panel(painter, &eye);
    paint_keyboard(painter, &eye);
}

/// The card holder: two translucent plates under the platen with the
/// aligning scale along their tops, a metal drawing notch at each inner end,
/// and the type guide's arch between them over the ribbon vibrator.
fn paint_card_holder(painter: &Painter, eye: &Eye, metrics: &Metrics) {
    let ppi = eye.ppi;
    let top = eye.at([0.0, TABLE_BACK.0, TABLE_BACK.1]).y;
    let bottom = eye.at([0.0, TABLE_FRONT.0, TABLE_FRONT.1]).y;
    for side in [-1.0, 1.0] {
        let (inner, outer) = (side * TABLE_X.0, side * TABLE_X.1);
        let outline = rounded(&[
            ([inner, TABLE_BACK.0, TABLE_BACK.1], 0.05),
            ([outer, TABLE_BACK.0, TABLE_BACK.1], 0.3),
            ([outer, TABLE_FRONT.0, TABLE_FRONT.1], 0.3),
            ([inner, TABLE_FRONT.0, TABLE_FRONT.1], 0.05),
        ]);
        eye.fill(painter, &outline, |_| GLASS);
        painter.add(Shape::closed_line(
            eye.polygon(&outline),
            Stroke::new(1.0, GLASS_EDGE),
        ));
        // The top edge catches the light.
        let edge = [
            [inner, TABLE_BACK.0, TABLE_BACK.1],
            [outer, TABLE_BACK.0, TABLE_BACK.1],
        ];
        eye.line(painter, &edge, 0.015, Color32::from_white_alpha(160));
    }

    // Marked on the columns' edges, so they line up with the type: every
    // fifth longer, counted from the printing point.
    let column = metrics.column_width;
    let from = (TABLE_X.0 * ppi / column).ceil() as i32;
    for k in from..from + ALIGNING_COLUMNS {
        let long = if k % 5 == 0 { 0.1 } else { 0.05 };
        for side in [-1.0, 1.0] {
            let x = eye.origin.x + side * (k as f32 - 0.5) * column;
            let tick = [pos2(x, top), pos2(x, top + long * ppi)];
            painter.line_segment(tick, Stroke::new(1.0, GLASS_EDGE));
        }
    }

    for side in [-1.0, 1.0] {
        let x = eye.origin.x + side * (TABLE_X.0 * ppi - NOTCH_INCHES.0 * ppi / 2.0);
        let notch = Rect::from_center_size(
            pos2(x, top + NOTCH_INCHES.1 * ppi / 2.0 - 0.03 * ppi),
            vec2(NOTCH_INCHES.0, NOTCH_INCHES.1) * ppi,
        );
        let mut mesh = Mesh::default();
        let (left, right) = (notch.left(), notch.right());
        roller(
            &mut mesh,
            left..=right,
            notch.center().y,
            notch.height() / 2.0,
            [METAL, METAL_SHINE],
        );
        painter.add(Shape::mesh(mesh));
        painter.rect_stroke(
            notch,
            CornerRadius::same(1),
            Stroke::new(1.0, EDGE),
            StrokeKind::Inside,
        );
        // The V a pencil rests in.
        let v = vec![
            pos2(notch.center().x - 0.05 * ppi, notch.top()),
            pos2(notch.center().x + 0.05 * ppi, notch.top()),
            pos2(notch.center().x, notch.top() + 0.08 * ppi),
        ];
        painter.add(Shape::convex_polygon(v, DARK, Stroke::NONE));
    }

    // The type guide: an arch just under the typing line, its legs going
    // down past the plates, the ribbon vibrator behind.
    let radius = TYPE_GUIDE_HALF * ppi;
    let arch_top = eye.origin.y + metrics.cell_size().y + 0.03 * ppi;
    let vibrator = Rect::from_x_y_ranges(
        eye.origin.x - VIBRATOR_HALF * ppi..=eye.origin.x + VIBRATOR_HALF * ppi,
        arch_top + radius..=bottom,
    );
    painter.rect_filled(vibrator, CornerRadius::same(2), DARK);
    let centre = pos2(eye.origin.x, arch_top + radius);
    let arch: Vec<Pos2> = std::iter::once(pos2(centre.x - radius, bottom))
        .chain((0..=12u8).map(|i| {
            let angle = PI + PI * f32::from(i) / 12.0;
            centre + radius * vec2(angle.cos(), angle.sin())
        }))
        .chain(std::iter::once(pos2(centre.x + radius, bottom)))
        .collect();
    let wire = 0.04 * ppi;
    painter.add(Shape::line(arch.clone(), Stroke::new(wire + 1.5, EDGE)));
    painter.add(Shape::line(arch, Stroke::new(wire, METAL_SHINE)));
}

/// A point on the ribbon cover's slope.
fn on_cover(x: f32, y: f32) -> [f32; 3] {
    let t = (y - COVER_BACK.0) / (COVER_FRONT.0 - COVER_BACK.0);
    [x, y, COVER_BACK.1 + t * (COVER_FRONT.1 - COVER_BACK.1)]
}

/// The type basket under the cover's opening: the type bars fanned out
/// wider than the opening, the cover cropping them, and the segment they
/// hang in: a teal core, a bronze band slotted for the bars, a silver rim.
fn paint_type_basket(painter: &Painter, eye: &Eye) {
    let centre = OPENING_BACK.0 - 0.15;
    let at =
        |angle: f32, radius: f32, z: f32| [radius * angle.sin(), centre + radius * angle.cos(), z];
    let bars = 52u16;
    let angles: Vec<f32> = (0..=bars)
        .map(|i| (-86.0 + 172.0 * f32::from(i) / f32::from(bars)).to_radians())
        .collect();
    for &angle in &angles {
        let (from, to) = (
            at(angle, SEGMENT_RIM, -0.85),
            at(angle, TYPE_BAR_REACH, -1.9),
        );
        eye.line(painter, &[from, to], 0.075, TYPE_BAR);
        // A streak down its lit edge, bright where it points to catch the light.
        let shine = streak(sub(to, from), 12);
        let aside = [-0.022 * angle.cos(), 0.022 * angle.sin(), 0.0];
        let edge = [add(from, aside), add(to, aside)];
        let colour = METAL_SHINE.gamma_multiply(0.12 + 0.88 * shine);
        eye.line(painter, &edge, 0.02, colour);
    }

    // The rim: brushed, so its streak runs round it.
    ring(
        painter,
        eye,
        centre,
        [SEGMENT_BAND, SEGMENT_RIM],
        |angle, _| {
            let along = [angle.cos(), -angle.sin(), 0.0];
            METAL.lerp_to_gamma(METAL_SHINE, streak(along, 6))
        },
    );
    ring(
        painter,
        eye,
        centre,
        [SEGMENT_CORE, SEGMENT_BAND],
        |_, radius| {
            let t = (radius - SEGMENT_CORE) / (SEGMENT_BAND - SEGMENT_CORE);
            BRONZE.lerp_to_gamma(BRONZE_DARK, t)
        },
    );
    for &angle in &angles {
        let slot = [
            at(angle, SEGMENT_CORE + 0.12, SEGMENT_Z),
            at(angle, SEGMENT_BAND - 0.06, SEGMENT_Z),
        ];
        eye.line(painter, &slot, 0.018, BRONZE_DARK);
    }
    speckle(painter, eye, 140, 0xB2_0A5E, |u, v| {
        let angle = (u - 0.5) * PI;
        at(
            angle,
            SEGMENT_CORE + v * (SEGMENT_BAND - SEGMENT_CORE),
            SEGMENT_Z,
        )
    });
    ring(painter, eye, centre, [0.0, SEGMENT_CORE], |_, radius| {
        TEAL_LIGHT.lerp_to_gamma(TEAL, radius / SEGMENT_CORE)
    });
    speckle(painter, eye, 90, 0x7E_A1, |u, v| {
        at((u - 0.5) * PI, v.sqrt() * SEGMENT_CORE * 0.97, SEGMENT_Z)
    });
    let edge: Vec<[f32; 3]> = (0..=36u8)
        .map(|i| {
            at(
                (-90.0 + 5.0 * f32::from(i)).to_radians(),
                SEGMENT_CORE,
                SEGMENT_Z,
            )
        })
        .collect();
    eye.line(painter, &edge, 0.025, PALE_RING);
}

/// The front half of an annulus round `(0, centre_y)` at the segment's
/// height, between `radii`; `colour` by angle and radius.
fn ring(
    painter: &Painter,
    eye: &Eye,
    centre_y: f32,
    [inner, outer]: [f32; 2],
    colour: impl Fn(f32, f32) -> Color32,
) {
    let at = |angle: f32, radius: f32| {
        [
            radius * angle.sin(),
            centre_y + radius * angle.cos(),
            SEGMENT_Z,
        ]
    };
    let steps = 40u8;
    let mut mesh = Mesh::default();
    for i in 0..steps {
        let [a, b] =
            [i, i + 1].map(|k| (-90.0 + 180.0 * f32::from(k) / f32::from(steps)).to_radians());
        let corner = |angle: f32, radius: f32| (eye.at(at(angle, radius)), colour(angle, radius));
        add_quad(
            &mut mesh,
            [
                corner(a, inner),
                corner(b, inner),
                corner(b, outer),
                corner(a, outer),
            ],
        );
    }
    painter.add(Shape::mesh(mesh));
}

/// `count` specks of grain, light and dark, at `place(u, v)` for seeded
/// `u` and `v` in 0..=1.
fn speckle(
    painter: &Painter,
    eye: &Eye,
    count: u64,
    seed: u64,
    place: impl Fn(f32, f32) -> [f32; 3],
) {
    for i in 0..count {
        let bits = splitmix64(seed ^ i);
        let p = place(unit(bits, 0), unit(bits, 16));
        let colour = if unit(bits, 32) < 0.5 {
            Color32::from_white_alpha(40)
        } else {
            Color32::from_black_alpha(50)
        };
        let radius = (0.012 * eye.scale(p)).max(0.6);
        painter.circle_filled(eye.at(p), radius, colour);
    }
}

/// Toward the light from the machine: to the writer's left, above, a little
/// in front.
fn toward_light() -> [f32; 3] {
    normalized([-0.6, 0.4, 0.7])
}

/// Toward the seated eye from the machine.
fn toward_eye() -> [f32; 3] {
    let (sin, cos) = EYE_TILT_DEGREES.to_radians().sin_cos();
    [0.0, cos, sin]
}

/// Plastic facing `normal`: lit over an ambient floor (Lambert).
fn matte(colour: Color32, normal: [f32; 3]) -> Color32 {
    let lit = dot(normalized(normal), toward_light()).max(0.0);
    brighten(colour, 0.7 + 0.4 * lit)
}

/// How bright a thin metal part running along `tangent` catches the light,
/// 0..=1: brushed and milled metal streaks along its grain
/// (Heidrich–Seidel). `sharpness` narrows the streak.
fn streak(tangent: [f32; 3], sharpness: i32) -> f32 {
    let tangent = normalized(tangent);
    let (lt, vt) = (dot(toward_light(), tangent), dot(toward_eye(), tangent));
    let across = (1.0 - lt * lt).max(0.0).sqrt() * (1.0 - vt * vt).max(0.0).sqrt();
    (across - lt * vt).max(0.0).powi(sharpness)
}

fn brighten(colour: Color32, by: f32) -> Color32 {
    // Safe casts: clamped to a byte.
    let channel = |c: u8| (f32::from(c) * by).clamp(0.0, 255.0) as u8;
    Color32::from_rgba_premultiplied(
        channel(colour.r()),
        channel(colour.g()),
        channel(colour.b()),
        colour.a(),
    )
}

fn dot(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn add(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

fn sub(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn normalized(v: [f32; 3]) -> [f32; 3] {
    let length = dot(v, v).sqrt().max(1e-6);
    [v[0] / length, v[1] / length, v[2] / length]
}

/// The cover: two plates, their tips rounded thick where the opening runs
/// out at the back, and the strip in front; the type bars in the opening;
/// the plates' thickness where they drop into it.
fn paint_ribbon_cover(painter: &Painter, eye: &Eye) {
    let (ob, of) = (OPENING_BACK, OPENING_FRONT);
    let (back, front) = COVER_HALF;
    let (bl, br) = (on_cover(-back, COVER_BACK.0), on_cover(back, COVER_BACK.0));
    let (fl, fr) = (
        on_cover(-front, COVER_FRONT.0),
        on_cover(front, COVER_FRONT.0),
    );
    let (tip_l, tip_r) = (on_cover(-ob.1, ob.0), on_cover(ob.1, ob.0));
    let (ofl, ofr) = (on_cover(-of.1, of.0), on_cover(of.1, of.0));
    eye.fill(painter, &[tip_l, tip_r, ofr, ofl], |_| INSIDE);
    paint_type_basket(painter, eye);
    paint_opening_shade(painter, eye, [tip_l, tip_r, ofr, ofl]);

    // The plates' inner edges: round each tip, then down to the front.
    let left_edge: Vec<[f32; 3]> = fillet(bl, tip_l, ofl, PLATE_TIP)
        .into_iter()
        .chain([ofl])
        .collect();
    let right_edge: Vec<[f32; 3]> = fillet(br, tip_r, ofr, PLATE_TIP)
        .into_iter()
        .chain([ofr])
        .collect();
    for (edge, facing) in [(&left_edge, 1.0), (&right_edge, -1.0)] {
        paint_plate_wall(painter, eye, edge, facing);
    }
    let lit = |[_, y, _]: [f32; 3]| {
        let t = (y - COVER_BACK.0) / (COVER_FRONT.0 - COVER_BACK.0);
        IVORY_SHADE.lerp_to_gamma(IVORY_LIT, t)
    };
    let left: Vec<[f32; 3]> = fillet(fl, bl, tip_l, 0.25)
        .into_iter()
        .chain(left_edge.iter().copied())
        .chain([fl])
        .collect();
    let right: Vec<[f32; 3]> = fillet(fr, br, tip_r, 0.25)
        .into_iter()
        .chain(right_edge.iter().copied())
        .chain([fr])
        .collect();
    // Square at the fold: the panel below meets it edge for edge.
    for piece in [left, right, vec![ofl, ofr, fr, fl]] {
        eye.fill(painter, &piece, lit);
    }
    // The opening's front corners rounded: cover fills them in.
    for (corner, before, after) in [(ofl, tip_l, ofr), (ofr, tip_r, ofl)] {
        let fill: Vec<[f32; 3]> = std::iter::once(corner)
            .chain(fillet(before, corner, after, OPENING_CORNER))
            .collect();
        eye.fill(painter, &fill, lit);
    }
    // The rounded top of the plates' edges catches the light.
    for edge in [&left_edge, &right_edge] {
        eye.line(painter, edge, 0.03, Color32::from_white_alpha(150));
    }
    let outline = [
        fillet(fl, bl, br, 0.25),
        fillet(bl, br, fr, 0.25),
        vec![fr, fl],
    ]
    .concat();
    eye.outline(painter, &outline);
}

/// A plate's thickness under its inner `edge`, facing into the opening
/// (`facing` +1 right, -1 left): lit as it faces.
fn paint_plate_wall(painter: &Painter, eye: &Eye, edge: &[[f32; 3]], facing: f32) {
    let mut mesh = Mesh::default();
    for pair in edge.windows(2) {
        let [a, b] = [pair[0], pair[1]];
        let along = sub(b, a);
        // Across the edge, the way it faces.
        let across = if along[1] * facing >= 0.0 {
            [along[1], -along[0]]
        } else {
            [-along[1], along[0]]
        };
        let colour = matte(IVORY_SHADE, [across[0], across[1], 0.2]);
        let down = |p: [f32; 3]| [p[0], p[1], p[2] - PLATE_THICKNESS];
        add_quad(
            &mut mesh,
            [
                (eye.at(a), colour),
                (eye.at(b), colour),
                (eye.at(down(b)), brighten(colour, 0.7)),
                (eye.at(down(a)), brighten(colour, 0.7)),
            ],
        );
    }
    painter.add(Shape::mesh(mesh));
}

/// The cover's shadow just inside the `opening`'s sides and front edge: its
/// back is open.
fn paint_opening_shade(painter: &Painter, eye: &Eye, opening: [[f32; 3]; 4]) {
    let [obl, obr, ofr, ofl] = opening;
    let reach = OPENING_SHADE_INCHES;
    let dark = Color32::from_black_alpha(190);
    let mut mesh = Mesh::default();
    // Each edge, and which way is inside from it.
    for (a, b, inward) in [
        (obl, ofl, [reach, 0.0]),
        (obr, ofr, [-reach, 0.0]),
        (ofl, ofr, [0.0, -0.6 * reach]),
    ] {
        let inside = |p: [f32; 3]| on_cover(p[0] + inward[0], p[1] + inward[1]);
        add_quad(
            &mut mesh,
            [
                (eye.at(a), dark),
                (eye.at(b), dark),
                (eye.at(inside(b)), Color32::TRANSPARENT),
                (eye.at(inside(a)), Color32::TRANSPARENT),
            ],
        );
    }
    painter.add(Shape::mesh(mesh));
}

/// The front panel, falling from the cover to the keyboard's opening.
fn paint_panel(painter: &Painter, eye: &Eye) {
    let (y0, z0) = COVER_FRONT;
    let (y1, z1) = PANEL_BOTTOM;
    let (top, bottom) = (COVER_HALF.1, PANEL_HALF_BOTTOM);
    let outline = [
        [-top, y0, z0],
        [top, y0, z0],
        [bottom, y1, z1],
        [-bottom, y1, z1],
    ];
    eye.fill(painter, &outline, |[_, y, _]| {
        IVORY_LIT.lerp_to_gamma(IVORY, (y - y0) / (y1 - y0))
    });
}

/// Where a row's key tops are: `(y, z)`, far row 0.
fn key_row(row: f32) -> (f32, f32) {
    (
        KEY_ROW.0 + row * KEY_ROW_STEP.0,
        KEY_ROW.1 + row * KEY_ROW_STEP.1,
    )
}

/// The side walls' top at `y`: easing down from the panel's foot, then
/// swooping down into the shelf, and level with it to the front.
fn wall_top(y: f32) -> f32 {
    let (y0, z0) = PANEL_BOTTOM;
    let (start, end) = WALL_SWOOP;
    if y <= start {
        let t = ((y - y0) / (start - y0)).clamp(0.0, 1.0);
        z0 + (WALL_EASE_Z - z0) * t
    } else {
        let t = ((y - start) / (end - start)).clamp(0.0, 1.0);
        WALL_EASE_Z + (SHELF.2 - WALL_EASE_Z) * t * t * (3.0 - 2.0 * t)
    }
}

/// The case's outer top edge on the right, back to front: straight along
/// the wall, then round its front corner by angle, so the curve is as fine
/// at its end as at its start. Its `x` alone: mirror it for the left.
fn case_side() -> Vec<[f32; 3]> {
    let (y0, _) = PANEL_BOTTOM;
    let (front, r, outer) = (SHELF.1, FRAME_ROUNDING, PANEL_HALF_BOTTOM);
    let straight = (0..64u8).map(|i| y0 + (front - r - y0) * f32::from(i) / 64.0);
    let straight = straight.map(|y| (outer, y));
    let corner = (0..=24u8).map(|i| {
        let angle = FRAC_PI_2 * f32::from(i) / 24.0;
        (outer - r + r * angle.cos(), front - r + r * angle.sin())
    });
    straight
        .chain(corner)
        .map(|(x, y)| [x, y, wall_top(y)])
        .collect()
}

/// The case's front edge, right to left, round its rounded front corners
/// at the walls' height: never its back edge.
fn case_front_edge() -> Vec<[f32; 3]> {
    let corner: Vec<[f32; 3]> = case_side()
        .into_iter()
        .filter(|p| p[1] >= SHELF.1 - FRAME_ROUNDING - 1e-4)
        .collect();
    let left = corner.iter().rev().map(|&[x, y, z]| [-x, y, z]);
    corner.iter().copied().chain(left).collect()
}

/// The top of a well wall's inner face, back to front, `from` to `to` (`y`):
/// `side` -1 left, +1 right.
fn inner_edge(side: f32, from: f32, to: f32) -> Vec<[f32; 3]> {
    (0..=16u8)
        .map(|i| {
            let y = from + (to - from) * f32::from(i) / 16.0;
            [side * OPENING_HALF, y, wall_top(y)]
        })
        .collect()
}

/// The keyboard deep in its well: the key bed, the levers' guide plate and
/// the shadows in it; the levers running back under the rows; the keys, far
/// row first; then the case round them: the thin side walls, their tops
/// swooping down into the shelf flush with the space bar, which sits in a
/// notch in it; the case's rounded front and the plinth.
fn paint_keyboard(painter: &Painter, eye: &Eye) {
    let (y0, z0) = PANEL_BOTTOM;
    let half = OPENING_HALF;
    let (shelf_back, case_front, shelf_z) = SHELF;
    let (notch, notch_front) = NOTCH;
    let bed = KEY_BED_Z;
    // One floor under the well and the notch, reaching under the shelf: the
    // shelf's rounded lip lets the eye in past its corner.
    let floor = [
        [-half, y0, bed],
        [half, y0, bed],
        [half, notch_front, bed],
        [-half, notch_front, bed],
    ];
    eye.fill(painter, &floor, |[_, y, _]| {
        KEY_BED.lerp_to_gamma(KEY_BED_FRONT, (y - y0) / (notch_front - y0))
    });
    // The machine's insides under the panel's edge.
    let under = z0 - PANEL_EDGE_INCHES;
    let inside = [
        [-half, y0, under],
        [half, y0, under],
        [half, y0, bed],
        [-half, y0, bed],
    ];
    eye.fill(painter, &inside, |_| KEY_BED);
    paint_keyboard_shade(painter, eye);
    // The rod behind the far row that the key levers rest on.
    let levers = key_levers();
    paint_rod(painter, eye, key_row(0.0).0 - ROD_BEHIND, &levers);

    // The walls' inner faces, down to the bed along the well and to the
    // shelf beside it; the notch's sides down to the bed.
    let mut mesh = Mesh::default();
    for side in [-1.0, 1.0] {
        // The notch's side follows the shelf's edge round its rounded lip.
        let at = |x: f32, y: f32| [x, y, shelf_z];
        let notch_side: Vec<[f32; 3]> = std::iter::once(at(side * half, shelf_back))
            .chain(fillet(
                at(side * half, shelf_back),
                at(side * notch, shelf_back),
                at(side * notch, notch_front),
                SHELF_LIP,
            ))
            .chain([at(side * notch, notch_front)])
            .collect();
        // Each face's path, what it drops to, and its shade at top and foot:
        // deep down to the bed; only a step down to the shelf; the notch's
        // sides in the shelf's shade.
        for (path, bed, [top, foot]) in [
            (inner_edge(side, y0, shelf_back), bed, [1.0, 0.55]),
            (
                inner_edge(side, shelf_back, case_front),
                shelf_z,
                [1.0, 0.9],
            ),
            (notch_side, bed, [0.75, 0.38]),
        ] {
            for pair in path.windows(2) {
                let [a, b] = [pair[0], pair[1]];
                let along = sub(b, a);
                let facing = [-side * along[1], side * along[0], 0.0];
                if !Eye::sees(a, facing) {
                    continue;
                }
                let lit = matte(IVORY_SHADE, facing);
                let (colour, low) = (brighten(lit, top), brighten(lit, foot));
                add_quad(
                    &mut mesh,
                    [
                        (eye.at(a), colour),
                        (eye.at(b), colour),
                        (eye.at([b[0], b[1], bed]), low),
                        (eye.at([a[0], a[1], bed]), low),
                    ],
                );
            }
        }
    }
    painter.add(Shape::mesh(mesh));

    // Every cap's shadow, then every lever, then the caps far row first:
    // the levers run back under the rows behind them.
    let (space_y, space_z) = SPACE_ROW;
    let depth = KEY_CAP.1 / 2.0;
    for (row, keys) in KEYS.iter().enumerate() {
        let (y, z) = key_row(row as f32);
        for key in keys.iter() {
            let half = KEY_CAP.0 * key.width / 2.0;
            let x = [key.x() - half, key.x() + half];
            paint_key_shadow(painter, eye, x, [y - depth, y + depth], z);
        }
    }
    for (left, right, _) in SPACE_BAR {
        let span = [space_y - 0.2, space_y + 0.2];
        paint_key_shadow(painter, eye, [left, right], span, space_z);
    }
    for &lever in &levers {
        paint_lever(painter, eye, lever);
    }
    paint_side_controls(painter, eye);
    // The panel's edge over the well: the levers go in under it.
    let edge = [
        [-half, y0, z0],
        [half, y0, z0],
        [half, y0, under],
        [-half, y0, under],
    ];
    eye.fill(painter, &edge, |_| IVORY_SHADE);

    for (row, keys) in KEYS.iter().enumerate() {
        let (y, z) = key_row(row as f32);
        for key in keys.iter() {
            paint_key(painter, eye, key, [key.x(), y, z]);
        }
    }
    for (left, right, name) in SPACE_BAR {
        let (back, front) = (space_y - 0.2, space_y + 0.2);
        paint_cap(
            painter,
            eye,
            [left, right],
            [back, front],
            space_z,
            [CAP, CAP_FRONT],
        );
        legend(
            painter,
            eye,
            [(left + right) / 2.0, space_y, space_z],
            name,
            "",
            0.55,
        );
    }

    // The walls' tops, lit as they swoop down to the case's front corners:
    // across from each point of the case's outer edge to the well.
    let outline = case_side();
    let mut mesh = Mesh::default();
    for side in [-1.0, 1.0] {
        for pair in outline.windows(2) {
            let [a, b] = [pair[0], pair[1]];
            let colour = matte(IVORY, [0.0, a[2] - b[2], b[1] - a[1]]);
            let (outer_a, outer_b) = ([side * a[0], a[1], a[2]], [side * b[0], b[1], b[2]]);
            add_quad(
                &mut mesh,
                [
                    (eye.at([side * half, a[1], a[2]]), colour),
                    (eye.at(outer_a), colour),
                    (eye.at(outer_b), colour),
                    (eye.at([side * half, b[1], b[2]]), colour),
                ],
            );
        }
    }
    painter.add(Shape::mesh(mesh));
    // The shelf between the walls: a block each side, rounding off into the
    // well beside the notch, and the strip in front of the notch.
    let shelf_colour = matte(IVORY, [0.0, 0.0, 1.0]);
    let at = |x: f32, y: f32| [x, y, shelf_z];
    for side in [-1.0, 1.0] {
        let block = rounded(&[
            (at(side * half, shelf_back), 0.0),
            (at(side * notch, shelf_back), SHELF_LIP),
            (at(side * notch, case_front), 0.0),
            (at(side * half, case_front), 0.0),
        ]);
        eye.fill(painter, &block, |_| shelf_colour);
        let corner = at(side * notch, notch_front);
        let fill: Vec<[f32; 3]> = std::iter::once(corner)
            .chain(fillet(
                at(side * notch, shelf_back),
                corner,
                at(-side * notch, notch_front),
                NOTCH_ROUNDING,
            ))
            .collect();
        eye.fill(painter, &fill, |_| shelf_colour);
    }
    let strip = [
        at(-notch, notch_front),
        at(notch, notch_front),
        at(notch, case_front),
        at(-notch, case_front),
    ];
    eye.fill(painter, &strip, |_| shelf_colour);

    // The case's rounded front, its face turning with the corners, and the
    // plinth under it.
    let front = case_front_edge();
    // Each point faces between its two neighbours: the shading blends round
    // the corners instead of stepping.
    let facing: Vec<[f32; 3]> = (0..front.len())
        .map(|i| {
            let before = front[i.saturating_sub(1)];
            let after = front[(i + 1).min(front.len() - 1)];
            let along = sub(after, before);
            [along[1], -along[0], 0.0]
        })
        .collect();
    let mut mesh = Mesh::default();
    for i in 0..front.len() - 1 {
        let (a, b) = (front[i], front[i + 1]);
        // Round the corners it turns away onto the sides: skip that.
        let along = sub(b, a);
        if !Eye::sees(a, [along[1], -along[0], 0.0]) {
            continue;
        }
        let [top_a, top_b] = [i, i + 1].map(|k| matte(IVORY, facing[k]));
        let [low_a, low_b] = [i, i + 1].map(|k| brighten(matte(IVORY_SHADE, facing[k]), 0.85));
        let [base_a, base_b] = [i, i + 1].map(|k| matte(PLINTH, facing[k]));
        let at = |p: [f32; 3], z: f32| eye.at([p[0], p[1], z]);
        add_quad(
            &mut mesh,
            [
                (eye.at(a), top_a),
                (eye.at(b), top_b),
                (at(b, PLINTH_TOP), low_b),
                (at(a, PLINTH_TOP), low_a),
            ],
        );
        add_quad(
            &mut mesh,
            [
                (at(a, PLINTH_TOP), base_a),
                (at(b, PLINTH_TOP), base_b),
                (at(b, DESK_Z), base_b),
                (at(a, DESK_Z), base_a),
            ],
        );
    }
    painter.add(Shape::mesh(mesh));
    for side in [-1.0, 1.0] {
        let edge: Vec<[f32; 3]> = outline.iter().map(|&[x, y, z]| [side * x, y, z]).collect();
        eye.line(painter, &edge, 0.012, EDGE);
    }
    paint_selector_marks(painter, eye);
}

/// In the gaps between the keys and the walls: on the left, the carriage
/// lock by the far row, standing above it, and the touch control's selector
/// by the third, midway between less and more; on the right, the ribbon
/// colour selector at its setting. Each selector's paddle stands up level
/// with its marks on the wall. Before the caps: the nearer keys hide what's
/// behind them.
fn paint_side_controls(painter: &Painter, eye: &Eye) {
    let (y, z) = key_row(0.0);
    paint_lock(painter, eye, [-LOCK_X, y - LOCK_BACK, z + LOCK_RISE]);
    let (y, _) = key_row(2.0);
    let ribbon = y + RIBBON_ALONG[RIBBON_SET];
    for (x, paddle_y) in [(-SELECTOR_X, y), (SELECTOR_X, ribbon)] {
        let top = wall_top(paddle_y) - PADDLE_BELOW_WALL;
        paint_selector(painter, eye, [x, paddle_y - SELECTOR_REACH, top]);
    }
}

/// The selectors' marks on the walls' tops beside them: a bold less and more
/// on the left; on the right, the ribbon's colours back to front, each a
/// small rounded square standing proud of the case, outlined dark.
fn paint_selector_marks(painter: &Painter, eye: &Eye) {
    let on_wall = (OPENING_HALF + PANEL_HALF_BOTTOM) / 2.0;
    let (y, _) = key_row(2.0);
    let (along, half, stroke) = TOUCH_MARKS;
    let bar = |centre: f32, upright: bool| {
        let (dx, dy) = if upright { (0.0, half) } else { (half, 0.0) };
        [[-on_wall - dx, centre - dy], [-on_wall + dx, centre + dy]]
            .map(|[x, y]| [x, y, wall_top(y) + 0.002])
    };
    for part in [
        bar(y - along, false),
        bar(y + along, false),
        bar(y + along, true),
    ] {
        eye.line(painter, &part, stroke, ENGRAVED);
    }
    let (size, round, proud) = RIBBON_MARK;
    for (colour, along) in RIBBON_MARKS.into_iter().zip(RIBBON_ALONG) {
        let (x, y) = (on_wall, y + along);
        let z = wall_top(y) + proud;
        let square = |z: f32| {
            rounded(&[
                ([x - size, y - size, z], round),
                ([x + size, y - size, z], round),
                ([x + size, y + size, z], round),
                ([x - size, y + size, z], round),
            ])
        };
        // Its front standing off the case, then its face.
        let foot = z - proud;
        let front = [
            [x - size, y + size, z],
            [x + size, y + size, z],
            [x + size, y + size, foot],
            [x - size, y + size, foot],
        ];
        eye.fill(painter, &front, |_| brighten(colour, 0.6));
        let face = square(z);
        eye.fill(painter, &face, |_| matte(colour, [0.0, 0.0, 1.0]));
        painter.add(Shape::closed_line(
            eye.polygon(&face),
            Stroke::new(1.0, MARK_EDGE),
        ));
    }
}

/// A steel rod along `path`, a streak of light down its lit side.
fn paint_steel(painter: &Painter, eye: &Eye, path: &[[f32; 3]]) {
    eye.line(painter, path, 0.05, STEM);
    let shine = streak([0.0, 0.0, 1.0], 4).max(0.35);
    let lit: Vec<[f32; 3]> = path.iter().map(|&[x, y, z]| [x - 0.018, y, z]).collect();
    eye.line(painter, &lit, 0.014, STEM_SHINE.gamma_multiply(shine));
}

/// From `foot` straight back, level, into the machine under the panel.
fn into_machine(foot: [f32; 3]) -> [f32; 3] {
    [foot[0], PANEL_BOTTOM.0 + 0.02, foot[2]]
}

/// The carriage lock: a lever whose green cap's top is at `top`, its post
/// running on down below the cap, then back into the machine.
fn paint_lock(painter: &Painter, eye: &Eye, top: [f32; 3]) {
    let [x, y, z] = top;
    let foot = [x, y, z - KEY_FRONT - LOCK_DROP];
    paint_steel(
        painter,
        eye,
        &[[x, y, z - KEY_FRONT], foot, into_machine(foot)],
    );
    let (half, depth) = LOCK_CAP;
    let colours = [SHIFT_CAP, SHIFT_FRONT];
    paint_cap(
        painter,
        eye,
        [x - half, x + half],
        [y - depth, y + depth],
        z,
        colours,
    );
}

/// A selector: a post at `post` (its paddle's top level) up from a rod out
/// of the machine, bending toward the writer to an upright chrome paddle
/// facing them, lit in bands as polished metal is.
fn paint_selector(painter: &Painter, eye: &Eye, post: [f32; 3]) {
    let [x, y, top] = post;
    let (half, height) = PADDLE;
    let bend = [x, y, top - height];
    // Its post runs well down, then back into the machine under the lock's.
    let foot = [x, y, bend[2] - SELECTOR_DROP];
    paint_steel(painter, eye, &[bend, foot, into_machine(foot)]);
    let paddle_y = y + SELECTOR_REACH;
    eye.line(painter, &[bend, [x, paddle_y, top - height]], 0.05, STEM);
    // Chrome mirrors its surroundings: bright sky above, dark room below,
    // a bright band where it turns to the light.
    let bands = [
        (0.0, METAL_SHINE),
        (0.3, brighten(METAL, 0.8)),
        (0.55, Color32::WHITE),
        (0.75, METAL),
        (1.0, brighten(METAL, 0.6)),
    ];
    let mut mesh = Mesh::default();
    for pair in bands.windows(2) {
        let [(from, upper), (to, lower)] = [pair[0], pair[1]];
        let at = |u: f32, t: f32| eye.at([x + u, paddle_y, top - height * t]);
        add_quad(
            &mut mesh,
            [
                (at(-half, from), upper),
                (at(half, from), upper),
                (at(half, to), lower),
                (at(-half, to), lower),
            ],
        );
    }
    painter.add(Shape::mesh(mesh));
    let rim = [
        [x - half, paddle_y - 0.03, top],
        [x + half, paddle_y - 0.03, top],
        [x + half, paddle_y, top],
        [x - half, paddle_y, top],
    ];
    eye.fill(painter, &rim, |_| METAL_SHINE);
    let outline = [
        [x - half, paddle_y, top],
        [x + half, paddle_y, top],
        [x + half, paddle_y, top - height],
        [x - half, paddle_y, top - height],
    ];
    eye.outline(painter, &outline);
}

/// Shade on the key bed at the foot of the walls and under the panel.
fn paint_keyboard_shade(painter: &Painter, eye: &Eye) {
    let (y0, _) = PANEL_BOTTOM;
    let (half, reach, bed) = (OPENING_HALF, 0.5, KEY_BED_Z);
    let dark = Color32::from_black_alpha(170);
    let mut mesh = Mesh::default();
    for (a, b, inward) in [
        ((-half, y0), (half, y0), (0.0, reach)),
        ((-half, y0), (-half, SHELF.0), (reach, 0.0)),
        ((half, y0), (half, SHELF.0), (-reach, 0.0)),
    ] {
        let at = |(x, y): (f32, f32)| eye.at([x, y, bed]);
        let inside = |(x, y): (f32, f32)| at((x + inward.0, y + inward.1));
        add_quad(
            &mut mesh,
            [
                (at(a), dark),
                (at(b), dark),
                (inside(b), Color32::TRANSPARENT),
                (inside(a), Color32::TRANSPARENT),
            ],
        );
    }
    painter.add(Shape::mesh(mesh));
}

/// Under every cap, where its post starts: the keys, far row first, then the
/// space bar's.
fn key_levers() -> Vec<[f32; 3]> {
    let (space_y, space_z) = SPACE_ROW;
    let keys = KEYS.iter().enumerate().flat_map(|(row, keys)| {
        let (y, z) = key_row(row as f32);
        keys.iter()
            .map(move |key| [key.x(), y - 0.1, z - KEY_FRONT])
    });
    let space = [-3.98, -1.8, 1.8, 3.88].map(|x| [x, space_y - 0.1, space_z - KEY_FRONT]);
    keys.chain(space).collect()
}

/// A lever's foot, where its post ends, and its end under the panel.
fn lever_path(under_cap: [f32; 3]) -> [[f32; 3]; 2] {
    let [x, y, z] = under_cap;
    let (y0, z0) = PANEL_BOTTOM;
    [
        [x, y, z - STEM_DROP],
        [x * LEVER_CONVERGE, y0 + 0.02, z0 - 0.3],
    ]
}

/// Where the lever from `under_cap` crosses depth `y`, if it does.
fn lever_at(under_cap: [f32; 3], y: f32) -> Option<[f32; 3]> {
    let [foot, end] = lever_path(under_cap);
    let t = (foot[1] - y) / (foot[1] - end[1]);
    (0.0..=1.0).contains(&t).then(|| lerp3(foot, end, t))
}

/// Where the rod the levers rest on lies at depth `y`: its axis's `z`, just
/// under the lowest lever crossing there. `None` if none crosses.
fn rod_axis(y: f32, levers: &[[f32; 3]]) -> Option<f32> {
    let low = levers
        .iter()
        .filter_map(|&l| lever_at(l, y))
        .map(|p| p[2])
        .reduce(f32::min)?;
    Some(low - ROD_GAP - ROD_RADIUS)
}

/// A steel rod across the well from wall to wall at depth `y`, under the
/// `levers` crossing there: a cylinder lit round its curve, its top in the
/// light, a highlight where it turns toward it, its underside in shade.
fn paint_rod(painter: &Painter, eye: &Eye, y: f32, levers: &[[f32; 3]]) {
    let Some(z) = rod_axis(y, levers) else {
        return;
    };
    let half = OPENING_HALF;
    let light = toward_light();
    let halfway = normalized(add(light, toward_eye()));
    // Round the side the eye sees: from behind its top to under its front.
    let bands = 16u8;
    let around: Vec<([f32; 3], Color32)> = (0..=bands)
        .map(|i| {
            let angle = (-60.0 + 190.0 * f32::from(i) / f32::from(bands)).to_radians();
            let normal = [0.0, angle.sin(), angle.cos()];
            let lit = dot(normal, light).max(0.0);
            let shine = dot(normal, halfway).max(0.0).powi(24);
            let colour = brighten(ROD, 0.5 + 0.6 * lit).lerp_to_gamma(METAL_SHINE, shine);
            let offset = [0.0, ROD_RADIUS * angle.sin(), ROD_RADIUS * angle.cos()];
            (add([0.0, y, z], offset), colour)
        })
        .collect();
    let mut mesh = Mesh::default();
    for pair in around.windows(2) {
        let [(a, colour_a), (b, colour_b)] = [pair[0], pair[1]];
        let at = |p: [f32; 3], x: f32| eye.at([x, p[1], p[2]]);
        add_quad(
            &mut mesh,
            [
                (at(a, -half), colour_a),
                (at(a, half), colour_a),
                (at(b, half), colour_b),
                (at(b, -half), colour_b),
            ],
        );
    }
    painter.add(Shape::mesh(mesh));
}

/// A key's flat steel post down from under its cap, its spring coiled
/// round it, lit in front and dark behind; and its lever running back from
/// the post under the rows behind, into the machine under the panel, a
/// streak of light along it.
fn paint_lever(painter: &Painter, eye: &Eye, under_cap: [f32; 3]) {
    let [x, y, z] = under_cap;
    let [foot, end] = lever_path(under_cap);
    eye.line(painter, &[under_cap, foot, end], 0.05, STEM);
    let shine = streak(sub(end, foot), 10).max(0.25);
    let lit = [
        [x - 0.018, y, z],
        [x - 0.018, foot[1], foot[2]],
        [end[0] - 0.018, end[1], end[2]],
    ];
    eye.line(painter, &lit, 0.014, STEM_SHINE.gamma_multiply(shine));
    let turns = 4u8;
    let coil: Vec<[f32; 3]> = (0..=2 * turns)
        .map(|i| {
            let t = 0.1 + 0.8 * f32::from(i) / f32::from(2 * turns);
            let out = if i % 2 == 0 { -0.07 } else { 0.07 };
            [x + out, y, z - STEM_DROP * t]
        })
        .collect();
    // Strands crossing in front go one way, those behind the other.
    for (i, strand) in coil.windows(2).enumerate() {
        let colour = if i % 2 == 0 { SPRING_LIT } else { SPRING_DARK };
        eye.line(painter, strand, 0.022, colour);
    }
}

/// A cap's shadow on the key bed below it, over `x` and `y` ranges, its top
/// at `z`.
fn paint_key_shadow(
    painter: &Painter,
    eye: &Eye,
    [left, right]: [f32; 2],
    [back, front]: [f32; 2],
    z: f32,
) {
    let light = toward_light();
    let cast = KEY_SHADOW_DROP / light[2];
    let (dx, dy) = (-light[0] * cast, -light[1] * cast);
    let z = z - KEY_SHADOW_DROP;
    let round = 0.15;
    let outline = eye.polygon(&rounded(&[
        ([left + dx, back + dy, z], round),
        ([right + dx, back + dy, z], round),
        ([right + dx, front + dy, z], round),
        ([left + dx, front + dy, z], round),
    ]));
    let blur = 0.1 * eye.scale([0.0, front, z]);
    painter.add(Shape::mesh(soft(
        &outline,
        blur,
        Color32::from_black_alpha(150),
    )));
}

/// A key's cap, its top's centre at `top`, with its legends.
fn paint_key(painter: &Painter, eye: &Eye, key: &Key, top: [f32; 3]) {
    let [x, y, z] = top;
    let half = KEY_CAP.0 * key.width / 2.0;
    let depth = KEY_CAP.1 / 2.0;
    let colours = if key.shift {
        [SHIFT_CAP, SHIFT_FRONT]
    } else {
        [CAP, CAP_FRONT]
    };
    paint_cap(
        painter,
        eye,
        [x - half, x + half],
        [y - depth, y + depth],
        z,
        colours,
    );
    let size = if key.legend.chars().count() > 1 {
        0.6
    } else {
        1.0
    };
    legend(painter, eye, top, key.legend, key.shifted, size);
}

/// A rounded cap over `x` and `y` ranges, its top at `z`: its body the
/// outline round its rounded top and its flared, rounded foot, darkening
/// toward the foot and away from the light; its dished top over it.
fn paint_cap(
    painter: &Painter,
    eye: &Eye,
    [left, right]: [f32; 2],
    [back, front]: [f32; 2],
    z: f32,
    [top_colour, front_colour]: [Color32; 2],
) {
    let outline = |z: f32, grow: f32| {
        let (l, r, b, f) = (left - grow, right + grow, back - grow, front + grow);
        // The foot rounder than the top: moulded plastic.
        let round = 0.12 + 2.0 * grow;
        rounded(&[
            ([l, b, z], round),
            ([r, b, z], round),
            ([r, f, z], round),
            ([l, f, z], round),
        ])
    };
    let top = outline(z, 0.0);
    let foot = outline(z - KEY_FRONT, KEY_FLARE);
    let body = hull(
        eye.polygon(&top)
            .into_iter()
            .chain(eye.polygon(&foot))
            .collect(),
    );
    let middle = (left + right) / 2.0;
    let (rim, base) = (
        eye.at([middle, front, z]),
        eye.at([middle, front + KEY_FLARE, z - KEY_FRONT]),
    );
    let half_width = (eye.at([right, front, z]).x - rim.x).max(1.0);
    let lit_front = matte(front_colour, [0.0, 1.0, 0.0]);
    let edge = Shape::closed_line(body.clone(), Stroke::new(1.0, brighten(front_colour, 0.8)));
    let mesh = convex_mesh(&body, |pos| {
        let down = ((pos.y - rim.y) / (base.y - rim.y).max(1.0)).clamp(0.0, 1.0);
        // Away from the light, to the right, a little darker.
        let across = ((pos.x - rim.x) / half_width).clamp(-1.0, 1.0);
        Vertex {
            pos,
            uv: WHITE_UV,
            color: brighten(lit_front, (1.0 - 0.1 * down) * (1.0 - 0.05 * across)),
        }
    });
    painter.add(Shape::mesh(mesh));
    painter.add(edge);
    // Dished: its back slope in the cap's own shade, its front catching light.
    let lit_top = matte(top_colour, [0.0, 0.2, 1.0]);
    eye.fill(painter, &top, |[_, py, _]| {
        brighten(lit_top, 0.9).lerp_to_gamma(lit_top, (py - back) / (front - back))
    });
    painter.add(Shape::closed_line(
        eye.polygon(&top),
        Stroke::new(1.0, brighten(front_colour, 0.85)),
    ));
}

/// The convex hull of `points` on screen.
fn hull(mut points: Vec<Pos2>) -> Vec<Pos2> {
    points.sort_by(|a, b| a.x.total_cmp(&b.x).then(a.y.total_cmp(&b.y)));
    points.dedup();
    if points.len() < 3 {
        return points;
    }
    let turns_left =
        |o: Pos2, a: Pos2, b: Pos2| (a - o).x * (b - o).y - (a - o).y * (b - o).x > 0.0;
    let chain = |points: &mut dyn Iterator<Item = Pos2>| {
        let mut half: Vec<Pos2> = Vec::new();
        for p in points {
            while half.len() >= 2 && !turns_left(half[half.len() - 2], half[half.len() - 1], p) {
                half.pop();
            }
            half.push(p);
        }
        half.pop();
        half
    };
    let mut outline = chain(&mut points.iter().copied());
    outline.extend(chain(&mut points.iter().rev().copied()));
    outline
}

/// `main` printed on a key top at `centre`, `shifted` above it; `size` of a
/// letter's. Laid flat on the top, it foreshortens with it.
fn legend(painter: &Painter, eye: &Eye, centre: [f32; 3], main: &str, shifted: &str, size: f32) {
    if main.is_empty() {
        return;
    }
    let [x, y, z] = centre;
    let on_top = |across: f32, down: f32| [x + across, y + down, z];
    if main == BACKSPACE {
        let (half, head) = (0.15, 0.06);
        let stroke = Stroke::new(0.03 * FLAT_TEXT, LEGEND);
        let at = |u: f32, v: f32| pos2(u * FLAT_TEXT, v * FLAT_TEXT);
        let shapes = vec![
            Shape::line_segment([at(-half, 0.0), at(half, 0.0)], stroke),
            Shape::line(
                vec![
                    at(head - half, -head),
                    at(-half, 0.0),
                    at(head - half, head),
                ],
                stroke,
            ),
        ];
        let mesh = warp(painter, shapes, |p| {
            eye.at(on_top(p.x / FLAT_TEXT, p.y / FLAT_TEXT))
        });
        painter.add(Shape::mesh(mesh));
    } else if shifted.is_empty() {
        paint_flat_text(
            painter,
            eye,
            main,
            0.24 * size,
            LEGEND,
            Align2::CENTER_CENTER,
            on_top,
        );
    } else {
        let size = 0.16 * size;
        paint_flat_text(
            painter,
            eye,
            shifted,
            size,
            LEGEND,
            Align2::CENTER_BOTTOM,
            on_top,
        );
        paint_flat_text(painter, eye, main, size, LEGEND, Align2::CENTER_TOP, on_top);
    }
}

/// `text`, `size` inches tall, laid flat on a surface and seen in
/// perspective like print on it: `place` maps inches across and down from
/// the text's `anchor` to the machine.
fn paint_flat_text(
    painter: &Painter,
    eye: &Eye,
    text: &str,
    size: f32,
    colour: Color32,
    anchor: Align2,
    place: impl Fn(f32, f32) -> [f32; 3],
) {
    let galley = painter.layout_no_wrap(
        text.to_owned(),
        FontId::proportional(size * FLAT_TEXT),
        colour,
    );
    let at = anchor.anchor_size(Pos2::ZERO, galley.size()).min;
    let shapes = vec![Shape::galley(at, galley, colour)];
    let mesh = warp(painter, shapes, |p| {
        eye.at(place(p.x / FLAT_TEXT, p.y / FLAT_TEXT))
    });
    painter.add(Shape::mesh(mesh));
}

/// A point on the front panel's slope.
fn on_panel(x: f32, y: f32) -> [f32; 3] {
    let (y0, z0) = COVER_FRONT;
    let (y1, z1) = PANEL_BOTTOM;
    [x, y, z0 + (y - y0) / (y1 - y0) * (z1 - z0)]
}

/// The panel's controls, left to right.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Control {
    Spacing,
    Zoom,
    Correct,
    Goal,
    Save,
}

impl Control {
    pub const ALL: [Self; 5] = [
        Self::Spacing,
        Self::Zoom,
        Self::Correct,
        Self::Goal,
        Self::Save,
    ];

    /// Across the panel, in inches from its centre: where the maker's
    /// badge would be on the left, where its emblem would be on the right.
    fn x(self) -> f32 {
        match self {
            Self::Spacing => -5.6,
            Self::Zoom => -4.1,
            Self::Correct => -2.35,
            Self::Goal => 1.2,
            Self::Save => 4.5,
        }
    }

    /// Room for the name and reading beside it, in inches.
    fn label_width(self) -> f32 {
        match self {
            Self::Spacing => 0.5,
            Self::Zoom => 0.8,
            Self::Correct => 1.2,
            Self::Goal => 2.2,
            Self::Save => 0.7,
        }
    }

    /// Where its name and reading start: past the knob, or the button and
    /// its lamp.
    fn label_x(self) -> f32 {
        let past = if self == Self::Save { 0.3 } else { 0.0 };
        self.x() + LABEL_GAP + past
    }

    fn name(self) -> &'static str {
        match self {
            Self::Spacing => "SPACING",
            Self::Zoom => "ZOOM",
            Self::Correct => "CORRECT",
            Self::Goal => "GOAL",
            Self::Save => "SAVE",
        }
    }
}

/// What the panel's controls show.
pub struct PanelState<'a> {
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

/// The front panel's controls for the typing line at `typing_y`.
pub struct Panel {
    eye: Eye,
}

impl Panel {
    pub fn new(view: Rect, metrics: &Metrics, typing_y: f32) -> Self {
        Self {
            eye: Eye::new(view, metrics, typing_y),
        }
    }

    /// Where `control` takes a click: the knob, its marks and its labels.
    pub fn rect(&self, control: Control) -> Rect {
        let (left, reach) = (
            control.x() - KNOB_RADIUS * INDEX_MARKS.1,
            KNOB_RADIUS * INDEX_MARKS.1,
        );
        let right = control.label_x() + control.label_width();
        let [left, right] = [left, right].map(|x| on_panel(x, CONTROLS_Y));
        let corners = [
            panel_offset(left, reach, 0.0),
            panel_offset(right, reach, 0.0),
            panel_offset(right, reach, 180.0),
            panel_offset(left, reach, 180.0),
        ];
        Rect::from_points(&self.eye.polygon(&corners))
    }

    /// Draws each control, `hovered` ringed.
    pub fn paint(&self, painter: &Painter, state: &PanelState, hovered: Option<Control>) {
        for control in Control::ALL {
            let lit = hovered == Some(control);
            let (turn, marks, reading, colour) = match control {
                Control::Spacing => {
                    let notch = match state.spacing {
                        LineSpacing::Single => 0,
                        LineSpacing::OneAndHalf => 1,
                        LineSpacing::Double => 2,
                    };
                    let marks = notches(3, 100.0);
                    let reading = ["1", "1½", "2"][notch].to_owned();
                    (marks[notch], marks, reading, READING)
                }
                Control::Zoom => {
                    let share = (f32::from(state.zoom_percent) - 50.0) / 150.0;
                    let turn = -135.0 + 270.0 * share.clamp(0.0, 1.0);
                    let reading = format!("{} %", state.zoom_percent);
                    (turn, notches(4, 270.0), reading, READING)
                }
                Control::Correct => {
                    let mut ways = vec![EraseMode::Paper, EraseMode::Eraser, EraseMode::Fluid];
                    if state.delete_in_cycle || state.erase == EraseMode::Delete {
                        ways.push(EraseMode::Delete);
                    }
                    let marks = notches(ways.len(), 40.0 * (ways.len() - 1) as f32);
                    let way = ways.iter().position(|&w| w == state.erase).unwrap_or(0);
                    let reading = ruler::correction_method(state.erase, state.slip_in).to_owned();
                    (marks[way], marks, reading, READING)
                }
                Control::Goal => {
                    // Off first, then the plate's cycle.
                    let positions = state.goals.len() + 1;
                    let span = (30.0 * (positions - 1) as f32).min(270.0);
                    let marks = notches(positions, span);
                    let at = state
                        .goal
                        .and_then(|goal| state.goals.iter().position(|&g| g == goal))
                        .map_or(0, |i| i + 1);
                    let reached = state.progress.is_some_and(|p| p.reached);
                    let colour = if reached { ruler::SAVED } else { READING };
                    (
                        marks[at],
                        marks,
                        ruler::goal_reading(state.progress),
                        colour,
                    )
                }
                Control::Save => {
                    let (reading, lamp) = ruler::autosave_state(state.keeping);
                    self.paint_button(painter, control, lit, lamp);
                    self.paint_labels(painter, control, reading, READING);
                    continue;
                }
            };
            self.paint_knob(painter, control, lit, turn, &marks);
            self.paint_labels(painter, control, &reading, colour);
        }
    }

    /// A ribbed knob standing off the panel, its pointer at `turn` degrees
    /// clockwise from the top, index marks engraved round it at `marks`.
    fn paint_knob(&self, painter: &Painter, control: Control, lit: bool, turn: f32, marks: &[f32]) {
        let eye = &self.eye;
        let centre = on_panel(control.x(), CONTROLS_Y);
        for &mark in marks {
            let [inner, outer] =
                [INDEX_MARKS.0, INDEX_MARKS.1].map(|r| panel_offset(centre, KNOB_RADIUS * r, mark));
            eye.line(painter, &[inner, outer], 0.025, ENGRAVED);
        }
        let top = self.paint_cylinder(painter, centre, KNOB_HEIGHT, lit, [KNOB_TOP, KNOB_SIDE]);
        let [from, to] = [0.25, 0.85].map(|r| panel_offset(top, KNOB_RADIUS * r, turn));
        eye.line(painter, &[from, to], 0.05, READING);
    }

    /// The save button, flush, and its lamp beside it: lit in `lamp`'s
    /// colour, dark if `None`.
    fn paint_button(&self, painter: &Painter, control: Control, lit: bool, lamp: Option<Color32>) {
        let eye = &self.eye;
        let centre = on_panel(control.x(), CONTROLS_Y);
        self.paint_cylinder(
            painter,
            centre,
            BUTTON_HEIGHT,
            lit,
            [SHIFT_CAP, SHIFT_FRONT],
        );
        let bulb = on_panel(control.x() + 0.42, CONTROLS_Y);
        let rim = circle(bulb, LAMP_RADIUS * 1.4);
        eye.fill(painter, &rim, |_| CHROME);
        let glass = circle(bulb, LAMP_RADIUS);
        eye.fill(painter, &glass, |_| lamp.unwrap_or(LAMP_OFF));
        let glint = circle(
            panel_offset(bulb, LAMP_RADIUS * 0.4, -40.0),
            LAMP_RADIUS * 0.3,
        );
        eye.fill(painter, &glint, |_| Color32::from_white_alpha(120));
    }

    /// A short cylinder on the panel at `base`, `height` tall: its shadow,
    /// ribbed side and top. Returns its top's centre.
    fn paint_cylinder(
        &self,
        painter: &Painter,
        base: [f32; 3],
        height: f32,
        lit: bool,
        [top_colour, side_colour]: [Color32; 2],
    ) -> [f32; 3] {
        let eye = &self.eye;
        let normal = panel_normal();
        let top = [0, 1, 2].map(|k| base[k] + normal[k] * height);
        let shadow = circle(
            panel_offset(base, 0.06, KNOB_SHADOW_TURN),
            KNOB_RADIUS * 1.1,
        );
        eye.fill(painter, &shadow, |_| Color32::from_black_alpha(60));
        // The side: the base's near half, the top's far half.
        let steps = 24u8;
        let at = |centre: [f32; 3], i: u8| {
            let turn = 90.0 + 360.0 * f32::from(i) / f32::from(steps);
            panel_offset(centre, KNOB_RADIUS, turn)
        };
        let half = steps / 2;
        let side: Vec<[f32; 3]> = (0..=half)
            .map(|i| at(base, i))
            .chain((half..=steps).map(|i| at(top, i)))
            .collect();
        eye.fill(painter, &side, |_| side_colour);
        for i in (1..half).step_by(2) {
            eye.line(painter, &[at(base, i), at(top, i)], 0.02, KNOB_RIB);
        }
        let face = circle(top, KNOB_RADIUS);
        eye.fill(painter, &face, |_| top_colour);
        let rim = if lit { HIGHLIGHT } else { side_colour };
        painter.add(Shape::closed_line(
            eye.polygon(&face),
            Stroke::new(if lit { 2.0 } else { 1.0 }, rim),
        ));
        top
    }

    /// `control`'s engraved name beside it, and its reading under the name.
    fn paint_labels(&self, painter: &Painter, control: Control, reading: &str, colour: Color32) {
        let eye = &self.eye;
        let (name_y, reading_y) = LABEL_LINES;
        // On the panel's slope, like the knobs.
        let on = |line: f32| {
            let start = on_panel(control.label_x(), CONTROLS_Y + line);
            move |across: f32, down: f32| {
                add(
                    add(start, [across, 0.0, 0.0]),
                    panel_offset([0.0; 3], down, 180.0),
                )
            }
        };
        let left = Align2::LEFT_CENTER;
        paint_flat_text(
            painter,
            eye,
            control.name(),
            0.11,
            ENGRAVED,
            left,
            on(name_y),
        );
        paint_flat_text(painter, eye, reading, 0.14, colour, left, on(reading_y));
    }
}

/// `count` index marks spread over `span` degrees, centred on the top.
fn notches(count: usize, span: f32) -> Vec<f32> {
    if count < 2 {
        return vec![0.0];
    }
    (0..count)
        .map(|i| -span / 2.0 + span * i as f32 / (count - 1) as f32)
        .collect()
}

/// Up the panel's slope and away from it, unit length.
fn panel_normal() -> [f32; 3] {
    let (y0, z0) = COVER_FRONT;
    let (y1, z1) = PANEL_BOTTOM;
    let (dy, dz) = (y1 - y0, z1 - z0);
    let length = (dy * dy + dz * dz).sqrt();
    [0.0, -dz / length, dy / length]
}

/// On the panel's plane from `centre`, `radius` inches toward `turn` degrees
/// clockwise from the top, the top being up the slope.
fn panel_offset(centre: [f32; 3], radius: f32, turn: f32) -> [f32; 3] {
    let (y0, z0) = COVER_FRONT;
    let (y1, z1) = PANEL_BOTTOM;
    let (dy, dz) = (y1 - y0, z1 - z0);
    let length = (dy * dy + dz * dz).sqrt();
    // Down the slope, toward the writer.
    let down = [0.0, dy / length, dz / length];
    let (sin, cos) = turn.to_radians().sin_cos();
    [0, 1, 2].map(|k| {
        let right = if k == 0 { 1.0 } else { 0.0 };
        centre[k] + radius * (sin * right - cos * down[k])
    })
}

/// A circle on the panel's plane.
fn circle(centre: [f32; 3], radius: f32) -> Vec<[f32; 3]> {
    (0..32u8)
        .map(|i| panel_offset(centre, radius, 360.0 * f32::from(i) / 32.0))
        .collect()
}

/// The top of the bail's scale for the typing line at `typing_y`.
pub fn bail_scale_top(metrics: &Metrics, typing_y: f32) -> f32 {
    typing_y - BAIL_ABOVE_INCHES * metrics.points_per_inch - ruler::HEIGHT / 2.0
}

/// The paper bail across the carriage for the sheet centred at
/// `carriage_x`, its scale printed on it later: the arms holding it to the
/// side plates, the bar, and the rubber rollers pressing the paper.
pub fn paint_bail(painter: &Painter, metrics: &Metrics, carriage_x: f32, typing_y: f32) {
    let ppi = metrics.points_per_inch;
    let [left, right] = platen_ends(carriage_x, metrics);
    let (left, right) = (
        left + SIDE_PLATE_INCHES * ppi,
        right - SIDE_PLATE_INCHES * ppi,
    );
    let y = bail_scale_top(metrics, typing_y) + ruler::HEIGHT / 2.0;
    let half_height = ruler::HEIGHT / 2.0 + BAIL_EXTRA_INCHES * ppi;
    let pivot_y = typing_y - BAIL_PIVOT_ABOVE * ppi;
    let arm = BAIL_ARM_INCHES * ppi;
    for x in [left..=left + arm, right - arm..=right] {
        let arm_rect = Rect::from_x_y_ranges(x, y - half_height..=pivot_y + arm / 2.0);
        painter.rect(
            arm_rect,
            CornerRadius::same(3),
            CHROME,
            Stroke::new(1.0, EDGE),
            StrokeKind::Inside,
        );
        let rivet = pos2(arm_rect.center().x, pivot_y);
        painter.circle(rivet, arm * 0.28, METAL, Stroke::new(1.0, EDGE));
    }
    let bar = Rect::from_x_y_ranges(left + arm..=right - arm, y - half_height..=y + half_height);
    // Brushed metal, brightest just above the middle.
    let stops = [
        (bar.top(), BAIL_TOP),
        (bar.top() + 0.35 * bar.height(), BAIL_MID),
        (bar.bottom(), BAIL_LOW),
    ];
    let mut mesh = Mesh::default();
    for pair in stops.windows(2) {
        let [(upper, upper_colour), (lower, lower_colour)] = [pair[0], pair[1]];
        add_quad(
            &mut mesh,
            [
                (pos2(bar.left(), upper), upper_colour),
                (pos2(bar.right(), upper), upper_colour),
                (pos2(bar.right(), lower), lower_colour),
                (pos2(bar.left(), lower), lower_colour),
            ],
        );
    }
    painter.add(Shape::mesh(mesh));
    painter.rect_stroke(
        bar,
        CornerRadius::same(2),
        Stroke::new(1.0, EDGE),
        StrokeKind::Inside,
    );
    let mut mesh = Mesh::default();
    for side in [-1.0, 1.0] {
        let x = carriage_x + side * BAIL_ROLLER_INCHES * ppi;
        let radius = ruler::HEIGHT / 2.0 + 0.07 * ppi;
        let width = 0.25 * ppi;
        roller(
            &mut mesh,
            x - width..=x + width,
            y,
            radius,
            [RUBBER, RUBBER_SHINE],
        );
    }
    painter.add(Shape::mesh(mesh));
}

/// Corners `(point, rounding)` of a flat convex polygon, each rounded by a
/// curve tangent to both sides.
fn rounded(corners: &[([f32; 3], f32)]) -> Vec<[f32; 3]> {
    let n = corners.len();
    let mut outline = Vec::new();
    for (i, &(corner, rounding)) in corners.iter().enumerate() {
        let (before, after) = (corners[(i + n - 1) % n].0, corners[(i + 1) % n].0);
        outline.extend(fillet(before, corner, after, rounding));
    }
    outline
}

/// `corner` rounded between the sides to `before` and `after`: a curve
/// tangent to both, from the side toward `before`. The corner alone if
/// `rounding` is zero.
fn fillet(before: [f32; 3], corner: [f32; 3], after: [f32; 3], rounding: f32) -> Vec<[f32; 3]> {
    const STEPS: u16 = 6;
    if rounding <= 0.0 {
        return vec![corner];
    }
    let toward = |other: [f32; 3]| {
        let d = sub(other, corner);
        // At most halfway: neighbouring roundings never cross.
        let t = (rounding / dot(d, d).sqrt().max(1e-6)).min(0.5);
        add(corner, [d[0] * t, d[1] * t, d[2] * t])
    };
    let (from, to) = (toward(before), toward(after));
    (0..=STEPS)
        .map(|step| {
            let t = f32::from(step) / f32::from(STEPS);
            lerp3(lerp3(from, corner, t), lerp3(corner, to, t), t)
        })
        .collect()
}

fn lerp3(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
    [0, 1, 2].map(|k| a[k] + (b[k] - a[k]) * t)
}

fn add_quad(mesh: &mut Mesh, corners: [(Pos2, Color32); 4]) {
    let first = mesh.vertices.len() as u32;
    for (pos, colour) in corners {
        mesh.colored_vertex(pos, colour);
    }
    mesh.add_triangle(first, first + 1, first + 2);
    mesh.add_triangle(first, first + 2, first + 3);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn eye(typing_y: f32, ppi: f32) -> Eye {
        Eye {
            origin: pos2(800.0, typing_y),
            ppi,
        }
    }

    #[test]
    fn the_printing_point_is_at_the_sheets_scale() {
        let eye = eye(500.0, 96.0);
        assert_eq!(eye.at([0.0, 0.0, 0.0]), eye.origin);
        assert!((eye.scale([0.0, 0.0, 0.0]) - 96.0).abs() < 1e-3);
        // Nearer the writer: lower on screen, and larger.
        let key = [0.0, KEY_ROW.0, KEY_ROW.1];
        assert!(eye.at(key).y > eye.at(on_cover(0.0, COVER_FRONT.0)).y);
        assert!(eye.scale(key) > 96.0);
    }

    fn metrics(zoom_percent: u16) -> Metrics {
        let sm9 = include_str!("../../../../profiles/olympia-sm9.toml");
        let profile = typewriter_core::Profile::from_toml_str(sm9).unwrap();
        Metrics::new(&profile, super::super::points_per_inch(zoom_percent))
    }

    #[test]
    fn sitting_back_raises_the_typing_line() {
        // So tall the controls are always in view.
        let tall = Rect::from_min_size(Pos2::ZERO, vec2(1600.0, 4000.0));
        let at = |zoom| typing_line_height(tall, &metrics(zoom), zoom);
        assert_eq!(at(100), TYPING_LINE_LEANING);
        assert_eq!(at(200), TYPING_LINE_LEANING);
        assert_eq!(at(50), TYPING_LINE_SITTING);
        assert!(at(75) < TYPING_LINE_LEANING);
    }

    #[test]
    fn the_controls_stay_in_view_up_to_life_size() {
        let view = Rect::from_min_size(Pos2::ZERO, vec2(1536.0, 960.0));
        for zoom in [50, 80, 100] {
            let metrics = metrics(zoom);
            let typing_y = view.height() * typing_line_height(view, &metrics, zoom);
            let panel = Panel::new(view, &metrics, typing_y);
            for control in Control::ALL {
                let rect = panel.rect(control);
                assert!(
                    view.contains_rect(rect),
                    "{zoom} %: {control:?} at {rect:?}"
                );
            }
        }
    }

    #[test]
    fn the_keys_sit_between_the_pillars() {
        for key in KEYS.iter().flat_map(|row| row.iter()) {
            let half = KEY_CAP.0 * key.width / 2.0;
            assert!(key.x().abs() + half < OPENING_HALF, "{}", key.legend);
        }
    }

    #[test]
    fn the_whole_machine_fits_below_the_typing_line_sitting_back() {
        // 50 %: 48 points an inch, in a view 1000 points high.
        let eye = eye(1000.0 * TYPING_LINE_SITTING, 48.0);
        let front = eye.at([0.0, BASE_FRONT, DESK_Z]);
        assert!(front.y < 1000.0, "{front:?}");
    }

    #[test]
    fn the_walls_stand_above_the_keys_and_the_shelf_holds_the_bar() {
        let eye = eye(300.0, 48.0);
        for row in 0..4 {
            let (y, z) = key_row(row as f32);
            assert!(wall_top(y) > z + 0.2, "row {row}");
        }
        // The walls meet the shelf at the case's front corners, above it before.
        assert!((wall_top(SHELF.1) - SHELF.2).abs() < 1e-4);
        assert!(wall_top(SPACE_ROW.0) > SHELF.2);
        let (space_y, space_z) = SPACE_ROW;
        assert!(SHELF.2 <= space_z);
        assert!(SHELF.0 < space_y - 0.2 && space_y + 0.2 < NOTCH.1);
        assert!(
            SPACE_BAR
                .iter()
                .all(|&(left, right, _)| -NOTCH.0 < left && right < NOTCH.0)
        );
        let bar = eye.at([0.0, space_y + 0.2, space_z]).y;
        let shelf = eye.at([0.0, SHELF.1, SHELF.2]).y;
        assert!(bar < shelf, "{bar} {shelf}");
        assert!(key_row(3.0).0 + KEY_CAP.1 / 2.0 < SHELF.0);
    }

    #[test]
    fn the_case_corner_is_sampled_finely_to_its_end() {
        let corner: Vec<[f32; 3]> = case_side()
            .into_iter()
            .filter(|p| p[1] >= SHELF.1 - FRAME_ROUNDING - 1e-4)
            .collect();
        // No step cuts across the curve: every one short, the last too.
        for pair in corner.windows(2) {
            let step = sub(pair[1], pair[0]);
            assert!(step[0].hypot(step[1]) < FRAME_ROUNDING / 10.0, "{pair:?}");
        }
        let last = corner[corner.len() - 1];
        assert!((last[1] - SHELF.1).abs() < 1e-4);
        assert!((last[0] - (PANEL_HALF_BOTTOM - FRAME_ROUNDING)).abs() < 1e-4);
    }

    #[test]
    fn the_case_front_faces_the_writer_or_the_sides() {
        for pair in case_front_edge().windows(2) {
            let along = sub(pair[1], pair[0]);
            // Outward, right to left: never back into the well.
            assert!(-along[0] >= -1e-4, "{pair:?}");
        }
    }

    #[test]
    fn the_eye_sees_faces_turned_toward_it_only() {
        let front = [0.0, SHELF.1, SHELF.2];
        assert!(Eye::sees(front, [0.0, 1.0, 0.0]));
        assert!(!Eye::sees(front, [0.0, -1.0, 0.0]));
        // A left-hand side face, turned away from the middle.
        let side = [-PANEL_HALF_BOTTOM, SHELF.1 - 0.1, SHELF.2];
        assert!(!Eye::sees(side, [-1.0, 0.0, 0.0]));
        assert!(Eye::sees(side, [1.0, 0.0, 0.0]));
    }

    #[test]
    fn the_rod_lies_under_every_lever_and_below_the_far_caps() {
        let levers = key_levers();
        let y = key_row(0.0).0 - ROD_BEHIND;
        let axis = rod_axis(y, &levers).unwrap();
        // Every lever runs back over it.
        for lever in &levers {
            let over = lever_at(*lever, y).unwrap();
            assert!(over[2] > axis + ROD_RADIUS, "{lever:?}");
        }
        assert!(axis + ROD_RADIUS < key_row(0.0).1 - KEY_FRONT);
        assert!(axis - ROD_RADIUS > KEY_BED_Z);
    }

    #[test]
    fn the_side_controls_fit_between_the_keys_and_the_walls() {
        let edge = |key: &Key| {
            let half = KEY_CAP.0 * key.width / 2.0;
            (key.x() - half, key.x() + half)
        };
        // The lock beside the far row's first key; the selectors beside the
        // third row's ends; all inside the walls.
        assert!(-LOCK_X + LOCK_CAP.0 < edge(&KEYS[0][0]).0);
        assert!(-SELECTOR_X + PADDLE.0 < edge(&KEYS[2][0]).0);
        assert!(SELECTOR_X - PADDLE.0 > edge(&KEYS[2][KEYS[2].len() - 1]).1);
        assert!(LOCK_X + LOCK_CAP.0 < OPENING_HALF);
        assert!(SELECTOR_X + PADDLE.0 < OPENING_HALF);
    }

    #[test]
    fn the_lock_stands_above_the_keys_and_the_selectors_by_their_marks() {
        // At least half the lock's cap above the far row's tops.
        let (_, row_top) = key_row(0.0);
        let cap_top = row_top + LOCK_RISE;
        assert!(cap_top - row_top >= row_top - (cap_top - KEY_FRONT));
        // The ribbon selector beside the blue mark, level with it.
        let eye = eye(300.0, 48.0);
        let (y, _) = key_row(2.0);
        let mark_y = y + RIBBON_ALONG[RIBBON_SET];
        let mark = eye.at([SELECTOR_X, mark_y, wall_top(mark_y)]).y;
        let paddle = eye
            .at([SELECTOR_X, mark_y, wall_top(mark_y) - PADDLE_BELOW_WALL])
            .y;
        let next = y + RIBBON_ALONG[RIBBON_SET + 1];
        let next_mark = eye.at([SELECTOR_X, next, wall_top(next)]).y;
        assert!((paddle - mark).abs() < (paddle - next_mark).abs());
        assert_eq!(RIBBON_SET, 0, "blue, for black");
    }

    #[test]
    fn the_selectors_rods_run_well_under_the_locks() {
        let (_, row_top) = key_row(0.0);
        let lock_rod = row_top + LOCK_RISE - KEY_FRONT - LOCK_DROP;
        let (y, _) = key_row(2.0);
        let touch_rod = wall_top(y) - PADDLE_BELOW_WALL - PADDLE.1 - SELECTOR_DROP;
        assert!(touch_rod < lock_rod - 0.3, "{touch_rod} {lock_rod}");
        // Both above the key bed.
        assert!(touch_rod > KEY_BED_Z);
    }

    #[test]
    fn a_hull_keeps_only_the_outline() {
        let square = [
            (0.0, 0.0),
            (2.0, 0.0),
            (2.0, 2.0),
            (0.0, 2.0),
            (1.0, 1.0),
            (1.0, 0.0),
        ];
        let outline = hull(square.iter().map(|&(x, y)| pos2(x, y)).collect());
        assert_eq!(outline.len(), 4);
        assert!(!outline.contains(&pos2(1.0, 1.0)));
    }

    #[test]
    fn the_layout_starts_with_plus_equals_and_has_no_one() {
        let first = &KEYS[0][0];
        assert_eq!((first.legend, first.shifted), ("=", "+"));
        let keys = KEYS.iter().flat_map(|row| row.iter());
        assert!(keys.clone().all(|key| key.legend != "1"));
        assert!(SPACE_BAR.iter().any(|&(_, _, name)| name == "clear"));
    }

    #[test]
    fn one_light_from_the_front_left() {
        let (left, right) = (
            matte(IVORY, [-1.0, 0.0, 0.0]),
            matte(IVORY, [1.0, 0.0, 0.0]),
        );
        assert!(left.r() > right.r());
        let up = matte(IVORY, [0.0, 0.0, 1.0]);
        assert!(up.r() > matte(IVORY, [0.0, 0.0, -1.0]).r());
        for angle in 0..36 {
            let turn = (10.0 * angle as f32).to_radians();
            let shine = streak([turn.cos(), turn.sin(), 0.0], 6);
            assert!((0.0..=1.0).contains(&shine));
        }
    }

    #[test]
    fn the_type_bars_reach_past_the_opening() {
        let back = OPENING_BACK.0 - 0.15;
        assert!(back + TYPE_BAR_REACH > OPENING_FRONT.0);
        assert!(TYPE_BAR_REACH * 84.0_f32.to_radians().sin() > OPENING_FRONT.1);
    }

    #[test]
    fn the_controls_sit_apart_on_the_panel() {
        let reach = KNOB_RADIUS * INDEX_MARKS.1;
        let spans: Vec<(f32, f32)> = Control::ALL
            .iter()
            .map(|c| (c.x() - reach, c.label_x() + c.label_width()))
            .collect();
        for pair in spans.windows(2) {
            assert!(pair[0].1 <= pair[1].0 + 1e-3, "{pair:?}");
        }
        let half = PANEL_HALF_BOTTOM - 0.3;
        assert!(
            spans
                .iter()
                .all(|&(left, right)| left > -half && right < half)
        );
    }

    #[test]
    fn the_machine_casts_its_shadow_one_way() {
        let eye = eye(300.0, 48.0);
        let (body, cast) = (footprint(&eye, (0.0, 0.0)), footprint(&eye, CAST_SHADOW));
        // Right and left ends of the front: hidden under the body on the
        // left, past it on the right.
        let right = |p: &[Pos2]| p.iter().map(|q| q.x).fold(f32::MIN, f32::max);
        let left = |p: &[Pos2]| p.iter().map(|q| q.x).fold(f32::MAX, f32::min);
        assert!(left(&cast) > left(&body));
        assert!(right(&cast) > right(&body));
    }

    #[test]
    fn the_table_clears_the_typing_line() {
        // A line is a sixth of an inch: its descenders must show.
        let eye = eye(0.0, 96.0);
        let table = eye.at([0.0, TABLE_BACK.0, TABLE_BACK.1]).y;
        let throat = eye.at([0.0, 0.05, -0.3]).y;
        assert!(table.min(throat) > 96.0 / 6.0 + 4.0, "{table} {throat}");
    }
}
