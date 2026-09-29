//! Pencilled notes in a finished sheet's top margin, in Caveat. One layout
//! for screen and PDF.

use std::sync::Arc;

use eframe::egui::{
    self, Color32, FontFamily, FontId, Galley, Id, Painter, Pos2, Rect, Response, Stroke, Ui, pos2,
    vec2,
};
use typewriter_core::profile::Margins;

use super::{Metrics, rotate, splitmix64, unit};

pub const PENCIL_FAMILY: &str = "pencil";
pub const CAVEAT: &[u8] = include_bytes!("../../../../assets/fonts/caveat/Caveat-Regular.ttf");
/// Caveat's hhea metrics in em (960 / -300 / 0 per 1000). egui rows are
/// ascent - descent + gap high, baseline an ascent below the top.
pub const ASCENT_EM: f32 = 0.96;
pub const LINE_EM: f32 = 1.26;
/// Em size. Larger than the type, as handwriting is.
pub const SIZE_INCHES: f32 = 0.2;
/// Gaps: paper edge to first line, last line to margin frame.
const TOP_PAD_INCHES: f32 = 0.12;
const BOTTOM_PAD_INCHES: f32 = 0.06;
/// Soft graphite.
pub const GRAPHITE: Color32 = Color32::from_rgba_premultiplied(0x40, 0x40, 0x46, 0xD8);
/// Each note sits a little askew, seeded per sheet.
const MAX_TILT_DEGREES: f32 = 0.8;

/// Where a note goes, in points from the paper's top-left.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NoteArea {
    /// Top-left of the first line.
    pub origin: Pos2,
    pub width: f32,
    /// Em size.
    pub size: f32,
    /// Line to line.
    pub pitch: f32,
    pub max_lines: usize,
    /// Clockwise radians.
    pub angle: f32,
    /// Lowest it may reach: just above the margin frame.
    pub bottom: f32,
}

impl NoteArea {
    /// Margin to margin in the top margin, for finished sheet `index`.
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
        let unit = unit(bits, 0) * 2.0 - 1.0;
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

    /// Top-left of line `line`, turned with the note, on a sheet at `paper`.
    pub fn line_origin(&self, paper: Pos2, line: usize) -> Pos2 {
        paper + self.origin.to_vec2() + rotate(vec2(0.0, line as f32 * self.pitch), self.angle)
    }

    /// Start of line `line`'s baseline.
    pub fn baseline(&self, paper: Pos2, line: usize) -> Pos2 {
        self.line_origin(paper, line) + rotate(vec2(0.0, ASCENT_EM * self.size), self.angle)
    }

    /// The whole top margin: clicking it starts a note.
    pub fn margin_rect(&self, paper: Pos2, paper_width: f32) -> Rect {
        Rect::from_min_max(paper, paper + vec2(paper_width, self.bottom))
    }

    /// The writing space, unturned, for the text field.
    pub fn writing_rect(&self, paper: Pos2) -> Rect {
        Rect::from_min_size(
            paper + self.origin.to_vec2(),
            vec2(self.width, self.pitch * self.max_lines as f32),
        )
    }
}

/// Draws `note` line by line, turned.
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

/// A pencil text field: `rows` lines over `rect`.
pub struct PencilField {
    pub rect: Rect,
    pub id: Id,
    pub font: FontId,
    pub rows: usize,
}

/// What a field did this frame.
pub struct Written {
    pub response: Response,
    /// As laid out: may run past the field's rows.
    pub galley: Arc<Galley>,
    /// The cursor, in chars.
    pub cursor: Option<usize>,
}

impl PencilField {
    /// Shows `text` for editing. `focus`: take focus after this frame, so
    /// the key or click that asked for it isn't written.
    pub fn show(&self, ui: &mut Ui, text: &mut String, focus: bool) -> Written {
        let output = ui
            .scope_builder(egui::UiBuilder::new().max_rect(self.rect), |ui| {
                ui.visuals_mut().text_cursor.stroke = Stroke::new(1.0, GRAPHITE);
                // Invisible here, painted below: the field clips Caveat's
                // overhangs (l, f) at its edges.
                egui::TextEdit::multiline(text)
                    .id(self.id)
                    .font(self.font.clone())
                    .text_color(Color32::TRANSPARENT)
                    .frame(egui::Frame::NONE)
                    .margin(egui::Margin::ZERO)
                    .desired_width(self.rect.width())
                    .desired_rows(self.rows)
                    .show(ui)
            })
            .inner;
        ui.painter().galley_with_override_text_color(
            output.galley_pos,
            output.galley.clone(),
            GRAPHITE,
        );
        let response = output.response.response;
        if focus && !response.has_focus() && !response.lost_focus() {
            response.request_focus();
        }
        Written {
            response,
            galley: output.galley,
            cursor: output.cursor_range.map(|range| range.primary.index.0),
        }
    }

    /// Puts the cursor before char `index`, for the next [`Self::show`].
    pub fn place_cursor(&self, ctx: &egui::Context, index: usize) {
        let mut state = egui::text_edit::TextEditState::load(ctx, self.id).unwrap_or_default();
        let cursor = egui::text::CCursor::new(index);
        state
            .cursor
            .set_char_range(Some(egui::text::CCursorRange::one(cursor)));
        state.store(ctx, self.id);
    }
}

/// The note as written: one line per galley row, wraps included, so every
/// renderer matches.
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
        // hhea: ascender, descender, line gap at offsets 4, 6, 8. head:
        // units per em at 18.
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
