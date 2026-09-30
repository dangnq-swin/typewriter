//! The carriage, travelling with the sheet: the platen, the carriage's back
//! and side plates, the paper bail. Flat on screen at the sheet's scale.

use std::f32::consts::PI;
use std::ops::RangeInclusive;

use eframe::egui::{Color32, CornerRadius, Mesh, Painter, Rect, Shape, Stroke, StrokeKind, pos2};

use super::geometry::add_quad;
use super::{CHROME, EDGE, METAL, METAL_SHINE};
use crate::render::{Metrics, ruler};

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
const RUBBER: Color32 = Color32::from_rgb(0x16, 0x15, 0x14);
const RUBBER_SHINE: Color32 = Color32::from_rgb(0x55, 0x53, 0x50);
const BAIL_TOP: Color32 = Color32::from_rgb(0xD4, 0xD6, 0xD2);
const BAIL_MID: Color32 = Color32::from_rgb(0xEE, 0xEF, 0xEC);
const BAIL_LOW: Color32 = Color32::from_rgb(0xAE, 0xB0, 0xAC);

/// The carriage's ends, where the knobs are, for a sheet centred at
/// `centre_x`.
pub fn platen_ends(centre_x: f32, metrics: &Metrics) -> [f32; 2] {
    let half = PLATEN_HALF_INCHES * metrics.points_per_inch;
    [centre_x - half, centre_x + half]
}

/// The carriage for the sheet centred at `carriage_x`: its back behind the
/// platen, the platen and its metal ends, and the side plates.
pub(super) fn paint(painter: &Painter, metrics: &Metrics, typing_y: f32, carriage_x: f32) {
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
    let end_ring = PLATEN_END_INCHES * ppi;
    let plate = SIDE_PLATE_INCHES * ppi;
    let radius = PLATEN_DIAMETER_INCHES * ppi / 2.0;
    let (inner_left, inner_right) = (left + plate, right - plate);
    let rubber = inner_left + end_ring..=inner_right - end_ring;
    roller(&mut mesh, rubber, typing_y, radius, [RUBBER, RUBBER_SHINE]);
    for x in [
        inner_left..=inner_left + end_ring,
        inner_right - end_ring..=inner_right,
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
}

/// A horizontal cylinder from the front, lit from above: `[shade, shine]`.
pub(super) fn roller(
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
