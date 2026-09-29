//! Everything drawn: sheets, platen view, folder, plates, scratchpad,
//! settings, PDF.

pub mod background;
pub mod calendar;
pub mod calm;
pub mod feed;
pub mod folder;
pub mod holder;
pub mod knob;
pub mod note;
pub mod pad;
pub mod paper;
pub mod pdf;
pub mod platen;
pub mod ruler;
pub mod scratchpad;
pub mod scrunch;
pub mod settings;

use std::collections::HashSet;

use eframe::egui::{Color32, FontFamily, FontId, Rect, Sense, Shape, Stroke, Vec2, vec2};
use typewriter_core::Profile;

pub const FONT_FAMILY: &str = "typewriter";
pub const COURIER_PRIME: &[u8] =
    include_bytes!("../../../../assets/fonts/courier-prime/CourierPrime-Regular.ttf");

/// Every Courier glyph advances 0.6 em.
const COURIER_ADVANCE_EM: f32 = 0.6;
/// Courier Prime's hhea ascent: a glyph's top to its baseline, in em.
pub const COURIER_ASCENT_EM: f32 = 1600.0 / 2048.0;
/// How far `l` rises and `g` drops from the baseline, in em.
const COURIER_ASCENDER_EM: f32 = 1312.0 / 2048.0;
const COURIER_DESCENDER_EM: f32 = 410.0 / 2048.0;
pub const MM_PER_INCH: f32 = 25.4;

/// A sheet drawn flat (folder, icons, cards).
pub const SHEET: Color32 = Color32::from_rgb(0xF7, 0xF4, 0xEC);
pub const SHEET_EDGE: Color32 = Color32::from_rgb(0xA8, 0xA0, 0x92);
/// Hovered controls and the typing pointer.
pub const HIGHLIGHT: Color32 = Color32::from_rgb(0x80, 0x30, 0x20);

/// Clickable, never focused: Tab and Enter belong to the typewriter, and a
/// focused control would take Enter as a click.
pub const CLICK: Sense = Sense::CLICK;
/// [`CLICK`], and draggable.
pub const CLICK_AND_DRAG: Sense = Sense::CLICK.union(Sense::DRAG);

/// Page geometry in screen points.
#[derive(Debug, Clone)]
pub struct Metrics {
    pub points_per_inch: f32,
    pub column_width: f32,
    pub half_line_height: f32,
    pub paper_size: Vec2,
    /// Cell (0, 0) from the paper's top-left. Centred: whole columns never
    /// fill the sheet exactly.
    pub grid_origin: Vec2,
    pub font: FontId,
}

impl Metrics {
    pub fn new(profile: &Profile, points_per_inch: f32) -> Self {
        let column_width = points_per_inch / f32::from(profile.pitch_cpi);
        let half_line_height = points_per_inch / f32::from(profile.lines_per_inch) / 2.0;
        let paper_size = vec2(
            profile.paper.width_mm as f32 / MM_PER_INCH * points_per_inch,
            profile.paper.height_mm as f32 / MM_PER_INCH * points_per_inch,
        );
        let grid = vec2(
            f32::from(profile.columns()) * column_width,
            f32::from(profile.half_lines()) * half_line_height,
        );
        Self {
            points_per_inch,
            column_width,
            half_line_height,
            paper_size,
            grid_origin: (paper_size - grid) / 2.0,
            font: FontId::new(
                column_width / COURIER_ADVANCE_EM,
                FontFamily::Name(FONT_FAMILY.into()),
            ),
        }
    }

    /// A cell's top-left from the paper's. Cells are a column by a full line.
    pub fn cell_offset(&self, half_line: u16, column: u16) -> Vec2 {
        self.grid_origin
            + vec2(
                f32::from(column) * self.column_width,
                f32::from(half_line) * self.half_line_height,
            )
    }

    /// Ascender, baseline and descender heights below a cell's top.
    pub fn type_lines(&self) -> [f32; 3] {
        let size = self.font.size;
        let baseline = COURIER_ASCENT_EM * size;
        [
            baseline - COURIER_ASCENDER_EM * size,
            baseline,
            baseline + COURIER_DESCENDER_EM * size,
        ]
    }

    pub fn cell_size(&self) -> Vec2 {
        vec2(self.column_width, self.half_line_height * 2.0)
    }
}

