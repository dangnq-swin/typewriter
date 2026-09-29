//! The folder view (a neat stack in a manila folder on a tilted desk) and
//! the open, read-only sheet.
//!
//! egui can't draw text in perspective: sheets in the folder show words as
//! faint ink bars until opened.

use eframe::egui::{
    self, Align2, Color32, CornerRadius, CursorIcon, FontId, Id, Mesh, Painter, Pos2, Rect, Shape,
    Stroke, Ui, Vec2, pos2, vec2,
};
use typewriter_core::Profile;
use typewriter_core::carriage::Carriage;
use typewriter_core::page::Page;
use typewriter_core::profile::Margins;

use super::calm::Dimming;
use super::note::{self, NoteArea};
use super::paper::{self, INK};
use super::{HIGHLIGHT, Metrics, SHEET, rotate, scratchpad};
use crate::filing::ExportFormat;

const TILT_DEGREES: f32 = 38.0;
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

const DIM: Color32 = Color32::from_rgba_premultiplied(0x1A, 0x17, 0x14, 0xB4);
const MANILA: Color32 = Color32::from_rgb(0xDD, 0xBF, 0x86);
const MANILA_HOVER: Color32 = Color32::from_rgb(0xE8, 0xCD, 0x98);
const MANILA_EDGE: Color32 = Color32::from_rgb(0xA3, 0x85, 0x52);
/// Lighter than elsewhere: stacked edges must not read as lines.
const SHEET_EDGE: Color32 = Color32::from_rgb(0xC9, 0xC2, 0xB4);
const SHADOW: Color32 = Color32::from_rgba_premultiplied(0, 0, 0, 0x30);
const LABEL: Color32 = Color32::from_rgb(0xEE, 0xE8, 0xDC);
const LABEL_DARK: Color32 = Color32::from_rgb(0x5A, 0x48, 0x2A);

/// Projects the desk plane. Plane `y` points away from the viewer; `lift` is
/// height above it.
struct Camera {
    centre: Pos2,
    distance: f32,
    sin: f32,
    cos: f32,
}

impl Camera {
    fn new(centre: Pos2, distance: f32) -> Self {
        let (sin, cos) = TILT_DEGREES.to_radians().sin_cos();
        Self {
            centre,
            distance,
            sin,
            cos,
        }
    }

    fn project(&self, plane: Vec2, lift: f32) -> Pos2 {
        let up = plane.y * self.cos + lift * self.sin;
        let depth = self.distance + plane.y * self.sin - lift * self.cos;
        let scale = self.distance / depth.max(1.0);
        pos2(self.centre.x + plane.x * scale, self.centre.y - up * scale)
    }
}

/// Where one sheet lies in the folder.
#[derive(Debug, Clone, Copy)]
struct Placement {
    centre: Vec2,
    angle: f32,
    lift: f32,
}

impl Placement {
    /// A sheet point (`y` down the page) on the desk plane.
    fn on_plane(&self, size: Vec2, local: Vec2) -> Vec2 {
        let d = vec2(local.x - size.x / 2.0, size.y / 2.0 - local.y);
        self.centre + rotate(d, self.angle)
    }

    fn quad(&self, camera: &Camera, size: Vec2, min: Vec2, max: Vec2, lift: f32) -> [Pos2; 4] {
        [
            vec2(min.x, min.y),
            vec2(max.x, min.y),
            vec2(max.x, max.y),
            vec2(min.x, max.y),
        ]
        .map(|p| camera.project(self.on_plane(size, p), lift))
    }
}

/// The scratchpad, true to scale, on the desk to the folder's right.
struct Book<'a> {
    camera: &'a Camera,
    size: Vec2,
    centre: Vec2,
    /// Its corners on screen.
    quad: [Pos2; 4],
}

