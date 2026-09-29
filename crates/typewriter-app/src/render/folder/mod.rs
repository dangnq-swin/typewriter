//! The folder view: a neat stack in a manila folder on a tilted desk, the
//! scratchpad and the writing log beside it, menus below. Words show as faint
//! ink bars until a sheet is opened ([`sheet`]).

mod menus;
mod sheet;

pub use menus::FolderAction;
pub use sheet::{OpenSheet, show_sheet};

use eframe::egui::{
    Align2, Color32, Context, CornerRadius, CursorIcon, FontId, Id, Mesh, Painter, Pos2, Rect,
    Shape, Stroke, Ui, Vec2, pos2, vec2,
};
use typewriter_core::page::Page;
use typewriter_core::profile::Margins;

use super::calendar::{self, Month};
use super::note::{self, NoteArea};
use super::paper::INK;
use super::perspective::{Camera, OnDesk, Placement, add_quad, contains, corners};
use super::{CLICK, DIM, DeskIcon, HIGHLIGHT, LABEL, Metrics, SHADOW, SHEET, scratchpad};

/// Camera distance, sheet heights. Smaller = stronger perspective.
const CAMERA_DISTANCE: f32 = 2.6;
/// One sheet's thickness, desk units.
const SHEET_THICKNESS: f32 = 0.4;
/// Stack height cap, share of a sheet's height. More sheets pack tighter.
const MAX_STACK: f32 = 0.03;
/// The chosen sheet slides left this share of a sheet width, turning top-left,
/// at its own height: sheets above still lie over it.
const PULL: f32 = 1.0;
const PULL_DEGREES: f32 = 45.0;
const PULL_SECONDS: f32 = 0.25;
/// The scratchpad, lying a little askew.
const BOOK_TILT_DEGREES: f32 = 8.0;
/// The calendar, turned a little towards the folder and leaning back as a
/// tent calendar does.
const CALENDAR_TURN_DEGREES: f32 = -50.0;
const CALENDAR_LEAN_DEGREES: f32 = 20.0;

const MANILA: Color32 = Color32::from_rgb(0xDD, 0xBF, 0x86);
const MANILA_HOVER: Color32 = Color32::from_rgb(0xE8, 0xCD, 0x98);
const MANILA_EDGE: Color32 = Color32::from_rgb(0xA3, 0x85, 0x52);
/// Lighter than elsewhere: stacked edges must not read as lines.
const SHEET_EDGE: Color32 = Color32::from_rgb(0xC9, 0xC2, 0xB4);
const LABEL_DARK: Color32 = Color32::from_rgb(0x5A, 0x48, 0x2A);

/// Manila fill and edge, `lit` when hovered.
fn manila(lit: bool) -> (Color32, Color32) {
    if lit {
        (MANILA_HOVER, HIGHLIGHT)
    } else {
        (MANILA, MANILA_EDGE)
    }
}

/// What the folder view shows.
pub struct Folder<'a> {
    /// Finished, oldest first.
    pub sheets: &'a [Page],
    pub metrics: &'a Metrics,
    pub margins: &'a Margins,
    /// The chosen sheet, pulled out.
    pub selected: usize,
    pub project: ProjectLabel<'a>,
    /// The writing log's month, on the calendar beside the folder.
    pub log: &'a Month,
    /// Something lies over the view: nothing answers the pointer.
    pub covered: bool,
}

/// The project's labels in the folder view.
pub struct ProjectLabel<'a> {
    /// On the folder's tab.
    pub name: &'a str,
    /// Where it is saved, above the folder.
    pub location: &'a str,
    /// Not a draft.
    pub saved: bool,
    /// Session totals below the location, `""` if none.
    pub stats: &'a str,
}

#[derive(Debug, Default)]
pub struct FolderResponse {
    /// The sheet clicked open.
    pub opened: Option<usize>,
    pub action: Option<FolderAction>,
    /// The chosen sheet's outline: where a scrunch starts.
    pub pulled: Option<[Pos2; 4]>,
    /// The scratchpad beside the folder was clicked.
    pub open_scratchpad: bool,
    /// The calendar beside the folder was clicked.
    pub open_log: bool,
}

