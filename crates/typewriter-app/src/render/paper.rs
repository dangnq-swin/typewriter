//! The sheet and everything struck or painted on it.

use eframe::egui::{
    Align2, Color32, CornerRadius, Painter, Pos2, Rect, Shape, Stroke, Vec2, pos2, vec2,
};
use typewriter_core::carriage::Carriage;
use typewriter_core::page::{Correction, Mark, Page};

use super::calm::Dimming;
use super::{MM_PER_INCH, Metrics};

pub const INK: Color32 = Color32::from_rgba_premultiplied(0x1C, 0x1A, 0x18, 0xEB);
/// Chalk from a correction slip: a slightly cool, matt white.
const CHALK: Color32 = Color32::from_rgb(0xF6, 0xF5, 0xF0);
/// How far chalk spreads past the type's strokes, in points at 96 per inch.
const CHALK_SPREAD: f32 = 0.6;
/// Dry correction fluid, a touch whiter than the paper, and wet.
const FLUID: Color32 = Color32::from_rgb(0xFA, 0xF9, 0xF4);
const FLUID_WET: Color32 = Color32::from_rgb(0xFE, 0xFE, 0xFD);
const FLUID_RIM: Color32 = Color32::from_rgba_premultiplied(0x40, 0x3C, 0x34, 0x22);
/// What an eraser leaves of the ink it rubs out.
const ERASER_GHOST: f32 = 0.12;
const SCUFF: Color32 = Color32::from_rgba_premultiplied(0x40, 0x40, 0x3E, 0x40);
const SCUFF_STREAK: Color32 = Color32::from_rgba_premultiplied(0x10, 0x0F, 0x0E, 0x18);
/// How far smudged ink runs, in points at 96 per inch.
const SMUDGE_SPREAD: f32 = 1.2;
const FRAME: Color32 = Color32::from_rgba_premultiplied(0x4A, 0x46, 0x40, 0x8C);
const FRAME_EXTENSION: Color32 = Color32::from_rgba_premultiplied(0x25, 0x23, 0x20, 0x46);
/// Clearance between the margins and the frame, so type never touches it.
pub const FRAME_PADDING_MM: f32 = 1.0;
/// Ink realism: how far a strike may land off its cell (in points at 96 per
/// inch) and how much lighter it may print.
const INK_MAX_OFFSET: f32 = 0.4;
const INK_DENSITY_VARIANCE: f32 = 0.1;

/// The sheet has no fill of its own. Its extent is shown by a frame around
/// the writing area whose sides run out to the paper's edges. The profile
/// has no bottom margin (the SM9 types down to the last line), so the frame
/// runs to the bottom of the sheet.
pub fn paint_margin_frame(
    painter: &Painter,
    metrics: &Metrics,
    carriage: &Carriage,
    top_lines: u16,
    origin: Pos2,
) {
    let pad = FRAME_PADDING_MM / MM_PER_INCH * metrics.points_per_inch;
    let sheet = Rect::from_min_size(origin, metrics.paper_size);
    let top_left =
        origin + metrics.cell_offset(top_lines * 2, carriage.left_margin) - Vec2::splat(pad);
    let right = origin.x + metrics.cell_offset(0, carriage.right_margin).x + pad;
    let frame = Rect::from_min_max(top_left, pos2(right, sheet.bottom()));

    let extension = Stroke::new(1.0, FRAME_EXTENSION);
    for y in [frame.top(), frame.bottom()] {
        painter.hline(sheet.x_range(), y, extension);
    }
    for x in [frame.left(), frame.right()] {
        painter.vline(x, sheet.y_range(), extension);
    }
    painter.rect_stroke(
        frame,
        CornerRadius::ZERO,
        Stroke::new(1.0, FRAME),
        eframe::egui::StrokeKind::Middle,
    );
}

/// How freshly correction fluid was dabbed on a cell: 1 just now, 0 dry.
pub type Wetness<'a> = &'a dyn Fn(u16, u16) -> f32;

/// A sheet with no fluid drying on it.
pub fn dry(_: u16, _: u16) -> f32 {
    0.0
}

/// One thing to draw for a sheet, whatever it is drawn on: the window or a
/// PDF. Positions are in the sheet's points, `y` down.
#[derive(Debug, Clone, PartialEq)]
pub enum Drawn {
    /// A character with its top-left corner at `at`.
    Glyph {
        at: Pos2,
        c: char,
        color: Color32,
    },
    Patch {
        rect: Rect,
        color: Color32,
    },
    Line {
        from: Pos2,
        to: Pos2,
        width: f32,
        color: Color32,
    },
    /// With a thin rim of another colour just inside its edge.
    Ellipse {
        centre: Pos2,
        radius: Vec2,
        fill: Color32,
        rim: Option<Color32>,
    },
}

/// Width of the rim on a dab of correction fluid, in points.
pub const RIM_WIDTH: f32 = 0.8;

