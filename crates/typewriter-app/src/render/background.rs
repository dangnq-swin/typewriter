//! The paper behind everything, fixed to the window: the bundled photo, a
//! flat paper tone or the user's own texture. An edition with its own
//! backdrop draws that instead, and the photo on each sheet.

use std::path::{Path, PathBuf};

use eframe::egui::epaint::{Mesh, Vertex, WHITE_UV};
use eframe::egui::{
    self, Color32, ColorImage, CornerRadius, Painter, Pos2, Rect, Shape, TextureFilter,
    TextureHandle, TextureOptions, Vec2, pos2,
};

use super::feed::convex_mesh;
use crate::settings::{self, Look, PaperTone};

/// Evened out and re-encoded by `build.rs`.
const PAPER_JPEG: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/paper.jpg"));
const FALLBACK: Color32 = tone(PaperTone::Ivory);

pub const fn tone(tone: PaperTone) -> Color32 {
    match tone {
        PaperTone::White => Color32::from_rgb(0xFA, 0xF9, 0xF5),
        PaperTone::Ivory => Color32::from_rgb(0xF4, 0xF0, 0xE6),
        PaperTone::Cream => Color32::from_rgb(0xF3, 0xE9, 0xD2),
        PaperTone::Buff => Color32::from_rgb(0xE6, 0xD6, 0xB2),
        PaperTone::Blue => Color32::from_rgb(0xDE, 0xE7, 0xEF),
        PaperTone::Green => Color32::from_rgb(0xE0, 0xEB, 0xDB),
        PaperTone::Pink => Color32::from_rgb(0xF3, 0xDF, 0xDD),
    }
}

/// The look's background keys, to tell when they change.
#[derive(Clone, PartialEq)]
struct Chosen(settings::Background, PaperTone, Option<PathBuf>);

impl Chosen {
    /// The bundled photo: the sheets', behind a backdrop.
    fn paper() -> Self {
        Self(settings::Background::Paper, PaperTone::default(), None)
    }

    fn of(look: &Look) -> Self {
        Self(
            look.background,
            look.background_colour,
            look.background_texture.clone(),
        )
    }
}

enum Fill {
    Texture(TextureHandle),
    Flat(Color32),
}

pub struct Background {
    fill: Fill,
    chosen: Chosen,
    /// The bundled photo, once decoded. `None` inside: it failed.
    paper: Option<Option<TextureHandle>>,
    /// Why the chosen texture isn't showing.
    problem: Option<String>,
    /// An edition's own, behind everything: `fill` is then the sheets'.
    backdrop: Option<fn(&Painter, Rect)>,
}

impl Background {
    /// Forgets `look`'s texture if its file is gone. With a `backdrop`,
    /// `look` is ignored: the backdrop is always there.
    pub fn load(
        ctx: &egui::Context,
        look: &mut Look,
        backdrop: Option<fn(&Painter, Rect)>,
    ) -> Self {
        let mut background = Self {
            fill: Fill::Flat(FALLBACK),
            chosen: if backdrop.is_some() {
                Chosen::paper()
            } else {
                Chosen::of(look)
            },
            paper: None,
            problem: None,
            backdrop,
        };
        background.fill = background.choose(ctx);
        if backdrop.is_none() {
            look.background_texture.clone_from(&background.chosen.2);
        }
        background
    }

    /// Puts changed background settings into effect. Forgets `look`'s
    /// texture if its file is gone.
    pub fn follow(&mut self, ctx: &egui::Context, look: &mut Look) {
        let chosen = Chosen::of(look);
        if self.backdrop.is_none() && chosen != self.chosen {
            self.chosen = chosen;
            self.fill = self.choose(ctx);
            look.background_texture.clone_from(&self.chosen.2);
        }
    }

    /// Why the own texture fell back to the bundled paper.
    pub fn problem(&self) -> Option<&str> {
        self.problem.as_deref()
    }

