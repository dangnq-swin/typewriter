//! The sheet and everything struck or painted on it.

use eframe::egui::{Color32, CornerRadius, Painter, Pos2, Rect, Shape, Stroke, Vec2, pos2, vec2};
use typewriter_core::accents;
use typewriter_core::carriage::Carriage;
use typewriter_core::page::{Correction, Mark, Page};

use super::calm::Dimming;
use super::{MM_PER_INCH, Metrics, splitmix64, unit};

pub const INK: Color32 = Color32::from_rgba_premultiplied(0x1C, 0x1A, 0x18, 0xEB);
/// Slip chalk: a slightly cool, matt white.
const CHALK: Color32 = Color32::from_rgb(0xF6, 0xF5, 0xF0);
/// Chalk spread past the strokes, points at 96 ppi.
const CHALK_SPREAD: f32 = 0.6;
/// Fluid, dry (a touch whiter than paper) and wet.
const FLUID: Color32 = Color32::from_rgb(0xFA, 0xF9, 0xF4);
const FLUID_WET: Color32 = Color32::from_rgb(0xFE, 0xFE, 0xFD);
const FLUID_RIM: Color32 = Color32::from_rgba_premultiplied(0x40, 0x3C, 0x34, 0x22);
/// Ink left after an eraser.
const ERASER_GHOST: f32 = 0.12;
const SCUFF: Color32 = Color32::from_rgba_premultiplied(0x40, 0x40, 0x3E, 0x40);
const SCUFF_STREAK: Color32 = Color32::from_rgba_premultiplied(0x10, 0x0F, 0x0E, 0x18);
/// How far smudged ink runs, points at 96 ppi.
const SMUDGE_SPREAD: f32 = 1.2;
const FRAME: Color32 = Color32::from_rgba_premultiplied(0x4A, 0x46, 0x40, 0x8C);
const FRAME_EXTENSION: Color32 = Color32::from_rgba_premultiplied(0x25, 0x23, 0x20, 0x46);
/// Frame clearance outside the margins: type must never touch it.
pub const FRAME_PADDING_MM: f32 = 1.0;
/// Ink realism: max strike offset (points at 96 ppi) and max lightening.
const INK_MAX_OFFSET: f32 = 0.4;
const INK_DENSITY_VARIANCE: f32 = 0.1;
/// Worn shift: capitals print this far above the line (points at 96 ppi,
/// about 0.3 pt), always the same way.
const SHIFT_MISALIGNMENT: f32 = 0.4;

/// The sheet has no fill: a frame round the writing area, sides extended to
/// the paper's edges, shows it. No bottom margin: the frame runs to the
/// sheet's bottom.
pub fn paint_margin_frame(
    painter: &Painter,
    metrics: &Metrics,
    carriage: &Carriage,
    top_lines: u16,
    origin: Pos2,
) {
    painter.extend(margin_frame(metrics, carriage, top_lines, origin));
}

/// The frame of [`paint_margin_frame`], as shapes.
pub fn margin_frame(
    metrics: &Metrics,
    carriage: &Carriage,
    top_lines: u16,
    origin: Pos2,
) -> Vec<Shape> {
    let pad = FRAME_PADDING_MM / MM_PER_INCH * metrics.points_per_inch;
    let sheet = Rect::from_min_size(origin, metrics.paper_size);
    let top_left =
        origin + metrics.cell_offset(top_lines * 2, carriage.left_margin) - Vec2::splat(pad);
    let right = origin.x + metrics.cell_offset(0, carriage.right_margin).x + pad;
    let frame = Rect::from_min_max(top_left, pos2(right, sheet.bottom()));

    let extension = Stroke::new(1.0, FRAME_EXTENSION);
    let mut shapes: Vec<Shape> = [frame.top(), frame.bottom()]
        .map(|y| Shape::hline(sheet.x_range(), y, extension))
        .into_iter()
        .chain([frame.left(), frame.right()].map(|x| Shape::vline(x, sheet.y_range(), extension)))
        .collect();
    shapes.push(Shape::rect_stroke(
        frame,
        CornerRadius::ZERO,
        Stroke::new(1.0, FRAME),
        eframe::egui::StrokeKind::Middle,
    ));
    shapes
}

/// Fluid wetness per cell: 1 just dabbed, 0 dry.
pub type Wetness<'a> = &'a dyn Fn(u16, u16) -> f32;

/// A sheet with no fluid drying on it.
pub fn dry(_: u16, _: u16) -> f32 {
    0.0
}