impl<'a> Book<'a> {
    /// `size`: a sheet's; `folder_half`, `folder_bottom`: the folder's
    /// right edge and near edge, desk units.
    fn beside(
        camera: &'a Camera,
        size: Vec2,
        metrics: &Metrics,
        folder_half: f32,
        folder_bottom: f32,
    ) -> Self {
        let book =
            scratchpad::BOOK_INCHES * metrics.points_per_inch * size.x / metrics.paper_size.x;
        let centre = vec2(
            folder_half + 0.12 * size.x + book.x / 2.0,
            folder_bottom + 0.08 * size.y + book.y / 2.0,
        );
        let mut this = Self {
            camera,
            size: book,
            centre,
            quad: [Pos2::ZERO; 4],
        };
        this.quad = [
            Pos2::ZERO,
            pos2(book.x, 0.0),
            pos2(book.x, book.y),
            pos2(0.0, book.y),
        ]
        .map(|p| this.to_screen(p));
        this
    }

    /// A point on the cover (from its top-left, y down) on screen.
    fn to_screen(&self, p: Pos2) -> Pos2 {
        let d = vec2(p.x - self.size.x / 2.0, self.size.y / 2.0 - p.y);
        let on_desk = self.centre + rotate(d, BOOK_TILT_DEGREES.to_radians());
        self.camera.project(on_desk, 0.0)
    }