/// Draws a sheet in the window. See [`sheet_marks`].
pub fn paint_sheet(
    painter: &Painter,
    metrics: &Metrics,
    page: &Page,
    origin: Pos2,
    ink_realism: bool,
    dimming: Dimming,
    wetness: Wetness<'_>,
) {
    let clip = painter.clip_rect();
    let marks = sheet_marks(
        metrics,
        page,
        origin,
        ink_realism,
        dimming,
        wetness,
        |cell| clip.intersects(cell),
    );
    for drawn in marks {
        match drawn {
            Drawn::Glyph { at, c, color } => {
                painter.text(at, Align2::LEFT_TOP, c, metrics.font.clone(), color);
            }
            Drawn::Patch { rect, color } => {
                painter.rect_filled(rect, CornerRadius::same(3), color);
            }
            Drawn::Line {
                from,
                to,
                width,
                color,
            } => {
                painter.line_segment([from, to], Stroke::new(width, color));
            }
            Drawn::Ellipse {
                centre,
                radius,
                fill,
                rim,
            } => {
                painter.add(Shape::ellipse_filled(centre, radius, fill));
                if let Some(rim) = rim {
                    painter.add(Shape::ellipse_stroke(
                        centre,
                        radius - Vec2::splat(RIM_WIDTH / 2.0),
                        Stroke::new(RIM_WIDTH, rim),
                    ));
                }
            }
        }
    }
}

/// Everything struck or painted on a sheet whose top-left is at `origin`,
/// in the order it was made, so a correction covers whatever was struck
/// before it. Cells for which `visible` is false are left out.
///
/// `ink_realism` varies each strike slightly, like uneven key pressure and
/// type slugs that do not land exactly in place. `dimming` fades the ink,
/// but not the corrections: a faded patch would show what it covers.
pub fn sheet_marks(
    metrics: &Metrics,
    page: &Page,
    origin: Pos2,
    ink_realism: bool,
    dimming: Dimming,
    wetness: Wetness<'_>,
    visible: impl Fn(Rect) -> bool,
) -> Vec<Drawn> {
    let scale = metrics.points_per_inch / 96.0;
    let mut out = Vec::new();
    for ((half_line, column), cell) in page.cells() {
        let cell_rect = Rect::from_min_size(
            origin + metrics.cell_offset(half_line, column),
            metrics.cell_size(),
        );
        // Fluid spills a little past its cell.
        if !visible(cell_rect.expand(metrics.column_width)) {
            continue;
        }
        let ink = INK.gamma_multiply(dimming.opacity(half_line));
        let marks = cell.marks();
        for (index, mark) in marks.iter().enumerate() {
            let seed = mark_seed(half_line, column, index);
            let (offset, density) = if ink_realism {
                ink_variation(half_line, column, index)
            } else {
                (Vec2::ZERO, 1.0)
            };
            let at = cell_rect.min + offset * scale;
            match mark {
                Mark::Glyph(c) | Mark::Smudged(c) => {
                    // An eraser never gets all the ink out.
                    let rubbed = marks[index + 1..]
                        .iter()
                        .filter(|m| matches!(m, Mark::Correction(Correction::Eraser)))
                        .count();
                    let color = ink.gamma_multiply(density * ERASER_GHOST.powi(rubbed as i32));
                    if matches!(mark, Mark::Smudged(_)) {
                        smudged(&mut out, at, *c, color, scale, seed);
                    } else {
                        out.push(Drawn::Glyph { at, c: *c, color });
                    }
                }
                Mark::Correction(Correction::Eraser) => scuff(&mut out, cell_rect, scale, seed),
                Mark::Correction(Correction::Chalk(c)) => chalk(&mut out, at, *c, scale),
                Mark::Correction(Correction::Fluid { .. }) => {
                    fluid(&mut out, cell_rect, wetness(half_line, column), seed);
                }
            }
        }
    }
    out
}

/// The fibres the eraser roughed up catch the light: a paler patch with a
/// few streaks along the rubbing.
fn scuff(out: &mut Vec<Drawn>, cell: Rect, scale: f32, seed: u64) {
    let patch = cell.expand2(vec2(1.5, -1.0) * scale);
    out.push(Drawn::Patch {
        rect: patch,
        color: SCUFF,
    });
    for i in 0..3_u32 {
        let unit = ((seed >> (i * 16)) & 0xFFFF) as f32 / 65535.0;
        let y = patch.top() + patch.height() * (0.2 + 0.6 * unit);
        let inset = patch.width() * 0.15 * unit;
        out.push(Drawn::Line {
            from: pos2(patch.left() + inset, y),
            to: pos2(patch.right() - inset, y),
            width: 0.8 * scale,
            color: SCUFF_STREAK,
        });
    }
}

