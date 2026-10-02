//! The notebook closed: a kraft-covered pocket memo book, an icon on the
//! desk and lying beside the folder. Open, it is [`super::pad`].

use emath::Rot2;

use eframe::egui::text::{LayoutJob, TextFormat};
use eframe::egui::{
    Align, Color32, FontFamily, FontId, Painter, Pos2, Rect, Shape, Stroke, Ui, Vec2, pos2, vec2,
};

use super::{DeskIcon, HIGHLIGHT};

pub const COVER_FAMILY: &str = "cover";
pub const JOST: &[u8] = include_bytes!("../../../../assets/fonts/jost/Jost-Bold.ttf");
/// A pocket memo book, 88.9 × 139.7 mm (3.5 × 5.5 in).
pub const BOOK_MM: Vec2 = vec2(88.9, 139.7);

/// Right-hand corners; the spine is square.
pub(super) const CORNER_WIDTHS: f32 = 0.1;
/// The desk icon, beside the folder icon.
const ICON: Rect = Rect::from_min_max(pos2(76.0, -50.0), pos2(98.0, -16.0));

pub(super) const KRAFT: Color32 = Color32::from_rgb(0xC6, 0xA2, 0x6E);
pub(super) const KRAFT_EDGE: Color32 = Color32::from_rgb(0x8E, 0x70, 0x4A);
const COVER_INK: Color32 = Color32::from_rgb(0x22, 0x1E, 0x1A);
pub(super) const STAPLE: Color32 = Color32::from_rgb(0x3C, 0x3A, 0x36);

const STICKER_SHADOW: Color32 = Color32::from_rgba_premultiplied(0, 0, 0, 0x30);
const TRANS_BLUE: Color32 = Color32::from_rgb(0x5B, 0xCE, 0xFA);
const TRANS_PINK: Color32 = Color32::from_rgb(0xF5, 0xA9, 0xB8);
const TRANS_FLAG: Flag = Flag::Stripes(&[
    TRANS_BLUE,
    TRANS_PINK,
    Color32::WHITE,
    TRANS_PINK,
    TRANS_BLUE,
]);
const ANARCHIST_FLAG: Flag = Flag::Diagonal {
    upper: Color32::from_rgb(0xD0, 0x1C, 0x1F),
    lower: Color32::from_rgb(0x14, 0x14, 0x14),
};