/// One drawing op, for the window or a PDF alike. Sheet points, `y` down.
#[derive(Debug, Clone, PartialEq)]
pub enum Drawn {
    /// `at` is the glyph's top-left.
    Glyph {
        at: Pos2,
        c: char,
        color: Color32,
    },
    Patch {
        rect: Rect,
        color: Color32,
    },
    Line {
        from: Pos2,
        to: Pos2,
        width: f32,
        color: Color32,
    },
    /// `rim`: a thin line just inside the edge.
    Ellipse {
        centre: Pos2,
        radius: Vec2,
        fill: Color32,
        rim: Option<Color32>,
    },
}

/// Fluid rim width, points.
pub const RIM_WIDTH: f32 = 0.8;

/// Draws a sheet in the window. See [`sheet_marks`].
pub fn paint_sheet(
    painter: &Painter,
    metrics: &Metrics,
    page: &Page,
    origin: Pos2,
    ink_realism: bool,
    dimming: Dimming,
    wetness: Wetness<'_>,
) {
    let clip = painter.clip_rect();
    let marks = sheet_marks(
        metrics,
        page,
        origin,
        ink_realism,
        dimming,
        wetness,
        |cell| clip.intersects(cell),
    );
    painter.extend(shapes(painter, metrics, marks));
}

/// `marks` as shapes, `painter` laying out their glyphs.
pub fn shapes(painter: &Painter, metrics: &Metrics, marks: Vec<Drawn>) -> Vec<Shape> {
    let mut shapes = Vec::new();
    for drawn in marks {
        match drawn {
            Drawn::Glyph { at, c, color } => {
                let galley = painter.layout_no_wrap(c.to_string(), metrics.font.clone(), color);
                shapes.push(Shape::galley(at, galley, color));
            }
            Drawn::Patch { rect, color } => {
                shapes.push(Shape::rect_filled(rect, CornerRadius::same(3), color));
            }
            Drawn::Line {
                from,
                to,
                width,
                color,
            } => {
                shapes.push(Shape::line_segment([from, to], Stroke::new(width, color)));
            }
            Drawn::Ellipse {
                centre,
                radius,
                fill,
                rim,
            } => {
                shapes.push(Shape::ellipse_filled(centre, radius, fill));
                if let Some(rim) = rim {
                    shapes.push(Shape::ellipse_stroke(
                        centre,
                        radius - Vec2::splat(RIM_WIDTH / 2.0),
                        Stroke::new(RIM_WIDTH, rim),
                    ));
                }
            }
        }
    }
    shapes
}

/// Everything a print depends on beyond the sheet itself.
#[derive(Clone, Copy)]
pub struct SheetLook<'a> {
    pub metrics: &'a Metrics,
    pub origin: Pos2,
    pub ink_realism: bool,
    pub dimming: Dimming,
    pub wetness: Wetness<'a>,
    /// Fluid on the sheet is drying: its shine moves every frame, so the
    /// print animates and cannot wait for the stamp.
    pub drying: bool,
}

/// The print a sheet makes on screen, kept from one frame to the next.
///
/// [`sheet_marks`] walks every cell and [`shapes`] lays out every glyph:
/// steady typing hands the same answer back every frame. The core says when
/// a sheet took a mark ([`Page::stamp`]); a moving look — zoom, feeding,
/// calm, a drying dab — is answered by [`SheetLook`].
#[derive(Default)]
pub struct SheetPrint {
    /// The key the shapes were made under, and them. `None` until first paint.
    painted: Option<(Painted, Vec<Shape>)>,
}

/// What one cached print stands for. Equal by value: a frame that changes
/// nothing recomputes nothing. Every float here comes back bit-identical
/// from the same inputs, or the look moved and the print must be remade.
#[derive(PartialEq)]
struct Painted {
    sheet: (u64, u64),
    points_per_inch: f32,
    origin: Pos2,
    ink_realism: bool,
    dimming: Dimming,
    clip: Rect,
}

