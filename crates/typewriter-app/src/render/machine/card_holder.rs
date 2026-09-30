//! The card holder at the printing point: translucent plates with the
//! aligning scale, the drawing notches, the type guide.

use std::f32::consts::PI;

use eframe::egui::{
    Color32, CornerRadius, Mesh, Painter, Pos2, Rect, Shape, Stroke, StrokeKind, pos2, vec2,
};

use super::carriage::roller;
use super::eye::Eye;
use super::geometry::rounded;
use super::{DARK, EDGE, METAL, METAL_SHINE};
use crate::render::Metrics;

/// The card holder's translucent plates under the platen, either side of
/// the type guide: `(y, z)` back and front, and their inner and outer `x`.
/// The sheet shows through them, and goes out of sight below them.
const TABLE_BACK: (f32, f32) = (0.18, -0.25);
pub(super) const TABLE_FRONT: (f32, f32) = (0.85, -0.45);
const TABLE_X: (f32, f32) = (0.55, 4.35);
/// The aligning scale on the card holder: columns marked either side of the
/// type guide, from the plates' inner ends.
const ALIGNING_COLUMNS: i32 = 12;
/// The metal drawing notches at the plates' inner ends: width and height.
const NOTCH_INCHES: (f32, f32) = (0.17, 0.2);
/// The type guide's arch over the ribbon vibrator: half its width, and the
/// vibrator's half width.
const TYPE_GUIDE_HALF: f32 = 0.2;
const VIBRATOR_HALF: f32 = 0.3;
const GLASS: Color32 = Color32::from_rgba_unmultiplied_const(0xF8, 0xF8, 0xF4, 0x46);
const GLASS_EDGE: Color32 = Color32::from_rgba_unmultiplied_const(0x70, 0x70, 0x6A, 0xB0);

/// The card holder: two translucent plates under the platen with the
/// aligning scale along their tops, a metal drawing notch at each inner end,
/// and the type guide's arch between them over the ribbon vibrator.
pub(super) fn paint(painter: &Painter, eye: &Eye, metrics: &Metrics) {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_table_clears_the_typing_line() {
        // A line is a sixth of an inch: its descenders must show.
        let eye = Eye::testing(0.0, 96.0);
        let table = eye.at([0.0, TABLE_BACK.0, TABLE_BACK.1]).y;
        let throat = eye.at([0.0, 0.05, -0.3]).y;
        assert!(table.min(throat) > 96.0 / 6.0 + 4.0, "{table} {throat}");
    }
}
