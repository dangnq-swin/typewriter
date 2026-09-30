//! The desk edition's typewriter, an Olympia SM9 seen from the chair.
//!
//! The body is a model in inches, projected from a seated eye: `x` right of
//! the machine's centre, `y` toward the writer, `z` up, the origin at the
//! printing point. There the projection's scale is the sheet's, so the flat
//! sheet and the carriage (platen, knobs, bail), which travel with it, line
//! up with the still body around them.

use std::f32::consts::PI;
use std::ops::RangeInclusive;

use eframe::egui::epaint::{Shadow, Vertex, WHITE_UV};
use eframe::egui::{
    Align2, Color32, CornerRadius, FontId, Mesh, Painter, Pos2, Rect, Shape, Stroke, StrokeKind,
    pos2, vec2,
};
use typewriter_core::session::Progress;
use typewriter_core::{EraseMode, Goal, LineSpacing};

use super::feed::convex_mesh;
use super::{HIGHLIGHT, Metrics, ruler};
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
const COVER_BACK: (f32, f32) = (0.95, -0.5);
const COVER_FRONT: (f32, f32) = (4.0, -1.25);
const COVER_HALF: (f32, f32) = (5.95, 6.35);
const OPENING_BACK: (f32, f32) = (1.05, 1.6);
const OPENING_FRONT: (f32, f32) = (3.55, 3.7);
/// The dark throat between platen and cover, behind the card holder.
const THROAT_TOP: (f32, f32) = (0.05, -0.3);
/// The card holder's translucent plates under the platen, either side of
/// the type guide: `(y, z)` back and front, and their inner and outer `x`.
/// The sheet shows through them, and goes out of sight below them.
const TABLE_BACK: (f32, f32) = (0.18, -0.25);
const TABLE_FRONT: (f32, f32) = (0.85, -0.45);
const TABLE_X: (f32, f32) = (0.55, 4.35);
/// The front panel, down to the keyboard's opening.
const PANEL_BOTTOM: (f32, f32) = (6.2, -2.15);
const PANEL_HALF_BOTTOM: f32 = 6.45;
/// The controls where the maker's badge would be: across the panel at `y`,
/// their names and readings below.
const CONTROLS_Y: f32 = 4.75;
const CONTROL_NAME_Y: f32 = 5.4;
const CONTROL_READING_Y: f32 = 5.65;
const KNOB_RADIUS: f32 = 0.3;
const KNOB_HEIGHT: f32 = 0.22;
const BUTTON_HEIGHT: f32 = 0.1;
/// Past the knob, where its index marks are engraved: inner and outer.
const INDEX_MARKS: (f32, f32) = (1.25, 1.5);
const LAMP_RADIUS: f32 = 0.08;
/// The keyboard's opening between the side pillars, the panel's thickness
/// over it, and the front lip.
const OPENING_HALF: f32 = 5.7;
const PANEL_EDGE_INCHES: f32 = 0.45;
const LIP_FRONT: (f32, f32) = (10.85, -3.95);
const BASE_FRONT: f32 = 11.2;
const DESK_Z: f32 = -4.35;

/// Keys: the far row's centre `(y, z)` of its tops, one row to the next.
const KEY_ROW: (f32, f32) = (6.95, -2.5);
const KEY_ROW_STEP: (f32, f32) = (0.8, -0.36);
const KEY_PITCH: f32 = 0.75;
/// A key cap's top, and its front below it.
const KEY_CAP: (f32, f32) = (0.56, 0.5);
const KEY_FRONT: f32 = 0.28;
/// The keyboard's middle, in key pitches from the far row's first key.
const KEYBOARD_MIDDLE: f32 = 5.75;
/// Where a key's lever goes, from under its cap: back and down.
const LEVER: (f32, f32) = (0.7, 1.0);