impl SheetPrint {
    /// The sheet's marks as shapes, clipped to `clip`, remade only when the
    /// sheet took a mark or the look moved. Borrowed for the frame's draw.
    pub fn shapes(
        &mut self,
        painter: &Painter,
        look: &SheetLook<'_>,
        page: &Page,
        clip: Rect,
    ) -> &[Shape] {
        let SheetLook {
            metrics,
            origin,
            ink_realism,
            dimming,
            wetness,
            drying,
        } = *look;
        let key = Painted {
            sheet: page.stamp(),
            points_per_inch: metrics.points_per_inch,
            origin,
            ink_realism,
            dimming,
            clip,
        };
        if drying
            || self
                .painted
                .as_ref()
                .is_none_or(|(cached, _)| *cached != key)
        {
            let marks = sheet_marks(
                metrics,
                page,
                origin,
                ink_realism,
                dimming,
                wetness,
                |cell| clip.intersects(cell),
            );
            self.painted = Some((key, shapes(painter, metrics, marks)));
        }
        match &self.painted {
            Some((_, shapes)) => shapes,
            None => &[],
        }
    }
}

/// [`paint_sheet`], printing through `cache`.
pub fn paint_sheet_cached(
    painter: &Painter,
    cache: &mut SheetPrint,
    look: &SheetLook<'_>,
    page: &Page,
) {
    let clip = painter.clip_rect();
    painter.extend(cache.shapes(painter, look, page, clip).iter().cloned());
}

/// Every mark on a sheet at `origin`, in the order made, so corrections
/// cover what came before. Skips cells where `visible` is false.
///
/// `ink_realism` varies each strike (uneven pressure, slugs off true).
/// `dimming` fades ink only: a faded correction would show what it covers.
pub fn sheet_marks(
    metrics: &Metrics,
    page: &Page,
    origin: Pos2,
    ink_realism: bool,
    dimming: Dimming,
    wetness: Wetness<'_>,
    visible: impl Fn(Rect) -> bool,
) -> Vec<Drawn> {
    let scale = metrics.points_per_inch / 96.0;
    let mut out = Vec::new();
    for ((half_line, column), cell) in page.cells() {
        let cell_rect = Rect::from_min_size(
            origin + metrics.cell_offset(half_line, column),
            metrics.cell_size(),
        );
        // Pad the test: fluid spills past its cell.
        if !visible(cell_rect.expand(metrics.column_width)) {
            continue;
        }
        let ink = INK.gamma_multiply(dimming.opacity(half_line));
        let marks = cell.marks();
        // Only while the cell reads so: an erased accent stays erased.
        let composed = composed(marks).filter(|&(_, _, c)| cell.reads_as() == Some(c));
        for (index, mark) in marks.iter().enumerate() {
            let seed = mark_seed(half_line, column, index);
            // Struck on a re-fed sheet: out of line with its first feeding.
            let fed = page.shift(half_line, column, index);
            let cell_rect = cell_rect.translate(vec2(
                f32::from(fed.across) / 100.0 * metrics.column_width,
                f32::from(fed.down) / 100.0 * metrics.half_line_height,
            ));
            let (offset, density) = if ink_realism {
                let (offset, density) = ink_variation(half_line, column, index);
                (offset + shift_misalignment(mark), density)
            } else {
                (Vec2::ZERO, 1.0)
            };
            let at = cell_rect.min + offset * scale;
            match mark {
                Mark::Glyph(c) | Mark::Smudged(c) => {
                    let c = match composed {
                        // Drawn with its letter.
                        Some((accent, _, _)) if index == accent => continue,
                        Some((_, letter, accented)) if index == letter => accented,
                        _ => *c,
                    };
                    // Each eraser pass leaves a ghost.
                    let rubbed = marks[index + 1..]
                        .iter()
                        .filter(|m| matches!(m, Mark::Correction(Correction::Eraser)))
                        .count();
                    let color = ink.gamma_multiply(density * ERASER_GHOST.powi(rubbed as i32));
                    if matches!(mark, Mark::Smudged(_)) {
                        smudged(&mut out, at, c, color, scale, seed);
                    } else {
                        out.push(Drawn::Glyph { at, c, color });
                    }
                }
                Mark::Correction(Correction::Eraser) => scuff(&mut out, cell_rect, scale, seed),
                Mark::Correction(Correction::Chalk(c)) => chalk(&mut out, at, *c, scale),
                Mark::Correction(Correction::Fluid { .. }) => {
                    fluid(&mut out, cell_rect, wetness(half_line, column), seed);
                }
            }
        }
    }
    out
}

