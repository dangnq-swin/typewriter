//! The folder of finished sheets, drawn as a tray on a tilted desk, and the
//! read-only view of one sheet taken out of it.
//!
//! egui cannot draw text in perspective, so sheets in the folder show their
//! words as faint ink bars; a sheet becomes readable once it is picked up.

use eframe::egui::{
    self, Align2, Color32, CornerRadius, CursorIcon, FontId, Id, Mesh, Painter, Pos2, Rect, Sense,
    Shape, Stroke, Ui, Vec2, pos2, vec2,
};
use typewriter_core::Profile;
use typewriter_core::carriage::Carriage;
use typewriter_core::page::Page;

use super::Metrics;
use super::paper::{self, INK, splitmix64};

const TILT_DEGREES: f32 = 38.0;
/// Camera distance in sheet heights. Smaller means stronger perspective.
const CAMERA_DISTANCE: f32 = 2.6;
/// The whole fan takes at most this share of a sheet's height, and each
/// sheet peeks out at most this much behind the next.
const MAX_FAN: f32 = 0.45;
const MAX_STEP: f32 = 0.09;
/// Hovering slides a sheet back out of the stack by this share of its height.
const LIFT_SLIDE: f32 = 0.08;
const LIFT_HEIGHT: f32 = 0.02;
const SHEET_THICKNESS: f32 = 0.4;

const DIM: Color32 = Color32::from_rgba_premultiplied(0x1A, 0x17, 0x14, 0xB4);
const MANILA: Color32 = Color32::from_rgb(0xDD, 0xBF, 0x86);
const MANILA_EDGE: Color32 = Color32::from_rgb(0xA3, 0x85, 0x52);
const SHEET: Color32 = Color32::from_rgb(0xF7, 0xF4, 0xEC);
const SHEET_EDGE: Color32 = Color32::from_rgb(0xC9, 0xC2, 0xB4);
const HIGHLIGHT: Color32 = Color32::from_rgb(0x80, 0x30, 0x20);
const SHADOW: Color32 = Color32::from_rgba_premultiplied(0, 0, 0, 0x30);
const LABEL: Color32 = Color32::from_rgb(0xEE, 0xE8, 0xDC);
const LABEL_DARK: Color32 = Color32::from_rgb(0x5A, 0x48, 0x2A);