    fn choose(&mut self, ctx: &egui::Context) -> Fill {
        self.problem = None;
        let Chosen(background, colour, texture) = &mut self.chosen;
        match (background, texture.as_deref()) {
            (settings::Background::Colour, _) => return Fill::Flat(tone(*colour)),
            (settings::Background::Texture, Some(path)) => match own_texture(ctx, path) {
                Ok(texture) => return Fill::Texture(texture),
                // Deleted or moved: back to none chosen, no complaint.
                Err(Unshown::Gone) => *texture = None,
                Err(Unshown::Problem(err)) => self.problem = Some(err),
            },
            // No file chosen yet: the paper until one is.
            (settings::Background::Texture, None) | (settings::Background::Paper, _) => {}
        }
        let paper = self.paper.get_or_insert_with(|| {
            let image = decode(PAPER_JPEG, max_side(ctx))
                .map_err(|err| eprintln!("paper texture unavailable, using a flat colour: {err}"))
                .ok()?;
            Some(ctx.load_texture("paper", image, texture_options()))
        });
        match paper {
            Some(texture) => Fill::Texture(texture.clone()),
            None => Fill::Flat(FALLBACK),
        }
    }

    pub fn paint(&self, painter: &Painter, view: Rect) {
        if let Some(backdrop) = self.backdrop {
            backdrop(painter, view);
            return;
        }
        match &self.fill {
            Fill::Texture(texture) => {
                let uv = cover_uv(texture.size_vec2(), view.size());
                painter.image(texture.id(), view, uv, Color32::WHITE);
            }
            Fill::Flat(colour) => {
                painter.rect_filled(view, CornerRadius::ZERO, *colour);
            }
        }
    }

    /// The body of `sheet`, cut to the convex polygon `points`. The plain
    /// app fills it with exactly the background behind it: a lifted sheet
    /// hides what's under it without a seam. Over a backdrop, sheets carry
    /// the paper with them.
    pub fn paint_sheet(&self, painter: &Painter, view: Rect, sheet: Rect, points: &[Pos2]) {
        let area = if self.backdrop.is_some() { sheet } else { view };
        let mesh = match &self.fill {
            Fill::Texture(texture) => {
                let uv = cover_uv(texture.size_vec2(), area.size());
                let mut mesh = convex_mesh(points, |pos| {
                    let at = (pos - area.min) / area.size();
                    Vertex {
                        pos,
                        uv: uv.min + at * uv.size(),
                        color: Color32::WHITE,
                    }
                });
                mesh.texture_id = texture.id();
                mesh
            }
            Fill::Flat(colour) => convex_mesh(points, |pos| Vertex {
                pos,
                uv: WHITE_UV,
                color: *colour,
            }),
        };
        painter.add(Shape::mesh(mesh));
    }

    /// A sheet `size` points flat, bent into `mesh`: each vertex's uv is
    /// where on the sheet it is (0..=1), its colour a tint.
    pub fn paint_bent_sheet(&self, painter: &Painter, size: Vec2, mut mesh: Mesh) {
        match &self.fill {
            Fill::Texture(texture) => {
                let uv = cover_uv(texture.size_vec2(), size);
                for vertex in &mut mesh.vertices {
                    vertex.uv = uv.min + vertex.uv.to_vec2() * uv.size();
                }
                mesh.texture_id = texture.id();
            }
            Fill::Flat(colour) => {
                for vertex in &mut mesh.vertices {
                    vertex.uv = WHITE_UV;
                    vertex.color = vertex.color * *colour;
                }
            }
        }
        painter.add(Shape::mesh(mesh));
    }
}

/// Why an own texture isn't showing.
enum Unshown {
    /// No file there any more.
    Gone,
    /// For the settings card.
    Problem(String),
}

fn own_texture(ctx: &egui::Context, path: &Path) -> Result<TextureHandle, Unshown> {
    let shown = crate::storage::home_relative(path);
    let bytes = std::fs::read(path).map_err(|err| match err.kind() {
        std::io::ErrorKind::NotFound => Unshown::Gone,
        _ => Unshown::Problem(format!("Could not read {shown}: {err}")),
    })?;
    let image = decode(&bytes, max_side(ctx))
        .map_err(|err| Unshown::Problem(format!("{shown} is not a JPEG or PNG image: {err}")))?;
    Ok(ctx.load_texture("own-paper", image, texture_options()))
}