/// Draws the folder. `renaming` turns the tab into a name field;
/// `renumbering`, the chosen sheet's number into one.
pub fn show_folder(
    ui: &mut Ui,
    view: Rect,
    folder: &Folder<'_>,
    renaming: Option<&mut String>,
    renumbering: Option<&mut String>,
) -> FolderResponse {
    let (sheets, project) = (folder.sheets, &folder.project);
    let painter = ui.painter_at(view);
    painter.rect_filled(view, CornerRadius::ZERO, DIM);
    let layout = Layout::new(view, folder.metrics);
    paint_cover(&painter, &layout);
    let pointer = ui
        .input(|i| i.pointer.hover_pos())
        .filter(|_| !folder.covered);
    let beside = paint_beside(&painter, &layout, folder, pointer);

    let tab_quad = layout.tab().map(|p| layout.camera.project(p, 0.0));
    let renaming_now = renaming.is_some();
    let tab_hovered = !renaming_now && pointer.is_some_and(|p| contains(&tab_quad, p));
    let tab_action = match renaming {
        Some(name) => menus::rename_field(ui, layout.tab_centre(), name),
        None => {
            painter.text(
                layout.tab_centre(),
                Align2::CENTER_CENTER,
                project.name,
                FontId::proportional(12.0),
                if tab_hovered { HIGHLIGHT } else { LABEL_DARK },
            );
            None
        }
    };

    let chosen = (!sheets.is_empty()).then(|| folder.selected.min(sheets.len() - 1));
    let stack = paint_stack(ui, &painter, &layout, folder, chosen, renumbering.is_some());

    // Clicks only open the pulled-out sheet; choosing is keys only.
    let response = ui.interact(view, Id::new("folder"), CLICK);
    let hovered = match (response.hover_pos(), stack.pulled, chosen) {
        (Some(pointer), Some(quad), Some(i)) if contains(&quad, pointer) => Some(i),
        _ => None,
    };
    let mut action = None;
    if let (Some(number), Some(at)) = (renumbering, stack.chosen_label) {
        action = menus::number_field(ui, at, number, sheets.len());
    }
    paint_labels(&painter, view, folder);
    action = action
        .or(tab_action)
        .or_else(|| menus::menus(ui, view, project.saved, !sheets.is_empty()));
    if tab_hovered {
        response.clone().on_hover_text(if project.saved {
            "Rename the project"
        } else {
            "Save the project as a file (Save As\u{2026})"
        });
    }
    if hovered.is_some() || tab_hovered {
        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
    }
    let mut opened = None;
    if response.clicked() && !renaming_now {
        if tab_hovered {
            action = action.or(Some(if project.saved {
                FolderAction::Rename
            } else {
                FolderAction::SaveAs
            }));
        } else {
            opened = hovered;
        }
    }
    // After the folder's own: the book takes clicks over it.
    let book = ui
        .interact(
            Rect::from_points(&beside.book),
            Id::new("scratchpad-book"),
            CLICK,
        )
        .on_hover_text("Scratchpad (1)");
    let calendar = ui
        .interact(
            Rect::from_points(&beside.calendar),
            Id::new("writing-log"),
            CLICK,
        )
        .on_hover_text("Writing log");
    if beside.book_hovered || beside.calendar_hovered {
        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
    }
    FolderResponse {
        opened,
        action,
        pulled: stack.pulled,
        open_scratchpad: beside.book_hovered && book.clicked(),
        open_log: beside.calendar_hovered && calendar.clicked(),
    }
}

/// Puts sheet `index` back in the stack at once: the sheet taking its place
/// must slide out, not start out.
pub fn put_back(ctx: &Context, index: usize) {
    ctx.animate_value_with_time(pull_id(index), 0.0, 0.0);
}

fn pull_id(index: usize) -> Id {
    Id::new(("folder-pull", index))
}