    fn paint(&self, painter: &Painter, hovered: bool) {
        let outline: Vec<Pos2> = scratchpad::outline(self.size)
            .into_iter()
            .map(|p| self.to_screen(p) + vec2(3.0, 5.0))
            .collect();
        painter.add(Shape::convex_polygon(outline, SHADOW, Stroke::NONE));
        let cover = scratchpad::cover(painter, self.size, hovered);
        painter.add(Shape::mesh(scratchpad::warp(painter, cover, |p| {
            self.to_screen(p)
        })));
    }
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

fn contains(quad: &[Pos2; 4], p: Pos2) -> bool {
    let side = |a: Pos2, b: Pos2| (b - a).x * (p - a).y - (b - a).y * (p - a).x;
    let signs = [
        side(quad[0], quad[1]),
        side(quad[1], quad[2]),
        side(quad[2], quad[3]),
        side(quad[3], quad[0]),
    ];
    signs.iter().all(|&s| s >= 0.0) || signs.iter().all(|&s| s <= 0.0)
}

fn add_quad(mesh: &mut Mesh, quad: [Pos2; 4], color: Color32) {
    let first = mesh.vertices.len() as u32;
    for p in quad {
        mesh.colored_vertex(p, color);
    }
    mesh.add_triangle(first, first + 1, first + 2);
    mesh.add_triangle(first, first + 2, first + 3);
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

/// What the folder's controls ask for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FolderAction {
    Save,
    SaveAs,
    /// Start renaming on the tab.
    Rename,
    RenameTo(String),
    CancelRename,
    New,
    Open,
    Export(ExportFormat),
    /// Start typing the chosen sheet's new number.
    Renumber,
    RenumberTo(String),
    CancelRenumber,
    /// Ask to scrunch up the chosen sheet.
    Scrunch,
    /// Roll the chosen sheet back into the machine.
    RollIn,
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

/// Draws the folder with sheet `selected` pulled out. `renaming` turns the
/// tab into a name field; `renumbering`, the chosen sheet's number into one.
#[allow(clippy::too_many_arguments)]
pub fn show_folder(
    ui: &mut Ui,
    view: Rect,
    sheets: &[Page],
    metrics: &Metrics,
    margins: &Margins,
    selected: usize,
    project: &ProjectLabel<'_>,
    renaming: Option<&mut String>,
    renumbering: Option<&mut String>,
) -> FolderResponse {
    let painter = ui.painter_at(view);
    painter.rect_filled(view, CornerRadius::ZERO, DIM);

    let aspect = metrics.paper_size.y / metrics.paper_size.x;
    let width = (view.width() * 0.30).min(view.height() * 0.40 / aspect);
    let size = vec2(width, width * aspect);
    // Shift right: centre the folder and the pulled-out sheet together.
    let camera = Camera::new(
        view.center() + vec2(0.45 * PULL * size.x, view.height() * 0.10),
        CAMERA_DISTANCE * size.y,
    );
    let placements = stack(sheets.len(), size);

    // Back cover and tab, under everything. Fixed size, whatever the count.
    let bottom = -0.55 * size.y;
    let top = 0.55 * size.y;
    let half = 0.6 * size.x;
    let cover = [
        vec2(-half, bottom),
        vec2(half, bottom),
        vec2(half, top),
        vec2(-half, top),
    ];
    let tab = [
        vec2(-half, top - 1.0),
        vec2(-0.1 * size.x, top - 1.0),
        vec2(-0.14 * size.x, top + 0.07 * size.y),
        vec2(-half + 0.04 * size.x, top + 0.07 * size.y),
    ];
    let shadow_offset = vec2(6.0, 10.0);
    let project_all = |points: [Vec2; 4]| points.map(|p| camera.project(p, 0.0)).to_vec();
    for outline in [project_all(tab), project_all(cover)] {
        let shadow = outline.iter().map(|&p| p + shadow_offset).collect();
        painter.add(Shape::convex_polygon(shadow, SHADOW, Stroke::NONE));
        painter.add(Shape::convex_polygon(
            outline,
            MANILA,
            Stroke::new(1.0, MANILA_EDGE),
        ));
    }
    let book = Book::beside(&camera, size, metrics, half, bottom);
    let book_hovered = ui
        .input(|i| i.pointer.hover_pos())
        .is_some_and(|p| contains(&book.quad, p));
    book.paint(&painter, book_hovered);
    let tab_quad = tab.map(|p| camera.project(p, 0.0));
    let tab_centre = camera.project(vec2(-0.35 * size.x, top + 0.035 * size.y), 0.0);
    let renaming_now = renaming.is_some();
    let tab_hovered = !renaming_now
        && ui
            .input(|i| i.pointer.hover_pos())
            .is_some_and(|p| contains(&tab_quad, p));
    let mut tab_action = None;
    if let Some(name) = renaming {
        tab_action = rename_field(ui, tab_centre, name);
    } else {
        painter.text(
            tab_centre,
            Align2::CENTER_CENTER,
            project.name,
            FontId::proportional(12.0),
            if tab_hovered { HIGHLIGHT } else { LABEL_DARK },
        );
    }

    let chosen = (!sheets.is_empty()).then(|| selected.min(sheets.len() - 1));
    let pull = |i: usize| {
        ui.ctx().animate_value_with_time(
            Id::new(("folder-pull", i)),
            if chosen == Some(i) { 1.0 } else { 0.0 },
            PULL_SECONDS,
        )
    };
    let pulls: Vec<f32> = (0..sheets.len()).map(pull).collect();
    let top_sheet = top_of_stack(&pulls);

    // One shadow for the stack: stacked sheets cast none that shows.
    if let Some(last) = placements.last() {
        let shadow = Placement { lift: 0.0, ..*last }
            .quad(&camera, size, Vec2::ZERO, size, 0.0)
            .map(|p| p + vec2(1.5, 2.5));
        painter.add(Shape::convex_polygon(shadow.to_vec(), SHADOW, Stroke::NONE));
    }

    let scale = size.x / metrics.paper_size.x;
    let line = metrics.cell_size().y;
    let mut pulled_quad = None;
    let mut chosen_label = None;
    for (i, (page, base)) in sheets.iter().zip(&placements).enumerate() {
        let amount = pulls[i];
        let placement = pulled(*base, size, amount);
        if amount > 0.0 {
            let shadow = placement
                .quad(&camera, size, Vec2::ZERO, size, 0.0)
                .map(|p| p + vec2(3.0, 5.0) * amount);
            painter.add(Shape::convex_polygon(shadow.to_vec(), SHADOW, Stroke::NONE));
        }
        let is_chosen = chosen == Some(i);
        let edge = if is_chosen {
            Stroke::new(1.5, HIGHLIGHT)
        } else {
            Stroke::new(1.0, SHEET_EDGE)
        };
        let outline = placement.quad(&camera, size, Vec2::ZERO, size, placement.lift);
        painter.add(Shape::convex_polygon(outline.to_vec(), SHEET, edge));
        if is_chosen {
            pulled_quad = Some(outline);
        }

        // Only the top sheet and sliding ones show words; the rest are
        // covered.
        if amount > 0.0 || top_sheet == Some(i) {
            let mut bars = Mesh::default();
            let ink = INK.gamma_multiply(0.35);
            for (half_line, first, last) in word_runs(page) {
                let top_left = metrics.cell_offset(half_line, first) + vec2(0.0, 0.3 * line);
                let bottom_right =
                    metrics.cell_offset(half_line, last + 1) + vec2(0.0, 0.75 * line);
                let quad = placement.quad(
                    &camera,
                    size,
                    top_left * scale,
                    bottom_right * scale,
                    placement.lift,
                );
                add_quad(&mut bars, quad, ink);
            }
            // Notes as faint pencil strokes, about line length.
            let area = NoteArea::new(metrics, margins, i);
            let pencil = note::GRAPHITE.gamma_multiply(0.45);
            for (row, line) in page.note().lines().enumerate() {
                let top = area.line_origin(Pos2::ZERO, row) + vec2(0.0, 0.35 * area.size);
                let length = (line.chars().count() as f32 * 0.42 * area.size).min(area.width);
                let quad = placement.quad(
                    &camera,
                    size,
                    top.to_vec2() * scale,
                    (top + vec2(length, 0.45 * area.size)).to_vec2() * scale,
                    placement.lift,
                );
                add_quad(&mut bars, quad, pencil);
            }
            painter.add(Shape::mesh(bars));
            let label = camera.project(
                placement.on_plane(size, vec2(0.94 * size.x, 0.025 * size.y)),
                placement.lift,
            );
            if is_chosen {
                chosen_label = Some(label);
            }
            if !(is_chosen && renumbering.is_some()) {
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

    // Clicks only open the pulled-out sheet; choosing is keys only.
    let response = ui.interact(view, Id::new("folder"), super::CLICK);
    let hovered = match (response.hover_pos(), pulled_quad, chosen) {
        (Some(pointer), Some(quad), Some(i)) if contains(&quad, pointer) => Some(i),
        _ => None,
    };

    let mut action = None;
    if let (Some(number), Some(at)) = (renumbering, chosen_label) {
        action = number_field(ui, at, number, sheets.len());
    }

    let hint = if sheets.is_empty() {
        "No finished sheets yet. Insert feeds a new one.".to_owned()
    } else {
        format!(
            "{} finished sheet{}  ·  arrow keys: choose  ·  Shift + arrow keys: move it  ·  Enter or click: read it  ·  Esc: back to the typewriter",
            sheets.len(),
            if sheets.len() == 1 { "" } else { "s" }
        )
    };
    painter.text(
        pos2(view.center().x, view.top() + 24.0),
        Align2::CENTER_CENTER,
        hint,
        FontId::proportional(13.0),
        LABEL,
    );

    painter.text(
        pos2(view.center().x, view.top() + 46.0),
        Align2::CENTER_CENTER,
        project.location,
        FontId::monospace(11.0),
        LABEL.gamma_multiply(0.7),
    );
    painter.text(
        pos2(view.center().x, view.top() + 66.0),
        Align2::CENTER_CENTER,
        project.stats,
        FontId::proportional(12.0),
        LABEL.gamma_multiply(0.85),
    );

    action = action
        .or(tab_action)
        .or_else(|| menus(ui, view, project.saved, !sheets.is_empty()));
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
    let book_response = ui
        .interact(
            Rect::from_points(&book.quad),
            Id::new("scratchpad-book"),
            super::CLICK,
        )
        .on_hover_text("Scratchpad (1)");
    if book_hovered {
        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
    }
    FolderResponse {
        opened,
        action,
        pulled: pulled_quad,
        open_scratchpad: book_hovered && book_response.clicked(),
    }
}

/// The chosen sheet's new number, over its old one. Enter moves it; Esc or
/// clicking away leaves it.
fn number_field(ui: &mut Ui, at: Pos2, number: &mut String, count: usize) -> Option<FolderAction> {
    let edit = egui::TextEdit::singleline(number)
        .id(Id::new("renumber-field"))
        .char_limit(count.to_string().len())
        .hint_text(format!("1\u{2013}{count}"))
        .font(FontId::proportional(11.0));
    let entered = inline_field(ui, Rect::from_center_size(at, vec2(44.0, 18.0)), edit);
    number.retain(|c| c.is_ascii_digit());
    Some(match entered? {
        true => FolderAction::RenumberTo(number.clone()),
        false => FolderAction::CancelRenumber,
    })
}

/// The new name, on the tab. Enter renames; Esc or clicking away cancels.
fn rename_field(ui: &mut Ui, at: Pos2, name: &mut String) -> Option<FolderAction> {
    let edit = egui::TextEdit::singleline(name).font(FontId::proportional(12.0));
    let entered = inline_field(ui, Rect::from_center_size(at, vec2(180.0, 22.0)), edit);
    Some(match entered? {
        true => FolderAction::RenameTo(name.trim().to_owned()),
        false => FolderAction::CancelRename,
    })
}

/// A focused, centred one-line field. `Some(entered)` once it loses focus:
/// true by Enter, false by Esc or a click away.
fn inline_field(ui: &mut Ui, rect: Rect, edit: egui::TextEdit<'_>) -> Option<bool> {
    let field = ui.put(rect, edit.horizontal_align(egui::Align::Center));
    if !field.has_focus() && !field.lost_focus() {
        field.request_focus();
    }
    field
        .lost_focus()
        .then(|| ui.input(|i| i.key_pressed(egui::Key::Enter)))
}

/// Four plates below the folder, each opening a menu.
fn menus(ui: &mut Ui, view: Rect, saved: bool, has_sheets: bool) -> Option<FolderAction> {
    let labels = [
        "Current project\u{2026}",
        "Other projects\u{2026}",
        "Sheet\u{2026}",
        "Export\u{2026}",
    ];
    let painter = ui.painter_at(view);
    let font = FontId::proportional(12.0);
    let gap = 12.0;
    let galleys: Vec<_> = labels
        .iter()
        .map(|label| painter.layout_no_wrap((*label).to_owned(), font.clone(), LABEL_DARK))
        .collect();
    let widths: Vec<f32> = galleys.iter().map(|g| g.size().x + 24.0).collect();
    let total = widths.iter().sum::<f32>() + gap * (labels.len() - 1) as f32;
    let mut x = view.center().x - total / 2.0;
    let y = view.bottom() - 44.0;
    let mut chosen = None;
    for (index, (galley, width)) in galleys.into_iter().zip(widths).enumerate() {
        let rect = Rect::from_min_size(pos2(x, y), vec2(width, 26.0));
        x += width + gap;
        let plate = ui.interact(rect, Id::new(("desk-menu", index)), super::CLICK);
        let open = egui::Popup::is_id_open(ui.ctx(), egui::Popup::default_response_id(&plate));
        let (fill, edge) = if plate.hovered() || open {
            (MANILA_HOVER, HIGHLIGHT)
        } else {
            (MANILA, MANILA_EDGE)
        };
        painter.rect(
            rect,
            CornerRadius::same(3),
            fill,
            Stroke::new(1.0, edge),
            egui::StrokeKind::Inside,
        );
        painter.galley(rect.center() - galley.size() / 2.0, galley, LABEL_DARK);
        if plate.hovered() {
            ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
        }
        egui::Popup::menu(&plate)
            .align(egui::RectAlign::TOP_START)
            .show(|ui| {
                let items: Vec<(&str, FolderAction, bool, &str)> = match index {
                    0 => vec![
                        ("Save", FolderAction::Save, true, "Saved automatically too."),
                        (
                            "Save As\u{2026}",
                            FolderAction::SaveAs,
                            true,
                            "Choose a name and place.",
                        ),
                        (
                            "Rename",
                            FolderAction::Rename,
                            saved,
                            "Save the project first.",
                        ),
                    ],
                    1 => vec![
                        ("New project", FolderAction::New, true, ""),
                        ("Open project\u{2026}", FolderAction::Open, true, ""),
                    ],
                    2 => vec![
                        (
                            "Roll back in",
                            FolderAction::RollIn,
                            has_sheets,
                            "No finished sheets yet.",
                        ),
                        (
                            "Renumber\u{2026}",
                            FolderAction::Renumber,
                            has_sheets,
                            "No finished sheets yet.",
                        ),
                        (
                            "Scrunch up\u{2026}",
                            FolderAction::Scrunch,
                            has_sheets,
                            "No finished sheets yet.",
                        ),
                    ],
                    _ => [
                        ("To Markdown", ExportFormat::Markdown),
                        ("To Text file", ExportFormat::Text),
                        ("To PDF", ExportFormat::Pdf),
                    ]
                    .into_iter()
                    .map(|(label, format)| {
                        (
                            label,
                            FolderAction::Export(format),
                            saved,
                            "Save the project first: exports go beside it.",
                        )
                    })
                    .collect(),
                };
                for (label, action, enabled, disabled_tip) in items {
                    let button = ui
                        .add_enabled(enabled, egui::Button::new(label))
                        .on_disabled_hover_text(disabled_tip);
                    if button.clicked() {
                        chosen = Some(action);
                    }
                }
            });
    }
    chosen
}

/// What happened to the open sheet's note.
#[derive(Debug, Default)]
pub struct SheetResponse {
    /// Clicked the top margin: start writing.
    pub start_note: bool,
    /// Finished writing: the note as written.
    pub note_written: Option<String>,
}

/// One sheet, scaled to fit, read-only except for its note. `note`: the
/// note being written, if any.
#[allow(clippy::too_many_arguments)]
pub fn show_sheet(
    ui: &mut Ui,
    view: Rect,
    profile: &Profile,
    carriage: &Carriage,
    page: &Page,
    index: usize,
    total: usize,
    max_points_per_inch: f32,
    ink_realism: bool,
    note: Option<&mut String>,
) -> SheetResponse {
    let painter = ui.painter_at(view);
    painter.rect_filled(view, CornerRadius::ZERO, DIM);
    let paper_inches = vec2(
        profile.paper.width_mm as f32,
        profile.paper.height_mm as f32,
    ) / super::MM_PER_INCH;
    let header = 44.0;
    let fit = ((view.width() - 40.0) / paper_inches.x)
        .min((view.height() - 2.0 * header) / paper_inches.y);
    let metrics = Metrics::new(profile, fit.min(max_points_per_inch).max(8.0));
    let origin = view.center() - metrics.paper_size / 2.0;
    let sheet = Rect::from_min_size(origin, metrics.paper_size);

    let shadow = egui::Shadow {
        offset: [0, 4],
        blur: 18,
        spread: 0,
        color: Color32::from_black_alpha(110),
    };
    painter.add(shadow.as_shape(sheet, CornerRadius::ZERO));
    painter.rect_filled(sheet, CornerRadius::ZERO, SHEET);
    paper::paint_margin_frame(
        &painter,
        &metrics,
        carriage,
        profile.margins.top_lines,
        origin,
    );
    paper::paint_sheet(
        &painter,
        &metrics,
        page,
        origin,
        ink_realism,
        Dimming::NONE,
        &paper::dry,
    );

    painter.text(
        pos2(view.center().x, sheet.top() - header / 2.0),
        Align2::CENTER_CENTER,
        format!("Sheet {} of {total}", index + 1),
        FontId::proportional(14.0),
        LABEL,
    );
    let hint = if note.is_some() {
        "Enter: new line  ·  click elsewhere or Esc: done"
    } else {
        "Click the top margin: pencil a note  ·  arrows or Page Up / Down: flip  ·  Esc: folder  ·  type: back to the typewriter"
    };
    painter.text(
        pos2(view.center().x, sheet.bottom() + header / 2.0),
        Align2::CENTER_CENTER,
        hint,
        FontId::proportional(12.0),
        LABEL,
    );

    let area = NoteArea::new(&metrics, &profile.margins, index);
    let mut response = SheetResponse::default();
    match note {
        Some(text) => response.note_written = note_field(ui, &area, origin, text),
        None => {
            note::paint_note(&painter, &area, origin, page.note(), 1.0);
            let margin = ui
                .interact(
                    area.margin_rect(origin, metrics.paper_size.x),
                    Id::new("note-margin"),
                    super::CLICK,
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

/// A small folder, bottom left, that opens the folder view.
pub fn desk_icon(ui: &mut Ui, view: Rect, count: usize, opacity: f32) -> bool {
    if opacity <= 0.0 {
        return false;
    }
    let body = Rect::from_min_size(view.left_bottom() + vec2(16.0, -46.0), vec2(48.0, 30.0));
    let hit = body.expand2(vec2(0.0, 6.0)).translate(vec2(0.0, -3.0));
    // React only when fully shown, not mid-fade.
    let response = (opacity >= 1.0).then(|| {
        ui.interact(hit, Id::new("folder-icon"), super::CLICK)
            .on_hover_text("Finished sheets (Page Up)")
    });
    let hovered = response.as_ref().is_some_and(|r| r.hovered());
    let (fill, edge) = if hovered {
        (MANILA_HOVER, HIGHLIGHT)
    } else {
        (MANILA, MANILA_EDGE)
    };
    let mut painter = ui.painter_at(view);
    painter.multiply_opacity(opacity);
    let tab = Rect::from_min_size(body.left_top() + vec2(3.0, -6.0), vec2(18.0, 8.0));
    painter.rect(
        tab,
        CornerRadius::same(2),
        fill,
        Stroke::new(1.0, edge),
        egui::StrokeKind::Inside,
    );
    painter.rect(
        body,
        CornerRadius::same(3),
        fill,
        Stroke::new(1.0, edge),
        egui::StrokeKind::Inside,
    );
    painter.text(
        body.center(),
        Align2::CENTER_CENTER,
        count,
        FontId::proportional(13.0),
        LABEL_DARK,
    );
    if hovered {
        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
    }
    response.is_some_and(|r| r.clicked())
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
    fn projection_shrinks_with_distance() {
        let camera = Camera::new(pos2(0.0, 0.0), 300.0);
        let near = camera.project(vec2(100.0, -50.0), 0.0);
        let far = camera.project(vec2(100.0, 50.0), 0.0);
        assert!(near.x > far.x);
        assert!(far.y < near.y);
        assert_eq!(camera.project(Vec2::ZERO, 0.0), pos2(0.0, 0.0));
    }

    #[test]
    fn quad_hit_test() {
        let quad = [
            pos2(0.0, 0.0),
            pos2(10.0, 0.0),
            pos2(12.0, 10.0),
            pos2(-2.0, 10.0),
        ];
        assert!(contains(&quad, pos2(5.0, 5.0)));
        assert!(!contains(&quad, pos2(11.0, 1.0)));
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