/// Roughed-up fibres catch the light: a pale patch, streaked along the rub.
fn scuff(out: &mut Vec<Drawn>, cell: Rect, scale: f32, seed: u64) {
    let patch = cell.expand2(vec2(1.5, -1.0) * scale);
    out.push(Drawn::Patch {
        rect: patch,
        color: SCUFF,
    });
    for i in 0..3_u32 {
        let unit = unit(seed, i * 16);
        let y = patch.top() + patch.height() * (0.2 + 0.6 * unit);
        let inset = patch.width() * 0.15 * unit;
        out.push(Drawn::Line {
            from: pos2(patch.left() + inset, y),
            to: pos2(patch.right() - inset, y),
            width: 0.8 * scale,
            color: SCUFF_STREAK,
        });
    }
}

/// Chalk in the struck character's shape, a little fuller than the type:
/// covers a slightly misaligned strike.
fn chalk(out: &mut Vec<Drawn>, at: Pos2, c: char, scale: f32) {
    let spread = CHALK_SPREAD * scale;
    for offset in [
        vec2(0.0, 0.0),
        vec2(spread, 0.0),
        vec2(-spread, 0.0),
        vec2(0.0, spread),
        vec2(0.0, -spread),
    ] {
        out.push(Drawn::Glyph {
            at: at + offset,
            c,
            color: CHALK,
        });
    }
}

/// Overlapping uneven dabs, slightly past the cell. Wet: brighter, with a
/// highlight.
fn fluid(out: &mut Vec<Drawn>, cell: Rect, wet: f32, seed: u64) {
    let unit = |shift: u32| ((seed >> shift) & 0xFF) as f32 / 255.0 - 0.5;
    let size = cell.size();
    let dabs = [0_u32, 16, 32].map(|shift| {
        let centre = cell.center() + vec2(unit(shift) * 0.3, unit(shift + 8) * 0.2) * size;
        let radius = vec2(0.55 + 0.1 * unit(shift + 4), 0.36 + 0.06 * unit(shift + 12)) * size;
        (centre, radius)
    });
    let fill = FLUID.lerp_to_gamma(FLUID_WET, wet);
    for (centre, radius) in dabs {
        out.push(Drawn::Ellipse {
            centre,
            radius,
            fill,
            rim: Some(FLUID_RIM),
        });
    }
    // Refill inside: only the blob's outer rim may show.
    for (centre, radius) in dabs {
        out.push(Drawn::Ellipse {
            centre,
            radius: radius - Vec2::splat(RIM_WIDTH),
            fill,
            rim: None,
        });
    }
    if wet > 0.0 {
        out.push(Drawn::Ellipse {
            centre: cell.center() + vec2(-0.18, -0.16) * size,
            radius: vec2(0.16, 0.07) * size,
            fill: Color32::WHITE.gamma_multiply(wet),
            rim: None,
        });
    }
}

/// Ink run on wet fluid: faint copies scattered a little.
fn smudged(out: &mut Vec<Drawn>, at: Pos2, c: char, color: Color32, scale: f32, seed: u64) {
    let unit = |shift: u32| ((seed >> shift) & 0xFF) as f32 / 255.0 - 0.5;
    for i in 0..4_u32 {
        let offset = vec2(unit(i * 16), unit(i * 16 + 8)) * (2.0 * SMUDGE_SPREAD * scale);
        out.push(Drawn::Glyph {
            at: at + offset,
            c,
            color: color.gamma_multiply(0.35),
        });
    }
}

/// Offset and density for one strike. Seeded by position: stable across
/// redraws.
fn ink_variation(half_line: u16, column: u16, index: usize) -> (Vec2, f32) {
    let bits = mark_seed(half_line, column, index);
    let unit = |shift| unit(bits, shift);
    let offset = vec2(unit(0) - 0.5, unit(16) - 0.5) * (2.0 * INK_MAX_OFFSET);
    let density = 1.0 - INK_DENSITY_VARIANCE * unit(32);
    (offset, density)
}

/// A dead key's accent and the letter struck on it, as mark indices and the
/// font's composed letter: its accent sits clear of capitals and ascenders,
/// where two glyphs struck in one cell collide.
fn composed(marks: &[Mark]) -> Option<(usize, usize, char)> {
    let struck = |m: &Mark| match m {
        Mark::Glyph(c) | Mark::Smudged(c) => Some(*c),
        Mark::Correction(_) => None,
    };
    let (accent, a) = marks
        .iter()
        .enumerate()
        .find_map(|(i, m)| struck(m).filter(|&c| accents::is_accent(c)).map(|c| (i, c)))?;
    marks.iter().enumerate().find_map(|(i, m)| {
        let letter = struck(m)?;
        accents::compose(letter, a).map(|c| (accent, i, c))
    })
}

