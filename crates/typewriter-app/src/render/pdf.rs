//! A PDF of the sheets as they look typed: one page per sheet, on white,
//! with every overstrike and correction drawn as on screen.

use anyhow::Context as _;
use eframe::egui::{Color32, Pos2, Vec2, vec2};
use printpdf::{
    Color, LinePoint, Mm, Op, PaintMode, ParsedFont, PdfDocument, PdfFontHandle, PdfPage,
    PdfSaveOptions, Point, Polygon, PolygonRing, Pt, Rgb, TextItem, TextMatrix, WindingOrder,
};
use typewriter_core::{Document, Profile, export};

use super::calm::Dimming;
use super::paper::{self, Drawn, RIM_WIDTH};
use super::{COURIER_PRIME, Metrics};

/// PDF units are points, 72 to the inch.
const POINTS_PER_INCH: f32 = 72.0;
/// Courier Prime's ascender as a share of the em. Glyphs are placed by
/// their top on screen but by their baseline in a PDF.
const ASCENT_EM: f32 = 1600.0 / 2048.0;
const ELLIPSE_POINTS: u16 = 32;

pub fn render(
    profile: &Profile,
    document: &Document,
    title: &str,
    ink_realism: bool,
) -> anyhow::Result<Vec<u8>> {
    let font = ParsedFont::from_bytes(COURIER_PRIME, 0, &mut Vec::new())
        .context("the typewriter font could not be read")?;
    let metrics = Metrics::new(profile, POINTS_PER_INCH);
    let mut pdf = PdfDocument::new(title);
    let font = PdfFontHandle::External(pdf.add_font(&font));
    let page = Page {
        font,
        size: metrics.font.size,
        height: metrics.paper_size.y,
    };
    let pages: Vec<PdfPage> = export::sheets(document)
        .map(|sheet| {
            let marks = paper::sheet_marks(
                &metrics,
                sheet,
                Pos2::ZERO,
                ink_realism,
                Dimming::NONE,
                &paper::dry,
                |_| true,
            );
            let mut ops = Vec::new();
            for drawn in marks {
                page.draw(&mut ops, drawn);
            }
            PdfPage::new(
                Mm(profile.paper.width_mm as f32),
                Mm(profile.paper.height_mm as f32),
                ops,
            )
        })
        .collect();
    anyhow::ensure!(!pages.is_empty(), "nothing has been typed yet");
    Ok(pdf
        .with_pages(pages)
        .save(&PdfSaveOptions::default(), &mut Vec::new()))
}

struct Page {
    font: PdfFontHandle,
    size: f32,
    height: f32,
}

impl Page {
    fn draw(&self, ops: &mut Vec<Op>, drawn: Drawn) {
        match drawn {
            Drawn::Glyph { at, c, color } => {
                let baseline = at.y + ASCENT_EM * self.size;
                ops.extend([
                    Op::StartTextSection,
                    Op::SetFont {
                        font: self.font.clone(),
                        size: Pt(self.size),
                    },
                    Op::SetFillColor {
                        col: on_white(color),
                    },
                    Op::SetTextMatrix {
                        matrix: TextMatrix::Translate(Pt(at.x), Pt(self.height - baseline)),
                    },
                    Op::ShowText {
                        items: vec![TextItem::Text(c.to_string())],
                    },
                    Op::EndTextSection,
                ]);
            }
            Drawn::Patch { rect, color } => {
                let corners = [
                    rect.left_top(),
                    rect.right_top(),
                    rect.right_bottom(),
                    rect.left_bottom(),
                ];
                self.fill(ops, &corners, on_white(color));
            }
            Drawn::Line {
                from,
                to,
                width,
                color,
            } => {
                let along = (to - from).normalized();
                let across = vec2(-along.y, along.x) * (width / 2.0);
                let corners = [from + across, to + across, to - across, from - across];
                self.fill(ops, &corners, on_white(color));
            }
            Drawn::Ellipse {
                centre,
                radius,
                fill,
                rim,
            } => {
                if let Some(rim) = rim {
                    self.fill(ops, &ellipse(centre, radius), over(rim, fill));
                    let inner = radius - Vec2::splat(RIM_WIDTH);
                    self.fill(ops, &ellipse(centre, inner), on_white(fill));
                } else {
                    self.fill(ops, &ellipse(centre, radius), on_white(fill));
                }
            }
        }
    }

