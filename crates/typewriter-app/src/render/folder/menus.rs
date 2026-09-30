//! The plates below the folder and their menus, and the one-line fields that
//! rename the project and renumber a sheet.

use eframe::egui::{
    self, CornerRadius, CursorIcon, FontId, Id, Pos2, Rect, Stroke, Ui, pos2, vec2,
};

use super::{LABEL_DARK, manila};
use crate::filing::ExportFormat;
use crate::render::CLICK;

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
    /// Stand the chosen sheet on the copy holder.
    PutOnHolder,
    Print(Printout),
}

/// What Print prints.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Printout {
    /// Every sheet, the one in the machine included.
    Project,
    ChosenSheet,
}

/// A menu item: its label, what it asks, and why it is off (`None`: on).
type Item = (&'static str, FolderAction, Option<&'static str>);

/// The plates' labels and menus, left to right.
fn plates(saved: bool, has_sheets: bool) -> [(&'static str, Vec<Item>); 4] {
    let off = |on: bool, why: &'static str| (!on).then_some(why);
    let no_sheets = off(has_sheets, "No finished sheets yet.");
    let unsaved = off(saved, "Save the project first: exports go beside it.");
    let export = |label, format| (label, FolderAction::Export(format), unsaved);
    [
        (
            "Current project\u{2026}",
            vec![
                ("Save", FolderAction::Save, None),
                ("Save As\u{2026}", FolderAction::SaveAs, None),
                (
                    "Rename",
                    FolderAction::Rename,
                    off(saved, "Save the project first."),
                ),
            ],
        ),
        (
            "Other projects\u{2026}",
            vec![
                ("New project", FolderAction::New, None),
                ("Open project\u{2026}", FolderAction::Open, None),
            ],
        ),
        (
            "Sheet\u{2026}",
            vec![
                ("Roll back in", FolderAction::RollIn, no_sheets),
                (
                    "Put on the copy holder",
                    FolderAction::PutOnHolder,
                    no_sheets,
                ),
                ("Renumber\u{2026}", FolderAction::Renumber, no_sheets),
                ("Scrunch up\u{2026}", FolderAction::Scrunch, no_sheets),
            ],
        ),
        (
            "Export\u{2026}",
            vec![
                export("To Markdown", ExportFormat::Markdown),
                export("To Text file", ExportFormat::Text),
                export("To PDF", ExportFormat::Pdf),
                (
                    "Print all sheets\u{2026}",
                    FolderAction::Print(Printout::Project),
                    None,
                ),
                (
                    "Print the chosen sheet\u{2026}",
                    FolderAction::Print(Printout::ChosenSheet),
                    no_sheets,
                ),
            ],
        ),
    ]
}

/// The plates below the folder, each opening its menu.
pub(super) fn menus(
    ui: &mut Ui,
    view: Rect,
    saved: bool,
    has_sheets: bool,
) -> Option<FolderAction> {
    let plates = plates(saved, has_sheets);
    let painter = ui.painter_at(view);
    let font = FontId::proportional(12.0);
    let gap = 12.0;
    let galleys: Vec<_> = plates
        .iter()
        .map(|(label, _)| painter.layout_no_wrap((*label).to_owned(), font.clone(), LABEL_DARK))
        .collect();
    let widths: Vec<f32> = galleys.iter().map(|g| g.size().x + 24.0).collect();
    let total = widths.iter().sum::<f32>() + gap * (plates.len() - 1) as f32;
    let mut x = view.center().x - total / 2.0;
    let y = view.bottom() - 44.0;
    let mut chosen = None;
    let plates = plates.into_iter().zip(galleys.into_iter().zip(widths));
    for (index, ((_, items), (galley, width))) in plates.enumerate() {
        let rect = Rect::from_min_size(pos2(x, y), vec2(width, 26.0));
        x += width + gap;
        let plate = ui.interact(rect, Id::new(("desk-menu", index)), CLICK);
        let open = egui::Popup::is_id_open(ui.ctx(), egui::Popup::default_response_id(&plate));
        let (fill, edge) = manila(plate.hovered() || open);
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
                for (label, action, off) in items {
                    let mut item = ui.add_enabled(off.is_none(), egui::Button::new(label));
                    if let Some(why) = off {
                        item = item.on_disabled_hover_text(why);
                    }
                    if item.clicked() {
                        chosen = Some(action);
                    }
                }
            });
    }
    chosen
}

/// The chosen sheet's new number, over its old one. Enter moves it; Esc or
/// clicking away leaves it.
pub(super) fn number_field(
    ui: &mut Ui,
    at: Pos2,
    number: &mut String,
    count: usize,
) -> Option<FolderAction> {
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
pub(super) fn rename_field(ui: &mut Ui, at: Pos2, name: &mut String) -> Option<FolderAction> {
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