/// The chalk goes on in the shape of the character struck through the slip,
/// a little fuller than the type, so a slightly misaligned strike leaves the
/// edges of the ink below.
fn chalk(out: &mut Vec<Drawn>, at: Pos2, c: char, scale: f32) {
    let spread = CHALK_SPREAD * scale;
    for offset in [
        vec2(0.0, 0.0),
        vec2(spread, 0.0),
        vec2(-spread, 0.0),
        vec2(0.0, spread),
        vec2(0.0, -spread),
    ] {
        out.push(Drawn::Glyph {
            at: at + offset,
            c,
            color: CHALK,
        });
    }
}

/// A few overlapping dabs of fluid, uneven and slightly past the cell. While
/// wet it is glossy: brighter, with a highlight.
fn fluid(out: &mut Vec<Drawn>, cell: Rect, wet: f32, seed: u64) {
    let unit = |shift: u32| ((seed >> shift) & 0xFF) as f32 / 255.0 - 0.5;
    let size = cell.size();
    let dabs = [0_u32, 16, 32].map(|shift| {
        let centre = cell.center() + vec2(unit(shift) * 0.3, unit(shift + 8) * 0.2) * size;
        let radius = vec2(0.55 + 0.1 * unit(shift + 4), 0.36 + 0.06 * unit(shift + 12)) * size;
        (centre, radius)
    });
    let fill = lerp_color(FLUID, FLUID_WET, wet);
    for (centre, radius) in dabs {
        out.push(Drawn::Ellipse {
            centre,
            radius,
            fill,
            rim: Some(FLUID_RIM),
        });
    }
    // Painted over again inside, so only the blob's outer rim shows.
    for (centre, radius) in dabs {
        out.push(Drawn::Ellipse {
            centre,
            radius: radius - Vec2::splat(RIM_WIDTH),
            fill,
            rim: None,
        });
    }
    if wet > 0.0 {
        out.push(Drawn::Ellipse {
            centre: cell.center() + vec2(-0.18, -0.16) * size,
            radius: vec2(0.16, 0.07) * size,
            fill: Color32::WHITE.gamma_multiply(wet),
            rim: None,
        });
    }
}

/// Ink that ran on wet fluid: the letter blurred out in a few directions,
/// fainter overall.
fn smudged(out: &mut Vec<Drawn>, at: Pos2, c: char, color: Color32, scale: f32, seed: u64) {
    let unit = |shift: u32| ((seed >> shift) & 0xFF) as f32 / 255.0 - 0.5;
    for i in 0..4_u32 {
        let offset = vec2(unit(i * 16), unit(i * 16 + 8)) * (2.0 * SMUDGE_SPREAD * scale);
        out.push(Drawn::Glyph {
            at: at + offset,
            c,
            color: color.gamma_multiply(0.35),
        });
    }
}

fn lerp_color(a: Color32, b: Color32, t: f32) -> Color32 {
    let mix = |x: u8, y: u8| (f32::from(x) + (f32::from(y) - f32::from(x)) * t).round() as u8;
    Color32::from_rgba_premultiplied(
        mix(a.r(), b.r()),
        mix(a.g(), b.g()),
        mix(a.b(), b.b()),
        mix(a.a(), b.a()),
    )
}

/// Offset and density for one strike. Derived from its place on the page, so
/// it never changes between redraws.
fn ink_variation(half_line: u16, column: u16, index: usize) -> (Vec2, f32) {
    let bits = mark_seed(half_line, column, index);
    let unit = |shift: u32| ((bits >> shift) & 0xFFFF) as f32 / 65535.0;
    let offset = vec2(unit(0) - 0.5, unit(16) - 0.5) * (2.0 * INK_MAX_OFFSET);
    let density = 1.0 - INK_DENSITY_VARIANCE * unit(32);
    (offset, density)
}

/// Random bits for one mark, the same on every redraw.
fn mark_seed(half_line: u16, column: u16, index: usize) -> u64 {
    splitmix64((u64::from(half_line) << 40) ^ (u64::from(column) << 20) ^ index as u64)
}

/// A well-mixed hash, so neighbouring cells do not vary in step.
pub fn splitmix64(mut x: u64) -> u64 {
    x = x.wrapping_add(0x9E37_79B9_7F4A_7C15);
    x = (x ^ (x >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    x = (x ^ (x >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    x ^ (x >> 31)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ink_variation_is_stable_and_subtle() {
        assert_eq!(ink_variation(12, 10, 0), ink_variation(12, 10, 0));
        assert_ne!(ink_variation(12, 10, 0), ink_variation(12, 11, 0));
        for column in 0..200 {
            let (offset, density) = ink_variation(12, column, 0);
            assert!(offset.x.abs() <= INK_MAX_OFFSET && offset.y.abs() <= INK_MAX_OFFSET);
            assert!((1.0 - INK_DENSITY_VARIANCE..=1.0).contains(&density));
        }
    }
}