/// The folder's size and place on the desk.
struct Layout {
    /// A sheet's size, desk units.
    size: Vec2,
    camera: Camera,
    /// Desk units per point of the typing view's paper.
    scale: f32,
    /// The back cover's edges: fixed, whatever the count.
    bottom: f32,
    top: f32,
    half_width: f32,
}

impl Layout {
    fn new(view: Rect, metrics: &Metrics) -> Self {
        let aspect = metrics.paper_size.y / metrics.paper_size.x;
        let width = (view.width() * 0.30).min(view.height() * 0.40 / aspect);
        let size = vec2(width, width * aspect);
        // Shift right: centre the folder and the pulled-out sheet together.
        let camera = Camera::new(
            view.center() + vec2(0.45 * PULL * size.x, view.height() * 0.10),
            CAMERA_DISTANCE * size.y,
        );
        Self {
            size,
            camera,
            scale: size.x / metrics.paper_size.x,
            bottom: -0.55 * size.y,
            top: 0.55 * size.y,
            half_width: 0.6 * size.x,
        }
    }

    fn cover(&self) -> [Vec2; 4] {
        let (bottom, top, half) = (self.bottom, self.top, self.half_width);
        [
            vec2(-half, bottom),
            vec2(half, bottom),
            vec2(half, top),
            vec2(-half, top),
        ]
    }

    fn tab(&self) -> [Vec2; 4] {
        let (top, half, size) = (self.top, self.half_width, self.size);
        [
            vec2(-half, top - 1.0),
            vec2(-0.1 * size.x, top - 1.0),
            vec2(-0.14 * size.x, top + 0.07 * size.y),
            vec2(-half + 0.04 * size.x, top + 0.07 * size.y),
        ]
    }

    fn tab_centre(&self) -> Pos2 {
        let at = vec2(-0.35 * self.size.x, self.top + 0.035 * self.size.y);
        self.camera.project(at, 0.0)
    }

    /// `inches` true to scale on the desk, desk units.
    fn desk(&self, metrics: &Metrics, inches: Vec2) -> Vec2 {
        inches * metrics.points_per_inch * self.scale
    }
}

/// Back cover and tab, under everything.
fn paint_cover(painter: &Painter, layout: &Layout) {
    let shadow_offset = vec2(6.0, 10.0);
    let project = |points: [Vec2; 4]| points.map(|p| layout.camera.project(p, 0.0)).to_vec();
    for outline in [project(layout.tab()), project(layout.cover())] {
        let shadow = outline.iter().map(|&p| p + shadow_offset).collect();
        painter.add(Shape::convex_polygon(shadow, SHADOW, Stroke::NONE));
        let (fill, edge) = manila(false);
        painter.add(Shape::convex_polygon(outline, fill, Stroke::new(1.0, edge)));
    }
}

/// The scratchpad and the calendar as drawn beside the folder.
struct Beside {
    book: [Pos2; 4],
    calendar: [Pos2; 4],
    book_hovered: bool,
    calendar_hovered: bool,
}

/// True to scale on the desk right of the folder: the book near, the
/// calendar standing beyond it.
fn paint_beside(
    painter: &Painter,
    layout: &Layout,
    folder: &Folder<'_>,
    pointer: Option<Pos2>,
) -> Beside {
    let (size, metrics) = (layout.size, folder.metrics);
    let left = layout.half_width + 0.12 * size.x;
    let book_size = layout.desk(metrics, scratchpad::BOOK_INCHES);
    let book_centre = vec2(left, layout.bottom + 0.08 * size.y) + book_size / 2.0;
    let calendar_size = layout.desk(metrics, calendar::CALENDAR_INCHES);
    let calendar_foot = vec2(
        left + 0.5 * calendar_size.x,
        book_centre.y + 0.5 * book_size.y + 0.1 * size.y,
    );
    let calendar = OnDesk::standing(
        &layout.camera,
        calendar_size,
        calendar_foot,
        CALENDAR_TURN_DEGREES,
        CALENDAR_LEAN_DEGREES,
    );
    let calendar_hovered = pointer.is_some_and(|p| contains(&calendar.quad, p));
    let card = Rect::from_min_size(Pos2::ZERO, calendar_size);
    calendar.paint(
        painter,
        corners(card).to_vec(),
        calendar::Layout::new(calendar_size).paint(painter, folder.log, None, calendar_hovered),
    );
    let book = OnDesk::lying(&layout.camera, book_size, book_centre, BOOK_TILT_DEGREES);
    let book_hovered = pointer.is_some_and(|p| contains(&book.quad, p));
    book.paint(
        painter,
        scratchpad::outline(book_size),
        scratchpad::cover(painter, book_size, book_hovered),
    );
    Beside {
        book: book.quad,
        calendar: calendar.quad,
        book_hovered,
        calendar_hovered,
    }
}

