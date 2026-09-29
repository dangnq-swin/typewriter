//! Notes pencilled in the top margin of a finished sheet, in a handwriting
//! font (Caveat). Laid out the same on screen and in the PDF.

use std::sync::Arc;

use eframe::egui::{Color32, FontFamily, FontId, Galley, Painter, Pos2, Rect, Vec2, pos2, vec2};
use typewriter_core::profile::Margins;

use super::Metrics;
use super::paper::splitmix64;

pub const PENCIL_FAMILY: &str = "pencil";
pub const CAVEAT: &[u8] = include_bytes!("../../../../assets/fonts/caveat/Caveat-Regular.ttf");
/// Caveat's vertical metrics as shares of the em (hhea: 960 / -300 / 0 per
/// 1000): egui's rows are ascent - descent + line gap high, with the
/// baseline an ascent below their top.
pub const ASCENT_EM: f32 = 0.96;
const LINE_EM: f32 = 1.26;
/// Handwriting is larger than the type.
const SIZE_INCHES: f32 = 0.2;
/// Room between the paper's top edge and the first written line, and between
/// the last one and the margin frame.
const TOP_PAD_INCHES: f32 = 0.12;
const BOTTOM_PAD_INCHES: f32 = 0.06;
/// Soft graphite.
pub const GRAPHITE: Color32 = Color32::from_rgba_premultiplied(0x40, 0x40, 0x46, 0xD8);
/// Written by hand, so each note sits a little askew, the same way every time.
const MAX_TILT_DEGREES: f32 = 0.8;

/// Where a sheet's note goes, in points from the paper's top-left corner.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NoteArea {
    /// Top-left of the first line.
    pub origin: Pos2,
    pub width: f32,
    /// The handwriting's em size.
    pub size: f32,
    /// From one written line to the next.
    pub pitch: f32,
    pub max_lines: usize,
    /// Clockwise, in radians.
    pub angle: f32,
    /// The bottom of the space it may take: just above the margin frame.
    pub bottom: f32,
}

impl NoteArea {
    /// From the left margin to the right one, in the top margin, for the
    /// finished sheet `index`.
    pub fn new(metrics: &Metrics, margins: &Margins, index: usize) -> Self {
        let inch = metrics.points_per_inch;
        let size = SIZE_INCHES * inch;
        let pitch = LINE_EM * size;
        let left = metrics.cell_offset(0, margins.left_column).x;
        let right = metrics.cell_offset(0, margins.right_column).x;
        let frame = metrics.cell_offset(margins.top_lines * 2, 0).y
            - super::paper::FRAME_PADDING_MM / super::MM_PER_INCH * inch;
        let top = TOP_PAD_INCHES * inch;
        let bottom = frame - BOTTOM_PAD_INCHES * inch;
        let max_lines = ((bottom - top) / pitch).floor().max(1.0) as usize;
        let bits = splitmix64(index as u64 ^ 0x6E6F7465);
        let unit = (bits & 0xFFFF) as f32 / 65535.0 * 2.0 - 1.0;
        Self {
            origin: pos2(left, top),
            width: right - left,
            size,
            pitch,
            max_lines,
            angle: (unit * MAX_TILT_DEGREES).to_radians(),
            bottom,
        }
    }

    pub fn font(&self) -> FontId {
        FontId::new(self.size, FontFamily::Name(PENCIL_FAMILY.into()))
    }

    /// Top-left of written line `line`, turned with the note, on a sheet
    /// whose top-left corner is at `paper`.
    pub fn line_origin(&self, paper: Pos2, line: usize) -> Pos2 {
        paper + self.origin.to_vec2() + self.rotate(vec2(0.0, line as f32 * self.pitch))
    }

    /// Where line `line`'s baseline starts.
    pub fn baseline(&self, paper: Pos2, line: usize) -> Pos2 {
        self.line_origin(paper, line) + self.rotate(vec2(0.0, ASCENT_EM * self.size))
    }

    /// The top margin across the whole sheet: clicking it starts a note.
    pub fn margin_rect(&self, paper: Pos2, paper_width: f32) -> Rect {
        Rect::from_min_max(paper, paper + vec2(paper_width, self.bottom))
    }

    /// The space the note is written in, unturned, for the text field.
    pub fn writing_rect(&self, paper: Pos2) -> Rect {
        Rect::from_min_size(
            paper + self.origin.to_vec2(),
            vec2(self.width, self.pitch * self.max_lines as f32),
        )
    }

    fn rotate(&self, v: Vec2) -> Vec2 {
        let (sin, cos) = self.angle.sin_cos();
        vec2(v.x * cos - v.y * sin, v.x * sin + v.y * cos)
    }
}

/// Draws `note` line by line, each line turned with the note.
pub fn paint_note(painter: &Painter, area: &NoteArea, paper: Pos2, note: &str, opacity: f32) {
    let color = GRAPHITE.gamma_multiply(opacity);
    for (i, line) in note.lines().enumerate() {
        let galley = painter.layout_no_wrap(line.to_owned(), area.font(), color);
        painter.add(
            eframe::egui::epaint::TextShape::new(area.line_origin(paper, i), galley, color)
                .with_angle(area.angle),
        );
    }
}

/// The note as written on the paper: one line per row of `galley`, wrapped
/// lines included, so it looks the same wherever it is drawn.
pub fn written_lines(galley: &Arc<Galley>) -> String {
    galley
        .rows
        .iter()
        .map(|row| row.row.text())
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use typewriter_core::Profile;

    fn sm9() -> Profile {
        Profile::from_toml_str(include_str!("../../../../profiles/olympia-sm9.toml")).unwrap()
    }

    #[test]
    fn about_three_lines_fit_in_the_sm9s_top_margin() {
        let profile = sm9();
        let metrics = Metrics::new(&profile, 96.0);
        let area = NoteArea::new(&metrics, &profile.margins, 0);
        assert_eq!(area.max_lines, 3);
        assert!(area.writing_rect(Pos2::ZERO).bottom() <= area.bottom + 1e-3);
        assert!(area.angle.abs() <= MAX_TILT_DEGREES.to_radians());
    }

    #[test]
    fn the_same_sheet_is_always_tilted_the_same_way() {
        let profile = sm9();
        let metrics = Metrics::new(&profile, 96.0);
        let a = NoteArea::new(&metrics, &profile.margins, 3);
        assert_eq!(a, NoteArea::new(&metrics, &profile.margins, 3));
        assert_ne!(a.angle, NoteArea::new(&metrics, &profile.margins, 4).angle);
    }

    #[test]
    fn caveat_metrics_match_the_font() {
        // hhea ascender, descender, line gap at byte offsets 4, 6, 8 of the
        // table; units per em at offset 18 of head.
        let table = |tag: &[u8]| {
            let count = u16::from_be_bytes([CAVEAT[4], CAVEAT[5]]) as usize;
            (0..count)
                .map(|i| &CAVEAT[12 + 16 * i..28 + 16 * i])
                .find(|record| &record[..4] == tag)
                .map(|record| u32::from_be_bytes([record[8], record[9], record[10], record[11]]))
                .unwrap() as usize
        };
        let int = |at: usize| f32::from(i16::from_be_bytes([CAVEAT[at], CAVEAT[at + 1]]));
        let head = table(b"head");
        let hhea = table(b"hhea");
        let em = f32::from(u16::from_be_bytes([CAVEAT[head + 18], CAVEAT[head + 19]]));
        assert_eq!(int(hhea + 4) / em, ASCENT_EM);
        assert!(((int(hhea + 4) - int(hhea + 6) + int(hhea + 8)) / em - LINE_EM).abs() < 1e-6);
    }
}
