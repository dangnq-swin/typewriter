//! Drawing the sheet and the platen view.

pub mod paper;
pub mod platen;
pub mod ruler;

use eframe::egui::{FontFamily, FontId, Vec2, vec2};
use typewriter_core::Profile;

pub const FONT_FAMILY: &str = "typewriter";

/// Courier's advance width is 0.6 em for every glyph.
const COURIER_ADVANCE_EM: f32 = 0.6;
const MM_PER_INCH: f32 = 25.4;

/// Physical page geometry converted to screen points.
#[derive(Debug, Clone)]
pub struct Metrics {
    pub points_per_inch: f32,
    pub column_width: f32,
    pub half_line_height: f32,
    pub paper_size: Vec2,
    /// Offset of cell (0, 0) from the paper's top-left. The grid is centred
    /// because whole columns never fill the sheet exactly.
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

    /// Top-left of a cell relative to the paper's top-left. A cell is one
    /// column wide and one full line (two half-lines) tall.
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