/// The stack as drawn.
struct Stack {
    /// The chosen sheet's outline.
    pulled: Option<[Pos2; 4]>,
    /// Where the chosen sheet's number is.
    chosen_label: Option<Pos2>,
}

/// The sheets, `chosen` sliding out. `renumbering`: leave its number off.
fn paint_stack(
    ui: &Ui,
    painter: &Painter,
    layout: &Layout,
    folder: &Folder<'_>,
    chosen: Option<usize>,
    renumbering: bool,
) -> Stack {
    let (camera, size, sheets) = (&layout.camera, layout.size, folder.sheets);
    let placements = stack(sheets.len(), size);
    let pull = |i: usize| {
        let target = if chosen == Some(i) { 1.0 } else { 0.0 };
        ui.ctx()
            .animate_value_with_time(pull_id(i), target, PULL_SECONDS)
    };
    let pulls: Vec<f32> = (0..sheets.len()).map(pull).collect();
    let top_sheet = top_of_stack(&pulls);

    // One shadow for the stack: stacked sheets cast none that shows.
    if let Some(last) = placements.last() {
        let shadow = Placement { lift: 0.0, ..*last }
            .quad(camera, size, Vec2::ZERO, size, 0.0)
            .map(|p| p + vec2(1.5, 2.5));
        painter.add(Shape::convex_polygon(shadow.to_vec(), SHADOW, Stroke::NONE));
    }

    let mut drawn = Stack {
        pulled: None,
        chosen_label: None,
    };
    for (i, (page, base)) in sheets.iter().zip(&placements).enumerate() {
        let amount = pulls[i];
        let placement = pulled(*base, size, amount);
        if amount > 0.0 {
            let shadow = placement
                .quad(camera, size, Vec2::ZERO, size, 0.0)
                .map(|p| p + vec2(3.0, 5.0) * amount);
            painter.add(Shape::convex_polygon(shadow.to_vec(), SHADOW, Stroke::NONE));
        }
        let is_chosen = chosen == Some(i);
        let edge = if is_chosen {
            Stroke::new(1.5, HIGHLIGHT)
        } else {
            Stroke::new(1.0, SHEET_EDGE)
        };
        let outline = placement.quad(camera, size, Vec2::ZERO, size, placement.lift);
        painter.add(Shape::convex_polygon(outline.to_vec(), SHEET, edge));
        if is_chosen {
            drawn.pulled = Some(outline);
        }

        // Only the top sheet and sliding ones show words; the rest are
        // covered.
        if amount > 0.0 || top_sheet == Some(i) {
            painter.add(Shape::mesh(faint_writing(
                layout, folder, i, page, &placement,
            )));
            let label = camera.project(
                placement.on_plane(size, vec2(0.94 * size.x, 0.025 * size.y)),
                placement.lift,
            );
            if is_chosen {
                drawn.chosen_label = Some(label);
            }
            if !(is_chosen && renumbering) {
                painter.text(
                    label,
                    Align2::CENTER_CENTER,
                    i + 1,
                    FontId::proportional(10.0),
                    LABEL_DARK,
                );
            }
        }
    }
    drawn
}