    fn fill(&self, ops: &mut Vec<Op>, outline: &[Pos2], color: Color) {
        let points = outline
            .iter()
            .map(|p| LinePoint {
                p: Point {
                    x: Pt(p.x),
                    y: Pt(self.height - p.y),
                },
                bezier: false,
            })
            .collect();
        ops.push(Op::SetFillColor { col: color });
        ops.push(Op::DrawPolygon {
            polygon: Polygon {
                rings: vec![PolygonRing { points }],
                mode: PaintMode::Fill,
                winding_order: WindingOrder::NonZero,
            },
        });
    }
}

fn ellipse(centre: Pos2, radius: Vec2) -> Vec<Pos2> {
    (0..ELLIPSE_POINTS)
        .map(|i| {
            let angle = std::f32::consts::TAU * f32::from(i) / f32::from(ELLIPSE_POINTS);
            centre + vec2(angle.cos() * radius.x, angle.sin() * radius.y)
        })
        .collect()
}

/// A translucent colour as it prints on white paper.
fn on_white(color: Color32) -> Color {
    over(color, Color32::WHITE)
}

/// `top` (premultiplied, possibly translucent) laid over an opaque `below`.
fn over(top: Color32, below: Color32) -> Color {
    let clear = 1.0 - f32::from(top.a()) / 255.0;
    let channel = |t: u8, b: u8| (f32::from(t) + clear * f32::from(b)).min(255.0) / 255.0;
    Color::Rgb(Rgb {
        r: channel(top.r(), below.r()),
        g: channel(top.g(), below.g()),
        b: channel(top.b(), below.b()),
        icc_profile: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use typewriter_core::{Command, Constraints, EraseMode, Typewriter};

    fn sm9() -> Typewriter {
        let profile =
            Profile::from_toml_str(include_str!("../../../../profiles/olympia-sm9.toml")).unwrap();
        Typewriter::new(profile, Constraints::default()).unwrap()
    }

    #[test]
    fn every_sheet_becomes_a_page() {
        let mut tw = sm9();
        for c in "Dear diary,".chars() {
            tw.apply(Command::Type(c));
        }
        tw.apply(Command::SetEraseMode(EraseMode::Fluid));
        tw.apply(Command::Erase);
        tw.apply(Command::FeedSheet);
        for c in "Page two".chars() {
            tw.apply(Command::Type(c));
        }
        let pdf = render(tw.profile(), tw.document(), "Diary", true).unwrap();
        assert!(pdf.starts_with(b"%PDF-"));
        let text = String::from_utf8_lossy(&pdf).replace("/Type /", "/Type/");
        let pages = text
            .match_indices("/Type/Page")
            .filter(|(i, _)| !text[i + 10..].starts_with('s'))
            .count();
        assert_eq!(pages, 2);
        if let Some(path) = std::env::var_os("TYPEWRITER_PDF_SAMPLE") {
            std::fs::write(path, &pdf).unwrap();
        }
    }

    #[test]
    fn an_empty_folder_has_nothing_to_print() {
        let tw = sm9();
        assert!(render(tw.profile(), tw.document(), "Empty", true).is_err());
    }

    #[test]
    fn translucent_ink_prints_lighter_on_white() {
        let Color::Rgb(ink) = on_white(Color32::from_black_alpha(128)) else {
            panic!("not RGB");
        };
        assert!((ink.r - 127.0 / 255.0).abs() < 0.01);
        let Color::Rgb(white) = on_white(Color32::TRANSPARENT) else {
            panic!("not RGB");
        };
        assert_eq!(white.r, 1.0);
    }
}