/// A flag's field.
enum Flag {
    /// Top to bottom.
    Stripes(&'static [Color32]),
    /// Split from the lower hoist corner to the upper fly corner.
    Diagonal { upper: Color32, lower: Color32 },
}

const TITLE: &str = "Ich kann nicht glauben, dass es kein Fieldnotiz ist!";
const DETAILS: &str =
    "48-Seiten-Memobuch\nStrapazierfähiges Material\nHergestellt mit Hilfe von Estradiol";

/// The closed book on the desk, beside the folder icon. True when clicked.
pub fn desk_icon(ui: &mut Ui, view: Rect, opacity: f32) -> bool {
    let rect = ICON.translate(view.left_bottom().to_vec2());
    let icon = DeskIcon {
        hit: rect,
        id: "notebook-icon",
        tip: "Notebook (1)",
    };
    icon.show(ui, view, opacity, |painter, hovered| {
        for mut shape in cover(painter, rect.size(), hovered) {
            shape.translate(rect.min.to_vec2());
            painter.add(shape);
        }
    })
}

/// The closed cover, `size` points, from (0, 0) at its top-left: square
/// stapled spine, rounded corners, the title centred and spaced out, the
/// details under it, two flag stickers below.
pub fn cover(painter: &Painter, size: Vec2, hovered: bool) -> Vec<Shape> {
    let w = size.x;
    let edge = if hovered { HIGHLIGHT } else { KRAFT_EDGE };
    let mut shapes = vec![Shape::convex_polygon(
        outline(size),
        KRAFT,
        Stroke::new((0.012 * w).min(1.0), edge),
    )];
    for (from, to) in [(0.09, 0.15), (0.47, 0.53), (0.85, 0.91)] {
        let staple = Rect::from_min_max(pos2(0.0, from * size.y), pos2(0.014 * w, to * size.y));
        shapes.push(Shape::rect_filled(staple, 0.0, STAPLE));
    }
    let centred = |text: &str, em: f32, spacing_em: f32| {
        let format = TextFormat {
            font_id: FontId::new(em * w, FontFamily::Name(COVER_FAMILY.into())),
            color: COVER_INK,
            extra_letter_spacing: spacing_em * em * w,
            ..Default::default()
        };
        let mut job = LayoutJob::single_section(text.to_owned(), format);
        job.wrap.max_width = 0.84 * w;
        job.halign = Align::Center;
        painter.layout_job(job)
    };
    let title = centred(&TITLE.to_uppercase(), 0.075, 0.12);
    let top = 0.2 * size.y;
    let details_top = top + title.size().y + 0.1 * w;
    shapes.push(Shape::galley(pos2(w / 2.0, top), title, COVER_INK));
    let details = centred(DETAILS, 0.042, 0.0);
    shapes.push(Shape::galley(
        pos2(w / 2.0, details_top),
        details,
        COVER_INK,
    ));
    let h = size.y;
    shapes.extend(sticker(pos2(0.4 * w, 0.78 * h), 0.3 * w, -7.0, TRANS_FLAG));
    shapes.extend(sticker(
        pos2(0.66 * w, 0.87 * h),
        0.28 * w,
        6.0,
        ANARCHIST_FLAG,
    ));
    shapes
}

/// A flag sticker, 5:3, with a white die-cut border and a hair of shadow.
/// `degrees`: clockwise.
fn sticker(centre: Pos2, width: f32, degrees: f32, flag: Flag) -> Vec<Shape> {
    let size = vec2(width, 0.6 * width);
    let border = 0.05 * size.y;
    let angle = degrees.to_radians();
    let turned = |points: &[Vec2]| {
        points
            .iter()
            .map(|&p| centre + Rot2::from_angle(angle) * p)
            .collect::<Vec<_>>()
    };
    let quad = |min: Vec2, max: Vec2| turned(&[min, vec2(max.x, min.y), max, vec2(min.x, max.y)]);
    let half = size / 2.0;
    let edge = half + Vec2::splat(border);
    let shadow = vec2(0.3, 0.5) * border;
    let mut shapes = vec![
        Shape::convex_polygon(
            quad(shadow - edge, shadow + edge),
            STICKER_SHADOW,
            Stroke::NONE,
        ),
        Shape::convex_polygon(quad(-edge, edge), Color32::WHITE, Stroke::NONE),
    ];
    match flag {
        Flag::Stripes(stripes) => {
            let stripe = size.y / stripes.len() as f32;
            for (i, &color) in stripes.iter().enumerate() {
                let top = -half.y + i as f32 * stripe;
                shapes.push(Shape::convex_polygon(
                    quad(vec2(-half.x, top), vec2(half.x, top + stripe)),
                    color,
                    Stroke::NONE,
                ));
            }
        }
        Flag::Diagonal { upper, lower } => {
            let (top_left, top_right) = (-half, vec2(half.x, -half.y));
            let (bottom_left, bottom_right) = (vec2(-half.x, half.y), half);
            let upper_half = turned(&[top_left, top_right, bottom_left]);
            let lower_half = turned(&[top_right, bottom_right, bottom_left]);
            shapes.push(Shape::convex_polygon(upper_half, upper, Stroke::NONE));
            shapes.push(Shape::convex_polygon(lower_half, lower, Stroke::NONE));
        }
    }
    shapes
}

/// The cover's outline, as [`cover`] draws it.
pub fn outline(size: Vec2) -> Vec<Pos2> {
    const STEPS: usize = 5;
    let radius = CORNER_WIDTHS * size.x;
    let arc = |centre: Pos2, start_degrees: f32| {
        (0..=STEPS).map(move |i| {
            let a = (start_degrees + 90.0 * i as f32 / STEPS as f32).to_radians();
            centre + radius * vec2(a.cos(), a.sin())
        })
    };
    let mut points = vec![Pos2::ZERO];
    points.extend(arc(pos2(size.x - radius, radius), -90.0));
    points.extend(arc(pos2(size.x - radius, size.y - radius), 0.0));
    points.push(pos2(0.0, size.y));
    points
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_outline_stays_within_its_size() {
        let size = vec2(60.0, 100.0);
        let points = outline(size);
        assert!(points.iter().all(|p| {
            (-1e-3..=size.x + 1e-3).contains(&p.x) && (-1e-3..=size.y + 1e-3).contains(&p.y)
        }));
    }
}