fn max_side(ctx: &egui::Context) -> usize {
    ctx.input(|i| i.max_texture_side)
}

/// Decodes a JPEG or PNG, scaling down to the GPU's `max_side` if needed.
fn decode(bytes: &[u8], max_side: usize) -> image::ImageResult<ColorImage> {
    let mut image = image::load_from_memory(bytes)?;
    let max_side = u32::try_from(max_side).unwrap_or(u32::MAX);
    if image.width().max(image.height()) > max_side {
        image = image.resize(max_side, max_side, image::imageops::FilterType::Triangle);
    }
    let rgb = image.to_rgb8();
    let size = [rgb.width() as usize, rgb.height() as usize];
    Ok(ColorImage::from_rgb(size, rgb.as_raw()))
}

fn texture_options() -> TextureOptions {
    // Mipmaps: stop the grain shimmering when the window is smaller than
    // the photo.
    TextureOptions {
        mipmap_mode: Some(TextureFilter::Linear),
        ..TextureOptions::LINEAR
    }
}

/// Cover-fit UVs: fill `view` undistorted, cropping both sides equally.
fn cover_uv(image: Vec2, view: Vec2) -> Rect {
    let image_aspect = image.x / image.y;
    let view_aspect = view.x / view.y;
    let (w, h) = if view_aspect > image_aspect {
        (1.0, image_aspect / view_aspect)
    } else {
        (view_aspect / image_aspect, 1.0)
    };
    Rect::from_min_max(
        pos2((1.0 - w) / 2.0, (1.0 - h) / 2.0),
        pos2((1.0 + w) / 2.0, (1.0 + h) / 2.0),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use eframe::egui::vec2;

    #[test]
    fn bundled_texture_decodes() {
        let image = decode(PAPER_JPEG, 2048).unwrap();
        assert!(image.width() > 0 && image.height() > 0);
    }

    #[test]
    fn texture_fits_the_gpu_limit() {
        let image = decode(PAPER_JPEG, 1000).unwrap();
        assert_eq!(image.width().max(image.height()), 1000);
    }

    #[test]
    fn a_gone_texture_is_forgotten_and_a_broken_one_complains() {
        let ctx = egui::Context::default();
        let dir = std::env::temp_dir().join(format!("typewriter-texture-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let mut look = Look {
            background: settings::Background::Texture,
            background_texture: Some(dir.join("gone.png")),
            ..Look::default()
        };
        let background = Background::load(&ctx, &mut look, None);
        assert_eq!(look.background_texture, None);
        assert_eq!(look.background, settings::Background::Texture);
        assert_eq!(background.problem(), None);

        let broken = dir.join("broken.png");
        std::fs::write(&broken, "not an image").unwrap();
        look.background_texture = Some(broken.clone());
        let mut background = background;
        background.follow(&ctx, &mut look);
        assert_eq!(look.background_texture, Some(broken));
        assert!(background.problem().is_some());
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn png_textures_decode() {
        let mut png = std::io::Cursor::new(Vec::new());
        image::RgbImage::from_pixel(4, 3, image::Rgb([200, 190, 170]))
            .write_to(&mut png, image::ImageFormat::Png)
            .unwrap();
        let image = decode(png.get_ref(), 2048).unwrap();
        assert_eq!(image.size, [4, 3]);
    }

    #[test]
    fn cover_crops_the_overflowing_axis() {
        let tall_view = cover_uv(vec2(300.0, 200.0), vec2(100.0, 100.0));
        assert!((tall_view.height() - 1.0).abs() < 1e-6);
        assert!((tall_view.width() - 2.0 / 3.0).abs() < 1e-6);
        assert!((tall_view.center().x - 0.5).abs() < 1e-6);

        let wide_view = cover_uv(vec2(300.0, 200.0), vec2(400.0, 100.0));
        assert!((wide_view.width() - 1.0).abs() < 1e-6);
        assert!((wide_view.height() - 0.375).abs() < 1e-6);
    }
}