/// As on a UK SM9, far row first.
const KEYS: [&[Key]; 4] = [
    &[
        Key::new("1", "*", 0.0),
        Key::new("2", "\"", 1.0),
        Key::new("3", "/", 2.0),
        Key::new("4", "@", 3.0),
        Key::new("5", "£", 4.0),
        Key::new("6", "_", 5.0),
        Key::new("7", "&", 6.0),
        Key::new("8", "'", 7.0),
        Key::new("9", "(", 8.0),
        Key::new("0", ")", 9.0),
        Key::new("¼", "¾", 10.0),
        Key::new("=", "+", 11.0),
        Key::new("TAB", "", 12.15),
    ],
    &[
        Key::new("::", "", -0.55),
        Key::new("Q", "", 0.45),
        Key::new("W", "", 1.45),
        Key::new("E", "", 2.45),
        Key::new("R", "", 3.45),
        Key::new("T", "", 4.45),
        Key::new("Y", "", 5.45),
        Key::new("U", "", 6.45),
        Key::new("I", "", 7.45),
        Key::new("O", "", 8.45),
        Key::new("P", "", 9.45),
        Key::new("-", "?", 10.45),
        Key::new("<", "", 11.55),
    ],
    &[
        Key::new("", "", -0.35),
        Key::new("A", "", 0.7),
        Key::new("S", "", 1.7),
        Key::new("D", "", 2.7),
        Key::new("F", "", 3.7),
        Key::new("G", "", 4.7),
        Key::new("H", "", 5.7),
        Key::new("J", "", 6.7),
        Key::new("K", "", 7.7),
        Key::new("L", "", 8.7),
        Key::new(";", ":", 9.7),
        Key::new("1/3", "2/3", 10.7),
    ],
    &[
        Key::shift(-0.1),
        Key::new("Z", "", 1.1),
        Key::new("X", "", 2.1),
        Key::new("C", "", 3.1),
        Key::new("V", "", 4.1),
        Key::new("B", "", 5.1),
        Key::new("N", "", 6.1),
        Key::new("M", "", 7.1),
        Key::new(",", ";", 8.1),
        Key::new(".", ":", 9.1),
        Key::new("½", "%", 10.1),
        Key::shift(11.45),
    ],
];
/// The half-space keys either side of the space bar, and the bar: `x`.
const SPACE_BAR: [(f32, f32); 3] = [(-3.05, -2.45), (-2.35, 2.35), (2.45, 3.05)];

const RUBBER: Color32 = Color32::from_rgb(0x16, 0x15, 0x14);
const RUBBER_SHINE: Color32 = Color32::from_rgb(0x55, 0x53, 0x50);
const METAL: Color32 = Color32::from_rgb(0x8C, 0x8E, 0x8C);
const METAL_SHINE: Color32 = Color32::from_rgb(0xDC, 0xDE, 0xDA);
const CHROME: Color32 = Color32::from_rgb(0xC8, 0xCA, 0xC6);
const IVORY_LIT: Color32 = Color32::from_rgb(0xEE, 0xEC, 0xE2);
const IVORY: Color32 = Color32::from_rgb(0xE2, 0xDF, 0xD2);
const IVORY_SHADE: Color32 = Color32::from_rgb(0xC6, 0xC2, 0xB3);
const IVORY_DEEP: Color32 = Color32::from_rgb(0xA8, 0xA3, 0x94);
const GLASS: Color32 = Color32::from_rgba_unmultiplied_const(0xF8, 0xF8, 0xF4, 0x46);
const GLASS_EDGE: Color32 = Color32::from_rgba_unmultiplied_const(0x70, 0x70, 0x6A, 0xB0);
const BAIL_TOP: Color32 = Color32::from_rgb(0xD4, 0xD6, 0xD2);
const BAIL_MID: Color32 = Color32::from_rgb(0xEE, 0xEF, 0xEC);
const BAIL_LOW: Color32 = Color32::from_rgb(0xAE, 0xB0, 0xAC);
const EDGE: Color32 = Color32::from_rgb(0x9C, 0x97, 0x88);
const DARK: Color32 = Color32::from_rgb(0x1A, 0x1A, 0x19);
const INSIDE: Color32 = Color32::from_rgb(0x10, 0x10, 0x10);
const INSIDE_FRONT: Color32 = Color32::from_rgb(0x2A, 0x2A, 0x28);
const TYPE_BAR: Color32 = Color32::from_rgb(0x74, 0x74, 0x70);
const SEGMENT: Color32 = Color32::from_rgb(0x9A, 0x9A, 0x94);
const TEAL: Color32 = Color32::from_rgb(0x2F, 0x80, 0x78);
const PLINTH: Color32 = Color32::from_rgb(0x8C, 0x8A, 0x86);
const CAP: Color32 = Color32::from_rgb(0xF0, 0xED, 0xE0);
const CAP_FRONT: Color32 = Color32::from_rgb(0xD2, 0xCD, 0xBC);
const SHIFT_CAP: Color32 = Color32::from_rgb(0x5E, 0xB2, 0x9C);
const SHIFT_FRONT: Color32 = Color32::from_rgb(0x3F, 0x8C, 0x78);
const LEGEND: Color32 = Color32::from_rgb(0x1E, 0x1E, 0x1E);
const STEM: Color32 = Color32::from_rgb(0x8A, 0x8A, 0x86);
const SPRING: Color32 = Color32::from_rgb(0xB4, 0xB4, 0xAE);
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
            width: 1.4,
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
    let readings = eye.at(on_panel(0.0, CONTROL_READING_Y + 0.15)).y;
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