/// Sheet `index`'s words as faint ink bars, and its note as pencil strokes
/// about line length.
fn faint_writing(
    layout: &Layout,
    folder: &Folder<'_>,
    index: usize,
    page: &Page,
    placement: &Placement,
) -> Mesh {
    let (camera, size, scale, metrics) =
        (&layout.camera, layout.size, layout.scale, folder.metrics);
    let line = metrics.cell_size().y;
    let mut bars = Mesh::default();
    let ink = INK.gamma_multiply(0.35);
    for (half_line, first, last) in word_runs(page) {
        let top_left = metrics.cell_offset(half_line, first) + vec2(0.0, 0.3 * line);
        let bottom_right = metrics.cell_offset(half_line, last + 1) + vec2(0.0, 0.75 * line);
        let quad = placement.quad(
            camera,
            size,
            top_left * scale,
            bottom_right * scale,
            placement.lift,
        );
        add_quad(&mut bars, quad, ink);
    }
    let area = NoteArea::new(metrics, folder.margins, index);
    let pencil = note::GRAPHITE.gamma_multiply(0.45);
    for (row, line) in page.note().lines().enumerate() {
        let top = area.line_origin(Pos2::ZERO, row) + vec2(0.0, 0.35 * area.size);
        let length = (line.chars().count() as f32 * 0.42 * area.size).min(area.width);
        let quad = placement.quad(
            camera,
            size,
            top.to_vec2() * scale,
            (top + vec2(length, 0.45 * area.size)).to_vec2() * scale,
            placement.lift,
        );
        add_quad(&mut bars, quad, pencil);
    }
    bars
}

/// The keys hint above the folder, where the project is saved, its totals.
fn paint_labels(painter: &Painter, view: Rect, folder: &Folder<'_>) {
    let count = folder.sheets.len();
    let hint = if count == 0 {
        "No finished sheets yet. Insert feeds a new one.".to_owned()
    } else {
        format!(
            "{count} finished sheet{}  ·  arrow keys: choose  ·  Shift + arrow keys: move it  ·  Enter or click: read it  ·  Esc: back to the typewriter",
            if count == 1 { "" } else { "s" }
        )
    };
    let centre = view.center().x;
    painter.text(
        pos2(centre, view.top() + 24.0),
        Align2::CENTER_CENTER,
        hint,
        FontId::proportional(13.0),
        LABEL,
    );
    painter.text(
        pos2(centre, view.top() + 46.0),
        Align2::CENTER_CENTER,
        folder.project.location,
        FontId::monospace(11.0),
        LABEL.gamma_multiply(0.7),
    );
    painter.text(
        pos2(centre, view.top() + 66.0),
        Align2::CENTER_CENTER,
        folder.project.stats,
        FontId::proportional(12.0),
        LABEL.gamma_multiply(0.85),
    );
}

/// The neat stack, oldest at the bottom. Thick folders pack tighter instead
/// of growing.
fn stack(count: usize, size: Vec2) -> Vec<Placement> {
    let thickness = if count > 1 {
        SHEET_THICKNESS.min(MAX_STACK * size.y / (count - 1) as f32)
    } else {
        SHEET_THICKNESS
    };
    (0..count)
        .map(|i| Placement {
            centre: Vec2::ZERO,
            angle: 0.0,
            lift: i as f32 * thickness,
        })
        .collect()
}

/// The highest sheet still resting in the stack (not the newest while that
/// one is pulled out).
fn top_of_stack(pulls: &[f32]) -> Option<usize> {
    pulls.iter().rposition(|&amount| amount <= 0.0)
}

/// `base` slid `amount` (0..=1) of the way out.
fn pulled(base: Placement, size: Vec2, amount: f32) -> Placement {
    Placement {
        centre: base.centre - vec2(amount * PULL * size.x, 0.0),
        angle: amount * PULL_DEGREES.to_radians(),
        lift: base.lift,
    }
}

