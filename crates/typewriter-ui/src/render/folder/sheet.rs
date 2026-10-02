//! One finished sheet open, scaled to fit, read-only but for its pencilled
//! note.

use eframe::egui::{
    self, Align2, Color32, CornerRadius, CursorIcon, FontId, Id, Pos2, Rect, Ui, pos2, vec2,
};
use typewriter_core::Profile;
use typewriter_core::carriage::Carriage;
use typewriter_core::page::Page;

use crate::render::calm::Dimming;
use crate::render::note::{self, NoteArea};
use crate::render::{CLICK, DIM, LABEL, MM_PER_INCH, Metrics, SHEET, paper};

/// What the open sheet shows.
pub struct OpenSheet<'a> {
    pub profile: &'a Profile,
    /// For the margin frame.
    pub carriage: &'a Carriage,
    pub page: &'a Page,
    /// Its place among the finished sheets, from 0, and their count.
    pub index: usize,
    pub total: usize,
    /// No closer than the typing view's zoom.
    pub max_points_per_inch: f32,
    pub ink_realism: bool,
}

/// What happened to the open sheet's note.
#[derive(Debug, Default)]
pub struct SheetResponse {
    /// Clicked the top margin: start writing.
    pub start_note: bool,
    /// Finished writing: the note as written.
    pub note_written: Option<String>,
}

/// Draws the sheet. `note`: the note being written, if any. `print` keeps
/// the sheet's marks between frames.
pub fn show_sheet(
    ui: &mut Ui,
    view: Rect,
    sheet: &OpenSheet<'_>,
    note: Option<&mut String>,
    print: &mut paper::SheetPrint,
) -> SheetResponse {
    let profile = sheet.profile;
    let painter = ui.painter_at(view);
    painter.rect_filled(view, CornerRadius::ZERO, DIM);
    let paper_mm = vec2(
        profile.paper.width_mm as f32,
        profile.paper.height_mm as f32,
    );
    let header = 44.0;
    // Fitted in points a millimetre, turned to points an inch for the clamp.
    let fit = ((view.width() - 40.0) / paper_mm.x).min((view.height() - 2.0 * header) / paper_mm.y)
        * MM_PER_INCH;
    let metrics = Metrics::new(profile, fit.min(sheet.max_points_per_inch).max(8.0));
    let origin = view.center() - metrics.paper_size / 2.0;
    let paper_rect = Rect::from_min_size(origin, metrics.paper_size);

    let shadow = egui::Shadow {
        offset: [0, 4],
        blur: 18,
        spread: 0,
        color: Color32::from_black_alpha(110),
    };
    painter.add(shadow.as_shape(paper_rect, CornerRadius::ZERO));
    painter.rect_filled(paper_rect, CornerRadius::ZERO, SHEET);
    paper::paint_margin_frame(
        &painter,
        &metrics,
        sheet.carriage,
        profile.margins.top_lines,
        origin,
    );
    paper::paint_sheet_cached(
        &painter,
        print,
        &paper::SheetLook {
            metrics: &metrics,
            origin,
            ink_realism: sheet.ink_realism,
            dimming: Dimming::NONE,
            wetness: &paper::dry,
            drying: false,
        },
        sheet.page,
    );

    painter.text(
        pos2(view.center().x, paper_rect.top() - header / 2.0),
        Align2::CENTER_CENTER,
        format!("Sheet {} of {}", sheet.index + 1, sheet.total),
        FontId::proportional(14.0),
        LABEL,
    );
    let hint = if note.is_some() {
        "Enter: new line  ·  click elsewhere or Esc: done"
    } else {
        "Click the top margin: pencil a note  ·  arrows or Page Up / Down: flip  ·  Esc: folder  ·  type: back to the typewriter"
    };
    painter.text(
        pos2(view.center().x, paper_rect.bottom() + header / 2.0),
        Align2::CENTER_CENTER,
        hint,
        FontId::proportional(12.0),
        LABEL,
    );

    let area = NoteArea::new(&metrics, &profile.margins, sheet.index);
    let mut response = SheetResponse::default();
    match note {
        Some(text) => response.note_written = note_field(ui, &area, origin, text),
        None => {
            note::paint_note(&painter, &area, origin, sheet.page.note(), 1.0);
            let margin = ui
                .interact(
                    area.margin_rect(origin, metrics.paper_size.x),
                    Id::new("note-margin"),
                    CLICK,
                )
                .on_hover_text("Pencil a note");
            if margin.hovered() {
                ui.ctx().set_cursor_icon(CursorIcon::Text);
            }
            response.start_note = margin.clicked();
        }
    }
    response
}

/// The note being written on the paper. Refuses input past the margin's
/// lines. Esc or a click away finishes.
fn note_field(ui: &mut Ui, area: &NoteArea, paper: Pos2, text: &mut String) -> Option<String> {
    let field = note::PencilField {
        rect: area.writing_rect(paper),
        id: Id::new("note-field"),
        font: area.font(),
        rows: area.max_lines,
    };
    let before = text.clone();
    let written = field.show(ui, text, true);
    if written.galley.rows.len() > field.rows {
        *text = before;
    }
    written.response.lost_focus().then(|| {
        let galley = ui
            .painter()
            .layout(text.clone(), area.font(), note::GRAPHITE, area.width);
        note::written_lines(&galley)
    })
}