/// Its shadow on the desk, soft, beside and below the body.
fn paint_shadow(painter: &Painter, eye: &Eye) {
    let corners = [
        [-COVER_HALF.1, COVER_FRONT.0, COVER_FRONT.1],
        [COVER_HALF.1, COVER_FRONT.0, COVER_FRONT.1],
        [PANEL_HALF_BOTTOM, BASE_FRONT, DESK_Z],
        [-PANEL_HALF_BOTTOM, BASE_FRONT, DESK_Z],
    ];
    let rect = Rect::from_points(&eye.polygon(&corners));
    // Safe casts: clamped to their types.
    let shadow = Shadow {
        offset: [
            (0.1 * eye.ppi).clamp(0.0, 127.0) as i8,
            (0.15 * eye.ppi).clamp(0.0, 127.0) as i8,
        ],
        blur: (0.8 * eye.ppi).clamp(8.0, 255.0) as u8,
        spread: 0,
        color: Color32::from_black_alpha(120),
    };
    painter.add(shadow.as_shape(rect, CornerRadius::same(12)));
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

/// The cover round its opening, the type bars fanned in the segment below.
fn paint_ribbon_cover(painter: &Painter, eye: &Eye) {
    let (ob, of) = (OPENING_BACK, OPENING_FRONT);
    let opening = [
        on_cover(-ob.1, ob.0),
        on_cover(ob.1, ob.0),
        on_cover(of.1, of.0),
        on_cover(-of.1, of.0),
    ];
    eye.fill(painter, &opening, |_| INSIDE);
    // The segment's arc, its bars pointing to the printing point.
    let arc = |angle: f32| [2.3 * angle.sin(), 1.0 + 2.3 * angle.cos(), -1.7];
    let angles: Vec<f32> = (0..=34u8)
        .map(|i| (-58.0 + 116.0 * f32::from(i) / 34.0_f32).to_radians())
        .collect();
    for &angle in &angles {
        let [x, y, z] = arc(angle);
        let inward = [0.45 * x, 1.0 + 0.45 * (y - 1.0), z + 0.4];
        eye.line(painter, &[[x, y, z], inward], 0.05, TYPE_BAR);
    }
    let segment: Vec<[f32; 3]> = angles.iter().map(|&angle| arc(angle)).collect();
    eye.line(painter, &segment, 0.1, SEGMENT);
    let vibrator: Vec<[f32; 3]> = (0..=16u8)
        .map(|i| {
            let angle = (-90.0 + 180.0 * f32::from(i) / 16.0_f32).to_radians();
            [1.3 * angle.sin(), 3.35 - angle.cos(), -1.35]
        })
        .collect();
    eye.fill(painter, &vibrator, |_| TEAL);

    // The cover in four pieces round the opening: back, sides, front. They
    // hide whatever of the type bars lies under them.
    let (back, front) = COVER_HALF;
    let (bl, br) = (on_cover(-back, COVER_BACK.0), on_cover(back, COVER_BACK.0));
    let (fl, fr) = (
        on_cover(-front, COVER_FRONT.0),
        on_cover(front, COVER_FRONT.0),
    );
    let [obl, obr, ofr, ofl] = opening;
    let lit = |[_, y, _]: [f32; 3]| {
        let t = (y - COVER_BACK.0) / (COVER_FRONT.0 - COVER_BACK.0);
        IVORY_SHADE.lerp_to_gamma(IVORY_LIT, t)
    };
    // Square at the fold: the panel below meets it edge for edge.
    let pieces = [
        rounded(&[(bl, 0.0), (br, 0.0), (obr, 0.0), (obl, 0.0)]),
        rounded(&[(bl, 0.25), (obl, 0.0), (ofl, 0.0), (fl, 0.0)]),
        rounded(&[(obr, 0.0), (br, 0.25), (fr, 0.0), (ofr, 0.0)]),
        rounded(&[(ofl, 0.0), (ofr, 0.0), (fr, 0.0), (fl, 0.0)]),
    ];
    for piece in &pieces {
        eye.fill(painter, piece, lit);
    }
    // The opening's rim catches the light.
    eye.line(painter, &[obl, ofl, ofr, obr], 0.03, IVORY_LIT);
    eye.outline(
        painter,
        &rounded(&[(bl, 0.25), (br, 0.25), (fr, 0.0), (fl, 0.0)]),
    );
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
    // The fold from cover to panel.
    let fold = [[-top + 0.4, y0, z0], [top - 0.4, y0, z0]];
    eye.line(painter, &fold, 0.03, Color32::WHITE);
}

/// The open keyboard: dark insides, the pillars either side, the keys on
/// their levers and springs, the space bar, the front lip and the plinth.
fn paint_keyboard(painter: &Painter, eye: &Eye) {
    let (y0, z0) = PANEL_BOTTOM;
    let (y1, z1) = LIP_FRONT;
    let (half, outer) = (OPENING_HALF, PANEL_HALF_BOTTOM);
    let under = z0 - PANEL_EDGE_INCHES;
    let inside = [
        [-half, y0, under],
        [half, y0, under],
        [half, y1, DESK_Z],
        [-half, y1, DESK_Z],
    ];
    eye.fill(painter, &inside, |[_, y, _]| {
        INSIDE.lerp_to_gamma(INSIDE_FRONT, (y - y0) / (y1 - y0))
    });
    // The panel's edge over the opening, and the pillars' inner walls.
    let edge = [
        [-half, y0, z0],
        [half, y0, z0],
        [half, y0, under],
        [-half, y0, under],
    ];
    eye.fill(painter, &edge, |_| IVORY_SHADE);
    for side in [-1.0, 1.0] {
        let x = side * half;
        let wall = [[x, y0, z0], [x, y1, z1], [x, y1, DESK_Z], [x, y0, under]];
        eye.fill(painter, &wall, |_| IVORY_DEEP);
    }

    // Back row first: each row's levers, then its caps over them.
    for (row, keys) in KEYS.iter().enumerate() {
        let step = row as f32;
        let (y, z) = (
            KEY_ROW.0 + step * KEY_ROW_STEP.0,
            KEY_ROW.1 + step * KEY_ROW_STEP.1,
        );
        for key in keys.iter() {
            paint_lever(painter, eye, [key.x(), y, z - KEY_FRONT]);
        }
        for key in keys.iter() {
            paint_key(painter, eye, key, [key.x(), y, z]);
        }
    }
    let (y, z) = (
        KEY_ROW.0 + 4.0 * KEY_ROW_STEP.0 + 0.1,
        KEY_ROW.1 + 4.0 * KEY_ROW_STEP.1 + 0.1,
    );
    for x in [-2.75, -1.8, 1.8, 2.75] {
        paint_lever(painter, eye, [x, y, z - KEY_FRONT]);
    }
    for (left, right) in SPACE_BAR {
        paint_cap(
            painter,
            eye,
            [left, right],
            [y - 0.2, y + 0.2],
            z,
            [CAP, CAP_FRONT],
        );
    }
    for ((left, right), sign) in [(SPACE_BAR[0], "-"), (SPACE_BAR[2], "+")] {
        legend(painter, eye, [(left + right) / 2.0, y, z], sign, "", 0.8);
    }

    // The pillars beside the keys, the lip in front, the plinth under it.
    for side in [-1.0, 1.0] {
        let pillar = [
            [side * outer, y0, z0],
            [side * half, y0, z0],
            [side * half, y1, z1],
            [side * outer, y1, z1],
        ];
        eye.fill(painter, &pillar, |[_, y, _]| {
            IVORY.lerp_to_gamma(IVORY_SHADE, (y - y0) / (y1 - y0))
        });
    }
    let lip_z = z1 - 0.05;
    let band = |[top, bottom]: [[f32; 2]; 2]| {
        [
            [-outer, top[0], top[1]],
            [outer, top[0], top[1]],
            [outer, bottom[0], bottom[1]],
            [-outer, bottom[0], bottom[1]],
        ]
    };
    eye.fill(painter, &band([[y1, z1], [BASE_FRONT, lip_z]]), |_| {
        IVORY_LIT
    });
    let plinth_z = DESK_Z + 0.15;
    let lip_front = band([[BASE_FRONT, lip_z], [BASE_FRONT, plinth_z]]);
    eye.fill(painter, &lip_front, |_| IVORY_SHADE);
    let plinth = band([[BASE_FRONT, plinth_z], [BASE_FRONT, DESK_Z]]);
    eye.fill(painter, &plinth, |_| PLINTH);
    let silhouette = [
        [-outer, y0, z0],
        [-outer, BASE_FRONT, DESK_Z],
        [outer, BASE_FRONT, DESK_Z],
        [outer, y0, z0],
    ];
    painter.add(Shape::line(
        eye.polygon(&silhouette),
        Stroke::new(1.0, EDGE),
    ));
}

/// A key's lever down into the machine from under its cap, its spring
/// coiled round it.
fn paint_lever(painter: &Painter, eye: &Eye, under_cap: [f32; 3]) {
    let [x, y, z] = under_cap;
    let end = [x, y - LEVER.0, z - LEVER.1];
    eye.line(painter, &[under_cap, end], 0.035, STEM);
    let turns = 7u8;
    let spring: Vec<[f32; 3]> = (0..=2 * turns)
        .map(|i| {
            let t = 0.3 + 0.4 * f32::from(i) / f32::from(2 * turns);
            let out = if i % 2 == 0 { -0.06 } else { 0.06 };
            [x + out, y - LEVER.0 * t, z - LEVER.1 * t]
        })
        .collect();
    eye.line(painter, &spring, 0.02, SPRING);
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

/// A rounded cap over `x` and `y` ranges, its top at `z`: the front face
/// below, then the top.
fn paint_cap(
    painter: &Painter,
    eye: &Eye,
    [left, right]: [f32; 2],
    [back, front]: [f32; 2],
    z: f32,
    [top_colour, front_colour]: [Color32; 2],
) {
    let front_face = rounded(&[
        ([left, front, z], 0.0),
        ([right, front, z], 0.0),
        ([right, front, z - KEY_FRONT], 0.1),
        ([left, front, z - KEY_FRONT], 0.1),
    ]);
    eye.fill(painter, &front_face, |_| front_colour);
    let top = rounded(&[
        ([left, back, z], 0.12),
        ([right, back, z], 0.12),
        ([right, front, z], 0.12),
        ([left, front, z], 0.12),
    ]);
    eye.fill(painter, &top, |_| top_colour);
    painter.add(Shape::closed_line(
        eye.polygon(&top),
        Stroke::new(1.0, front_colour),
    ));
}

/// `main` on a key top at `centre`, `shifted` above it; `size` of a letter's.
fn legend(painter: &Painter, eye: &Eye, centre: [f32; 3], main: &str, shifted: &str, size: f32) {
    if main.is_empty() {
        return;
    }
    let (scale, at) = (eye.scale(centre), eye.at(centre));
    if shifted.is_empty() {
        let font = FontId::proportional(0.24 * size * scale);
        painter.text(at, Align2::CENTER_CENTER, main, font, LEGEND);
    } else {
        let font = FontId::proportional(0.16 * size * scale);
        painter.text(at, Align2::CENTER_BOTTOM, shifted, font.clone(), LEGEND);
        painter.text(at, Align2::CENTER_TOP, main, font, LEGEND);
    }
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

    /// Across the panel, in inches from its centre.
    fn x(self) -> f32 {
        match self {
            Self::Spacing => -4.4,
            Self::Zoom => -2.5,
            Self::Correct => -0.6,
            Self::Goal => 1.7,
            Self::Save => 4.4,
        }
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

    /// Where `control` takes a click: the knob, its marks and its reading.
    pub fn rect(&self, control: Control) -> Rect {
        let x = control.x();
        let reach = KNOB_RADIUS * INDEX_MARKS.1;
        let corners = [
            on_panel(x - reach, CONTROLS_Y - reach),
            on_panel(x + reach, CONTROLS_Y - reach),
            on_panel(x + reach, CONTROL_READING_Y + 0.12),
            on_panel(x - reach, CONTROL_READING_Y + 0.12),
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
        let centre = on_panel(control.x() - 0.2, CONTROLS_Y);
        self.paint_cylinder(
            painter,
            centre,
            BUTTON_HEIGHT,
            lit,
            [SHIFT_CAP, SHIFT_FRONT],
        );
        let bulb = on_panel(control.x() + 0.45, CONTROLS_Y);
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
        let shadow = circle(panel_offset(base, 0.06, 180.0), KNOB_RADIUS * 1.1);
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

    /// `control`'s engraved name, and its reading under it.
    fn paint_labels(&self, painter: &Painter, control: Control, reading: &str, colour: Color32) {
        let eye = &self.eye;
        let name = on_panel(control.x(), CONTROL_NAME_Y);
        let font = FontId::proportional(0.11 * eye.scale(name));
        painter.text(
            eye.at(name),
            Align2::CENTER_CENTER,
            control.name(),
            font,
            ENGRAVED,
        );
        let at = on_panel(control.x(), CONTROL_READING_Y);
        let font = FontId::proportional(0.14 * eye.scale(at));
        painter.text(eye.at(at), Align2::CENTER_CENTER, reading, font, colour);
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
    const STEPS: u16 = 6;
    let n = corners.len();
    let mut outline = Vec::new();
    for (i, &(corner, rounding)) in corners.iter().enumerate() {
        if rounding <= 0.0 {
            outline.push(corner);
            continue;
        }
        let toward = |j: usize| {
            let other = corners[j].0;
            let d = [0, 1, 2].map(|k| other[k] - corner[k]);
            let length = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt().max(1e-6);
            // At most halfway: neighbouring roundings never cross.
            let t = (rounding / length).min(0.5);
            [0, 1, 2].map(|k| corner[k] + d[k] * t)
        };
        let (from, to) = (toward((i + n - 1) % n), toward((i + 1) % n));
        for step in 0..=STEPS {
            let t = f32::from(step) / f32::from(STEPS);
            outline.push(lerp3(lerp3(from, corner, t), lerp3(corner, to, t), t));
        }
    }
    outline
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
    fn the_controls_stay_in_view_up_to_a_close_zoom() {
        let view = Rect::from_min_size(Pos2::ZERO, vec2(1536.0, 960.0));
        for zoom in [50, 80, 100, 120] {
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
    fn the_table_clears_the_typing_line() {
        // A line is a sixth of an inch: its descenders must show.
        let eye = eye(0.0, 96.0);
        let table = eye.at([0.0, TABLE_BACK.0, TABLE_BACK.1]).y;
        let throat = eye.at([0.0, 0.05, -0.3]).y;
        assert!(table.min(throat) > 96.0 / 6.0 + 4.0, "{table} {throat}");
    }
}