/// Where a worn shift puts `mark`: capitals up a hair, the rest true.
fn shift_misalignment(mark: &Mark) -> Vec2 {
    match mark {
        Mark::Glyph(c) | Mark::Smudged(c) if c.is_uppercase() => vec2(0.0, -SHIFT_MISALIGNMENT),
        _ => Vec2::ZERO,
    }
}

/// Stable random bits for one mark.
fn mark_seed(half_line: u16, column: u16, index: usize) -> u64 {
    splitmix64((u64::from(half_line) << 40) ^ (u64::from(column) << 20) ^ index as u64)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_accent_and_its_letter_draw_as_one_composed_letter() {
        let marks = [Mark::Glyph('\u{b4}'), Mark::Glyph('E')];
        assert_eq!(composed(&marks), Some((0, 1, '\u{c9}')));
        let after = [Mark::Glyph('e'), Mark::Glyph('^')];
        assert_eq!(composed(&after), Some((1, 0, '\u{ea}')));
        assert_eq!(composed(&[Mark::Glyph('^'), Mark::Glyph('q')]), None);
        assert_eq!(composed(&[Mark::Glyph('x')]), None);
    }

    #[test]
    fn marks_on_a_refed_sheet_are_drawn_out_of_line() {
        let profile = typewriter_core::Profile::from_toml_str(include_str!(
            "../../../../profiles/olympia-sm9.toml"
        ))
        .unwrap();
        let metrics = Metrics::new(&profile, 96.0);
        let mut page = Page::new(profile.columns(), profile.half_lines());
        page.strike(12, 10, 'a');
        page.refeed(typewriter_core::page::Shift {
            across: 50,
            down: -25,
        });
        page.strike(12, 10, 'b');
        let at: Vec<Pos2> = sheet_marks(
            &metrics,
            &page,
            Pos2::ZERO,
            false,
            Dimming::NONE,
            &dry,
            |_| true,
        )
        .into_iter()
        .filter_map(|drawn| match drawn {
            Drawn::Glyph { at, .. } => Some(at),
            _ => None,
        })
        .collect();
        let moved = at[1] - at[0];
        assert!((moved.x - 0.5 * metrics.column_width).abs() < 1e-3);
        assert!((moved.y + 0.25 * metrics.half_line_height).abs() < 1e-3);
    }

    #[test]
    fn an_erased_accent_stays_erased_under_its_letter() {
        let profile = typewriter_core::Profile::from_toml_str(include_str!(
            "../../../../profiles/olympia-sm9.toml"
        ))
        .unwrap();
        let metrics = Metrics::new(&profile, 96.0);
        let mut page = Page::new(profile.columns(), profile.half_lines());
        page.strike(12, 10, '\u{b4}');
        page.cover(12, 10, Correction::Eraser);
        page.strike(12, 10, 'e');
        let glyphs = |page: &Page| -> String {
            sheet_marks(
                &metrics,
                page,
                Pos2::ZERO,
                false,
                Dimming::NONE,
                &dry,
                |_| true,
            )
            .into_iter()
            .filter_map(|drawn| match drawn {
                Drawn::Glyph { c, .. } => Some(c),
                _ => None,
            })
            .collect()
        };
        assert_eq!(glyphs(&page), "\u{b4}e", "the ghost, then a plain e");
        page.strike(12, 11, '\u{b4}');
        page.strike(12, 11, 'e');
        assert!(glyphs(&page).ends_with('\u{e9}'));
    }

    #[test]
    fn a_worn_shift_lifts_only_capitals() {
        assert!(shift_misalignment(&Mark::Glyph('A')).y < 0.0);
        assert!(shift_misalignment(&Mark::Smudged('\u{c9}')).y < 0.0);
        assert_eq!(shift_misalignment(&Mark::Glyph('a')), Vec2::ZERO);
        assert_eq!(shift_misalignment(&Mark::Glyph('!')), Vec2::ZERO);
    }

    #[test]
    fn ink_variation_is_stable_and_subtle() {
        assert_eq!(ink_variation(12, 10, 0), ink_variation(12, 10, 0));
        assert_ne!(ink_variation(12, 10, 0), ink_variation(12, 11, 0));
        for column in 0..200 {
            let (offset, density) = ink_variation(12, column, 0);
            assert!(offset.x.abs() <= INK_MAX_OFFSET && offset.y.abs() <= INK_MAX_OFFSET);
            assert!((1.0 - INK_DENSITY_VARIANCE..=1.0).contains(&density));
        }
    }

    /// A steady frame: the print drawn, then the frame's font textures
    /// drained, as in an app.
    fn frame(draw: impl FnOnce(&eframe::egui::Painter)) {
        let ctx = eframe::egui::Context::default();
        crate::app::fonts::install(&ctx);
        let mut output = ctx.run_ui(eframe::egui::RawInput::default(), |_| {});
        output.textures_delta.clear(); // egui withholds its fonts until a first run
        let painter = eframe::egui::Painter::new(
            ctx.clone(),
            eframe::egui::LayerId::background(),
            Rect::EVERYTHING,
        );
        draw(&painter);
        let mut output = ctx.run_ui(eframe::egui::RawInput::default(), |_| {});
        output.textures_delta.clear(); // drain what the glyph layout queued
    }

    fn sm9() -> typewriter_core::Profile {
        typewriter_core::Profile::from_toml_str(include_str!(
            "../../../../profiles/olympia-sm9.toml"
        ))
        .unwrap()
    }

    fn page_on(profile: &typewriter_core::Profile) -> Page {
        Page::new(profile.columns(), profile.half_lines())
    }

    #[test]
    fn the_print_keeps_up_with_the_sheet_and_the_look() {
        let profile = sm9();
        let metrics = Metrics::new(&profile, 96.0);
        let mut page = page_on(&profile);
        page.strike(12, 10, 'a');
        let mut cache = SheetPrint::default();
        let mut first: Vec<Shape> = Vec::new();
        let mut after: Vec<Shape> = Vec::new();
        let mut moved: Vec<Shape> = Vec::new();
        frame(|painter| {
            let look = SheetLook {
                metrics: &metrics,
                origin: Pos2::ZERO,
                ink_realism: false,
                dimming: Dimming::NONE,
                wetness: &dry,
                drying: false,
            };
            first = cache
                .shapes(painter, &look, &page, Rect::EVERYTHING)
                .to_vec();
            // Nothing moved: the kept print answers.
            assert_eq!(
                cache.shapes(painter, &look, &page, Rect::EVERYTHING),
                &first[..]
            );
            // The sheet takes a mark: the print follows.
            page.strike(12, 11, 'b');
            after = cache
                .shapes(painter, &look, &page, Rect::EVERYTHING)
                .to_vec();
            // The look moves: the print moves with it.
            let shifted = SheetLook {
                origin: pos2(10.0, 0.0),
                ..look
            };
            moved = cache
                .shapes(painter, &shifted, &page, Rect::EVERYTHING)
                .to_vec();
        });
        assert_eq!(glyphs(&first).len(), 1);
        assert_eq!(glyphs(&after).len(), 2);
        assert_eq!(moved.len(), after.len());
        assert!(
            moved.iter().zip(after.iter()).all(|(m, a)| {
                m.visual_bounding_rect() == a.visual_bounding_rect().translate(vec2(10.0, 0.0))
            }),
            "every shape moved by the new origin"
        );
    }

    #[test]
    fn a_drying_sheet_remakes_the_print_at_once() {
        let profile = sm9();
        let metrics = Metrics::new(&profile, 96.0);
        let mut page = page_on(&profile);
        page.strike(12, 10, 'x');
        page.cover(12, 10, Correction::Fluid { wet: true });
        let mut cache = SheetPrint::default();
        let (mut n, mut with_shine) = (0, 0);
        frame(|painter| {
            let look = SheetLook {
                metrics: &metrics,
                origin: Pos2::ZERO,
                ink_realism: false,
                dimming: Dimming::NONE,
                wetness: &dry,
                drying: false,
            };
            n = cache.shapes(painter, &look, &page, Rect::EVERYTHING).len();
            // The dab wets up: without `drying` the stamp alone says nothing
            // changed, which is why the app must pass it.
            let wet = |_: u16, _: u16| -> f32 { 1.0 };
            let wetting = SheetLook {
                wetness: &wet,
                drying: true,
                ..look
            };
            with_shine = cache
                .shapes(painter, &wetting, &page, Rect::EVERYTHING)
                .len();
        });
        assert!(with_shine > n, "the wet highlight is drawn");
    }

    fn glyphs(shapes: &[Shape]) -> Vec<char> {
        shapes
            .iter()
            .filter_map(|s| match s {
                Shape::Text(text) => text.galley.rows.first().and_then(|row| {
                    row.glyphs
                        .iter()
                        .find(|g| !g.chr.is_whitespace())
                        .map(|g| g.chr)
                }),
                _ => None,
            })
            .collect()
    }
}
