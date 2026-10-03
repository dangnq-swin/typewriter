//! The copy holder: a finished sheet on a stand left of the machine, read
//! while typing (retyping a page), a line guide keeping the place.

use eframe::egui::{self, Color32, CornerRadius, Pos2, Rect, Stroke, StrokeKind, Ui, pos2, vec2};
use typewriter_core::Profile;
use typewriter_core::page::Page;

use super::calm::Dimming;
use super::{HIGHLIGHT, Metrics, SHEET, SHEET_EDGE, paper};

/// Window edge to stand.
const MARGIN: f32 = 24.0;
/// Room left below for the desk icons.
const BOTTOM_ROOM: f32 = 70.0;
/// At most this share of the window's width: the paper needs the rest.
const MOST_WIDTH: f32 = 0.3;
/// Board showing around the sheet.
const BOARD: f32 = 10.0;
const CLOSE_RADIUS: f32 = 9.0;

const BOARD_FILL: Color32 = Color32::from_rgb(0x3A, 0x34, 0x2E);
const BOARD_EDGE: Color32 = Color32::from_rgb(0x22, 0x1E, 0x1A);
const CLIP: Color32 = Color32::from_rgb(0x9C, 0x9A, 0x94);
const CLIP_EDGE: Color32 = Color32::from_rgb(0x5A, 0x58, 0x54);
/// The line under the guide, lit a little.
const GUIDE_BAND: Color32 = Color32::from_rgba_premultiplied(0x30, 0x2C, 0x10, 0x18);
const GUIDE_BAR: Color32 = Color32::from_rgb(0x70, 0x6E, 0x68);

/// A sheet on the stand. A copy: the folder's sheet stays as it is.
#[derive(Debug, Clone)]
pub struct Holder {
    page: Page,
    /// The half-line the guide sits under.
    guide: u16,
}

impl Holder {
    /// The guide starts under the first typed line.
    pub fn new(page: Page) -> Self {
        let guide = (0..page.half_lines())
            .find(|&row| !page.line_text(row).is_empty())
            .unwrap_or(0);
        Self { page, guide }
    }

    /// The typed line nearest `half_line`, or `half_line` on a blank sheet.
    fn nearest_line(&self, half_line: u16) -> u16 {
        (0..self.page.half_lines())
            .filter(|&row| !self.page.line_text(row).is_empty())
            .min_by_key(|&row| row.abs_diff(half_line))
            .unwrap_or(half_line)
    }
}

/// The stand and its sheet at the window's left. True when the sheet is
/// taken down. `print` keeps the sheet's marks between frames.
pub fn show(
    ui: &mut Ui,
    view: Rect,
    profile: &Profile,
    holder: &mut Holder,
    ink: bool,
    print: &mut paper::SheetPrint,
) -> bool {
    let paper_mm = vec2(
        profile.paper.width_mm as f32,
        profile.paper.height_mm as f32,
    );
    let height = view.height() - MARGIN - BOTTOM_ROOM - 2.0 * BOARD;
    let width = view.width() * MOST_WIDTH - MARGIN - 2.0 * BOARD;
    let points_per_mm = (height / paper_mm.y).min(width / paper_mm.x);
    if points_per_mm * super::MM_PER_INCH < 8.0 {
        // No room to read: stay down until the window grows.
        return false;
    }
    let metrics = Metrics::new(profile, points_per_mm * super::MM_PER_INCH);
    let origin = view.left_top() + vec2(MARGIN + BOARD, MARGIN + BOARD);
    let sheet = Rect::from_min_size(origin, metrics.paper_size);
    let board = sheet.expand(BOARD);

    let painter = ui.painter_at(view);
    let shadow = egui::Shadow {
        offset: [0, 4],
        blur: 16,
        spread: 0,
        color: Color32::from_black_alpha(100),
    };
    painter.add(shadow.as_shape(board, CornerRadius::same(4)));
    painter.rect(
        board,
        CornerRadius::same(4),
        BOARD_FILL,
        Stroke::new(1.0, BOARD_EDGE),
        StrokeKind::Inside,
    );
    painter.rect(
        sheet,
        CornerRadius::ZERO,
        SHEET,
        Stroke::new(1.0, SHEET_EDGE),
        StrokeKind::Inside,
    );
    paper::paint_sheet_cached(
        &painter,
        print,
        &paper::SheetLook {
            metrics: &metrics,
            origin,
            ink_realism: ink,
            dimming: Dimming::NONE,
            wetness: &paper::dry,
            drying: false,
            magnify: None,
        },
        &holder.page,
    );

    // The guide: the line lit, a bar just under its descenders.
    let line_top = origin.y + metrics.cell_offset(holder.guide, 0).y;
    let [_, _, descender] = metrics.type_lines();
    let band = Rect::from_x_y_ranges(sheet.x_range(), line_top..=line_top + metrics.cell_size().y);
    painter.rect_filled(band, CornerRadius::ZERO, GUIDE_BAND);
    let bar_top = line_top + descender + 1.0;
    painter.rect_filled(
        Rect::from_x_y_ranges(board.x_range(), bar_top..=bar_top + 4.0),
        CornerRadius::same(1),
        GUIDE_BAR,
    );

    // The spring clip holding the sheet's top.
    let clip = Rect::from_center_size(
        pos2(sheet.center().x, sheet.top()),
        vec2(0.25 * sheet.width(), 1.6 * BOARD),
    );
    painter.rect(
        clip,
        CornerRadius::same(3),
        CLIP,
        Stroke::new(1.0, CLIP_EDGE),
        StrokeKind::Inside,
    );

    let response = super::button(ui, sheet, "copy-holder", "Click a line to move the guide");
    if response.clicked()
        && let Some(pointer) = response.interact_pointer_pos()
    {
        holder.guide = holder.nearest_line(half_line_at(&metrics, origin, pointer));
    }
    close_button(ui, &painter, board)
}