/// Eases 0..=1 in and out. Clamps `t`.
pub fn smoothstep(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Where TrueType table `tag` starts in `font`.
pub fn font_table(font: &[u8], tag: &[u8; 4]) -> Option<usize> {
    let count = usize::from(u16_at(font, 4)?);
    (0..count)
        .map(|i| 12 + 16 * i)
        .find(|&record| font.get(record..record + 4) == Some(tag))
        .and_then(|record| u32_at(font, record + 8))
        .and_then(|offset| usize::try_from(offset).ok())
}

/// The characters `font` has a glyph for, from its format-4 character map
/// (the Basic Multilingual Plane). Empty if it has none.
pub fn font_characters(font: &[u8]) -> HashSet<char> {
    format_4_characters(font).unwrap_or_default()
}

fn format_4_characters(font: &[u8]) -> Option<HashSet<char>> {
    let cmap = font_table(font, b"cmap")?;
    let subtables = usize::from(u16_at(font, cmap + 2)?);
    let table = (0..subtables).find_map(|i| {
        let offset = usize::try_from(u32_at(font, cmap + 8 + 8 * i)?).ok()?;
        (u16_at(font, cmap + offset)? == 4).then_some(cmap + offset)
    })?;
    // Four parallel arrays of segments: ends, (a pad,) starts, deltas and
    // offsets into the glyph array.
    let segments = usize::from(u16_at(font, table + 6)?) / 2;
    let ends = table + 14;
    let starts = ends + 2 * segments + 2;
    let deltas = starts + 2 * segments;
    let offsets = deltas + 2 * segments;
    let mut characters = HashSet::new();
    for s in 0..segments {
        let (end, start) = (u16_at(font, ends + 2 * s)?, u16_at(font, starts + 2 * s)?);
        let delta = u16_at(font, deltas + 2 * s)?;
        let offset = u16_at(font, offsets + 2 * s)?;
        for code in start..=end {
            let glyph = if offset == 0 {
                code.wrapping_add(delta)
            } else {
                let at = offsets + 2 * s + usize::from(offset) + 2 * usize::from(code - start);
                match u16_at(font, at)? {
                    0 => 0,
                    glyph => glyph.wrapping_add(delta),
                }
            };
            if glyph != 0
                && let Some(c) = char::from_u32(code.into())
            {
                characters.insert(c);
            }
        }
    }
    Some(characters)
}

fn u16_at(bytes: &[u8], at: usize) -> Option<u16> {
    Some(u16::from_be_bytes(bytes.get(at..at + 2)?.try_into().ok()?))
}

fn u32_at(bytes: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_be_bytes(bytes.get(at..at + 4)?.try_into().ok()?))
}

/// A turn button's arrow in `rect`: back (-1) points left, on (1) right.
pub fn chevron(rect: Rect, direction: isize, stroke: Stroke) -> [Shape; 2] {
    let (tip, back) = if direction < 0 {
        (rect.left_center() + vec2(0.25 * rect.width(), 0.0), 0.35)
    } else {
        (rect.right_center() - vec2(0.25 * rect.width(), 0.0), -0.35)
    };
    let arm = vec2(back * rect.width(), 0.3 * rect.height());
    [
        Shape::line_segment([tip, tip + arm], stroke),
        Shape::line_segment([tip, tip + vec2(arm.x, -arm.y)], stroke),
    ]
}

/// `v` turned by `angle` radians: clockwise on screen, where y is down.
pub fn rotate(v: Vec2, angle: f32) -> Vec2 {
    let (sin, cos) = angle.sin_cos();
    vec2(v.x * cos - v.y * sin, v.x * sin + v.y * cos)
}

/// Bits `shift..shift + 16` of `bits` as 0..=1.
pub fn unit(bits: u64, shift: u32) -> f32 {
    ((bits >> shift) & 0xFFFF) as f32 / 65535.0
}

/// A well-mixed hash: neighbouring seeds must not vary in step.
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
    fn pica_at_96_points_per_inch() {
        let profile =
            Profile::from_toml_str(include_str!("../../../../profiles/olympia-sm9.toml")).unwrap();
        let m = Metrics::new(&profile, 96.0);
        assert!((m.column_width - 9.6).abs() < 1e-4);
        assert!((m.font.size - 16.0).abs() < 1e-4);
        assert!((m.cell_size().y - 16.0).abs() < 1e-4);
    }

    #[test]
    fn courier_prime_maps_latin_but_not_cyrillic_or_symbols() {
        let characters = font_characters(COURIER_PRIME);
        for c in ['a', 'Z', '\u{e9}', '\u{2014}', '\u{a7}', '\u{201c}'] {
            assert!(characters.contains(&c), "{c}");
        }
        for c in ['\u{416}', '\u{3b1}', '\u{2715}', '\u{2191}'] {
            assert!(!characters.contains(&c), "{c}");
        }
        assert!(font_characters(b"not a font").is_empty());
    }

    #[test]
    fn type_lines_run_top_to_bottom_inside_the_cell() {
        let profile =
            Profile::from_toml_str(include_str!("../../../../profiles/olympia-sm9.toml")).unwrap();
        let m = Metrics::new(&profile, 96.0);
        let [ascender, baseline, descender] = m.type_lines();
        assert!(0.0 < ascender && ascender < baseline && baseline < descender);
        assert!(descender < m.cell_size().y);
    }
}