/// Perspective projection of the desk plane. Plane coordinates have `y`
/// pointing away from the viewer; `lift` is height above the plane.
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
    /// A point on the sheet, in sheet units with `y` down the page, on the
    /// desk plane.
    fn on_plane(&self, size: Vec2, local: Vec2) -> Vec2 {
        let d = vec2(local.x - size.x / 2.0, size.y / 2.0 - local.y);
        let (sin, cos) = self.angle.sin_cos();
        self.centre + vec2(d.x * cos - d.y * sin, d.x * sin + d.y * cos)
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

/// Sheets fanned in the folder, oldest first. The newest lies in front and
/// each older one peeks out a little further back, slightly askew.
fn fan(count: usize, size: Vec2) -> (Vec<Placement>, f32) {
    let step = if count > 1 {
        (MAX_FAN * size.y / (count - 1) as f32).min(MAX_STEP * size.y)
    } else {
        0.0
    };
    let extent = step * count.saturating_sub(1) as f32;
    let placements = (0..count)
        .map(|i| {
            let bits = splitmix64(i as u64);
            let unit = |shift: u32| ((bits >> shift) & 0xFFFF) as f32 / 65535.0 - 0.5;
            Placement {
                centre: vec2(unit(0) * 0.03 * size.x, extent / 2.0 - i as f32 * step),
                angle: unit(16) * 2.0_f32.to_radians(),
                lift: i as f32 * SHEET_THICKNESS,
            }
        })
        .collect();
    (placements, extent)
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

/// Draws the open folder and returns the sheet clicked, if any. `selected`
/// is the sheet chosen with the keyboard; moving the pointer onto a sheet
/// chooses it too.
pub fn show_folder(
    ui: &mut Ui,
    view: Rect,
    sheets: &[Page],
    metrics: &Metrics,
    selected: &mut usize,
) -> Option<usize> {
    let painter = ui.painter_at(view);
    painter.rect_filled(view, CornerRadius::ZERO, DIM);

    let aspect = metrics.paper_size.y / metrics.paper_size.x;
    let width = (view.width() * 0.42).min(view.height() * 0.40 / aspect);
    let size = vec2(width, width * aspect);
    let camera = Camera::new(
        view.center() + vec2(0.0, view.height() * 0.10),
        CAMERA_DISTANCE * size.y,
    );
    let (placements, extent) = fan(sheets.len(), size);

    // The folder's back cover with its tab, under everything.
    let bottom = -size.y / 2.0 - extent / 2.0 - 0.05 * size.y;
    let top = size.y / 2.0 + extent / 2.0 + 0.05 * size.y;
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
    let tab_centre = camera.project(vec2(-0.35 * size.x, top + 0.035 * size.y), 0.0);
    painter.text(
        tab_centre,
        Align2::CENTER_CENTER,
        "Finished sheets",
        FontId::proportional(12.0),
        LABEL_DARK,
    );

    let base_quads: Vec<[Pos2; 4]> = placements
        .iter()
        .map(|p| p.quad(&camera, size, Vec2::ZERO, size, p.lift))
        .collect();
    let response = ui.interact(view, Id::new("folder"), Sense::click());
    // Hit-test the resting positions, newest first, so a sheet sliding out
    // under the pointer does not flicker between hovered and not.
    let hovered = response.hover_pos().and_then(|pointer| {
        (0..sheets.len())
            .rev()
            .find(|&i| contains(&base_quads[i], pointer))
    });
    // Only a moving pointer takes over, so a resting one does not undo the
    // arrow keys.
    if let Some(i) = hovered
        && ui.input(|input| input.pointer.delta() != Vec2::ZERO)
    {
        *selected = i;
    }
    let chosen = (!sheets.is_empty()).then(|| (*selected).min(sheets.len() - 1));

    let step = if sheets.len() > 1 {
        extent / (sheets.len() - 1) as f32
    } else {
        size.y
    };
    let scale = size.x / metrics.paper_size.x;
    let line = metrics.cell_size().y;
    for (i, (page, base)) in sheets.iter().zip(&placements).enumerate() {
        let lifted = ui.ctx().animate_value_with_time(
            Id::new(("folder-lift", i)),
            if chosen == Some(i) { 1.0 } else { 0.0 },
            0.15,
        );
        let placement = Placement {
            centre: base.centre + vec2(0.0, lifted * LIFT_SLIDE * size.y),
            lift: base.lift + lifted * LIFT_HEIGHT * size.y,
            ..*base
        };
        let shadow = placement
            .quad(&camera, size, Vec2::ZERO, size, base.lift)
            .map(|p| p + vec2(1.5, 2.5) * (1.0 + 3.0 * lifted));
        painter.add(Shape::convex_polygon(shadow.to_vec(), SHADOW, Stroke::NONE));
        let edge = if chosen == Some(i) {
            Stroke::new(1.5, HIGHLIGHT)
        } else {
            Stroke::new(1.0, SHEET_EDGE)
        };
        let outline = placement.quad(&camera, size, Vec2::ZERO, size, placement.lift);
        painter.add(Shape::convex_polygon(outline.to_vec(), SHEET, edge));

        let mut bars = Mesh::default();
        let ink = INK.gamma_multiply(0.35);
        for (half_line, first, last) in word_runs(page) {
            let top_left = metrics.cell_offset(half_line, first) + vec2(0.0, 0.3 * line);
            let bottom_right = metrics.cell_offset(half_line, last + 1) + vec2(0.0, 0.75 * line);
            let quad = placement.quad(
                &camera,
                size,
                top_left * scale,
                bottom_right * scale,
                placement.lift,
            );
            add_quad(&mut bars, quad, ink);
        }
        painter.add(Shape::mesh(bars));

        if step >= 0.05 * size.y || chosen == Some(i) {
            let label = camera.project(
                placement.on_plane(size, vec2(0.94 * size.x, 0.025 * size.y)),
                placement.lift,
            );
            painter.text(
                label,
                Align2::CENTER_CENTER,
                i + 1,
                FontId::proportional(10.0),
                LABEL_DARK,
            );
        }
    }

    let hint = if sheets.is_empty() {
        "No finished sheets yet. Insert feeds a new one.".to_owned()
    } else {
        format!(
            "{} finished sheet{}  ·  arrow keys: choose  ·  Enter or click: read it  ·  Esc: back to the typewriter",
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

    if hovered.is_some() {
        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
    }
    if response.clicked() { hovered } else { None }
}

/// One sheet taken out of the folder, read-only and scaled to fit.
#[allow(clippy::too_many_arguments)]
pub fn show_sheet(
    painter: &Painter,
    view: Rect,
    profile: &Profile,
    carriage: &Carriage,
    page: &Page,
    number: usize,
    total: usize,
    max_points_per_inch: f32,
    ink_realism: bool,
) {
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
        painter,
        &metrics,
        carriage,
        profile.margins.top_lines,
        origin,
    );
    paper::paint_sheet(painter, &metrics, page, origin, ink_realism);

    painter.text(
        pos2(view.center().x, sheet.top() - header / 2.0),
        Align2::CENTER_CENTER,
        format!("Sheet {number} of {total}"),
        FontId::proportional(14.0),
        LABEL,
    );
    painter.text(
        pos2(view.center().x, sheet.bottom() + header / 2.0),
        Align2::CENTER_CENTER,
        "Arrow keys or Page Up / Page Down: flip  ·  Esc: back to the folder  ·  type to return to the typewriter",
        FontId::proportional(12.0),
        LABEL,
    );
}

/// A small folder on the desk, bottom left, that opens the folder view.
pub fn desk_icon(ui: &mut Ui, view: Rect, count: usize) -> bool {
    let body = Rect::from_min_size(view.left_bottom() + vec2(16.0, -46.0), vec2(48.0, 30.0));
    let hit = body.expand2(vec2(0.0, 6.0)).translate(vec2(0.0, -3.0));
    let response = ui
        .interact(hit, Id::new("folder-icon"), Sense::click())
        .on_hover_text("Finished sheets (Page Up)");
    let (fill, edge) = if response.hovered() {
        (Color32::from_rgb(0xE8, 0xCD, 0x98), HIGHLIGHT)
    } else {
        (MANILA, MANILA_EDGE)
    };
    let painter = ui.painter_at(view);
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
    if response.hovered() {
        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
    }
    response.clicked()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fan_puts_the_newest_in_front_and_older_further_back() {
        let size = vec2(100.0, 141.0);
        let (placements, extent) = fan(4, size);
        assert!((extent - 3.0 * MAX_STEP * size.y).abs() < 1e-3);
        for pair in placements.windows(2) {
            assert!(pair[0].centre.y > pair[1].centre.y);
            assert!(pair[0].lift < pair[1].lift);
        }
    }

    #[test]
    fn a_thick_folder_is_squeezed_into_the_fan() {
        let size = vec2(100.0, 141.0);
        let (_, extent) = fan(200, size);
        assert!((extent - MAX_FAN * size.y).abs() < 1e-2);
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