/// Runs of visible type, as `(half_line, first column, last column)`.
fn word_runs(page: &Page) -> Vec<(u16, u16, u16)> {
    let mut runs: Vec<(u16, u16, u16)> = Vec::new();
    for ((half_line, column), cell) in page.cells() {
        if cell.top_glyph().is_none_or(char::is_whitespace) {
            continue;
        }
        match runs.last_mut() {
            Some(run) if run.0 == half_line && run.2 + 1 == column => run.2 = column,
            _ => runs.push((half_line, column, column)),
        }
    }
    runs
}

/// A small folder, bottom left, that opens the folder view.
pub fn desk_icon(ui: &mut Ui, view: Rect, count: usize, opacity: f32) -> bool {
    let body = Rect::from_min_size(view.left_bottom() + vec2(16.0, -46.0), vec2(48.0, 30.0));
    let icon = DeskIcon {
        hit: body.expand2(vec2(0.0, 6.0)).translate(vec2(0.0, -3.0)),
        id: "folder-icon",
        tip: "Finished sheets (Page Up)",
    };
    icon.show(ui, view, opacity, |painter, hovered| {
        let (fill, edge) = manila(hovered);
        let tab = Rect::from_min_size(body.left_top() + vec2(3.0, -6.0), vec2(18.0, 8.0));
        for (rect, radius) in [(tab, 2), (body, 3)] {
            painter.rect(
                rect,
                CornerRadius::same(radius),
                fill,
                Stroke::new(1.0, edge),
                eframe::egui::StrokeKind::Inside,
            );
        }
        painter.text(
            body.center(),
            Align2::CENTER_CENTER,
            count,
            FontId::proportional(13.0),
            LABEL_DARK,
        );
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_stack_is_neat_with_the_newest_on_top() {
        let size = vec2(100.0, 141.0);
        let placements = stack(4, size);
        for pair in placements.windows(2) {
            assert_eq!(pair[0].centre, pair[1].centre);
            assert_eq!(pair[0].angle, pair[1].angle);
            assert!(pair[0].lift < pair[1].lift);
        }
    }

    #[test]
    fn the_sheet_under_a_pulled_out_newest_is_the_top_of_the_stack() {
        assert_eq!(top_of_stack(&[0.0, 0.0, 0.0]), Some(2));
        assert_eq!(top_of_stack(&[0.0, 0.0, 1.0]), Some(1));
        // Still sliding out: the one under it already shows.
        assert_eq!(top_of_stack(&[0.0, 0.0, 0.4]), Some(1));
        assert_eq!(top_of_stack(&[1.0]), None);
    }

    #[test]
    fn a_thick_folder_packs_its_sheets_tighter() {
        let size = vec2(100.0, 141.0);
        let placements = stack(2000, size);
        let height = placements.last().unwrap().lift;
        assert!((height - MAX_STACK * size.y).abs() < 1e-3);
    }

    #[test]
    fn the_chosen_sheet_slides_out_left_turning_top_left_at_its_height() {
        let size = vec2(100.0, 141.0);
        let base = stack(5, size)[2];
        let out = pulled(base, size, 1.0);
        assert_eq!(out.lift, base.lift);
        assert!(out.centre.x < -0.9 * size.x);
        // Its top-left corner is now further left than its bottom-left.
        let top_left = out.on_plane(size, Vec2::ZERO);
        let bottom_left = out.on_plane(size, vec2(0.0, size.y));
        assert!(top_left.x < bottom_left.x);
        assert!((out.angle.to_degrees() - PULL_DEGREES).abs() < 1e-4);
    }

    #[test]
    fn word_runs_split_at_spaces_and_lines() {
        let mut page = Page::new(20, 20);
        for (i, c) in "ab cd".chars().enumerate() {
            if c != ' ' {
                page.strike(2, i as u16, c);
            }
        }
        page.strike(4, 0, 'x');
        assert_eq!(word_runs(&page), [(2, 0, 1), (2, 3, 4), (4, 0, 0)]);
    }
}