/// The half-line under `pointer`, on a sheet at `origin`.
fn half_line_at(metrics: &Metrics, origin: Pos2, pointer: Pos2) -> u16 {
    let rows = (pointer.y - origin.y - metrics.grid_origin.y) / metrics.half_line_height;
    // Safe cast: clamped at 0, and far below u16::MAX on any screen.
    rows.floor().max(0.0) as u16
}

/// A small ✕ on the board's top-right corner. True when clicked.
fn close_button(ui: &mut Ui, painter: &egui::Painter, board: Rect) -> bool {
    let centre = board.right_top() + vec2(-CLOSE_RADIUS - 3.0, CLOSE_RADIUS + 3.0);
    let rect = Rect::from_center_size(centre, vec2(2.0, 2.0) * CLOSE_RADIUS);
    let response = super::button(ui, rect, "copy-holder-close", "Take the sheet down");
    let color = if response.hovered() { HIGHLIGHT } else { CLIP };
    painter.circle_filled(centre, CLOSE_RADIUS, BOARD_EDGE);
    // Drawn: the interface font has no ✕.
    let arm = 0.4 * CLOSE_RADIUS;
    let stroke = Stroke::new(1.5, color);
    painter.line_segment([centre + vec2(-arm, -arm), centre + vec2(arm, arm)], stroke);
    painter.line_segment([centre + vec2(-arm, arm), centre + vec2(arm, -arm)], stroke);
    response.clicked()
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;

    #[test]
    fn the_guide_starts_on_the_first_line_and_snaps_to_typed_ones() {
        let mut page = Page::new(40, 40);
        page.strike(12, 0, 'a');
        page.strike(16, 0, 'b');
        let holder = Holder::new(page);
        assert_eq!(holder.guide, 12);
        assert_eq!(holder.nearest_line(15), 16);
        assert_eq!(holder.nearest_line(0), 12);
        assert_eq!(Holder::new(Page::new(40, 40)).nearest_line(7), 7);
    }

    #[test]
    fn a_click_finds_its_half_line() {
        let profile =
            Profile::from_toml_str(include_str!("../../../../profiles/olympia-sm9.toml")).unwrap();
        let metrics = Metrics::new(&profile, 96.0);
        let origin = pos2(10.0, 20.0);
        let row_12 = origin.y + metrics.cell_offset(12, 0).y + 1.0;
        assert_eq!(half_line_at(&metrics, origin, pos2(50.0, row_12)), 12);
        assert_eq!(half_line_at(&metrics, origin, pos2(50.0, 0.0)), 0);
    }
}
