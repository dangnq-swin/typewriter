//! Everything drawn: sheets, platen view, folder, plates, settings, PDF.

pub mod background;
pub mod calm;
pub mod feed;
pub mod folder;
pub mod note;
pub mod paper;
pub mod pdf;
pub mod platen;
pub mod ruler;
pub mod scrunch;
pub mod settings;

use eframe::egui::{Color32, FontFamily, FontId, Vec2, vec2};
use typewriter_core::Profile;

pub const FONT_FAMILY: &str = "typewriter";
pub const COURIER_PRIME: &[u8] =
    include_bytes!("../../../../assets/fonts/courier-prime/CourierPrime-Regular.ttf");

/// Every Courier glyph advances 0.6 em.
const COURIER_ADVANCE_EM: f32 = 0.6;
pub const MM_PER_INCH: f32 = 25.4;

/// A sheet drawn flat (folder, icons, cards).
pub const SHEET: Color32 = Color32::from_rgb(0xF7, 0xF4, 0xEC);
pub const SHEET_EDGE: Color32 = Color32::from_rgb(0xA8, 0xA0, 0x92);
/// Hovered controls and the typing pointer.
pub const HIGHLIGHT: Color32 = Color32::from_rgb(0x80, 0x30, 0x20);

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

    pub fn cell_size(&self) -> Vec2 {
        vec2(self.column_width, self.half_line_height * 2.0)
    }
}

/// Eases 0..=1 in and out. Clamps `t`.
pub fn smoothstep(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
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
}
