//! The paper texture behind everything, fixed to the window.

use eframe::egui::epaint::{Vertex, WHITE_UV};
use eframe::egui::{
    self, Color32, ColorImage, CornerRadius, Painter, Pos2, Rect, Shape, TextureFilter,
    TextureHandle, TextureOptions, Vec2, pos2,
};

use super::feed::convex_mesh;

/// Evened out and re-encoded by `build.rs`.
const PAPER_JPEG: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/paper.jpg"));
const FALLBACK: Color32 = Color32::from_rgb(0xF4, 0xF0, 0xE6);

pub struct Background {
    texture: Option<TextureHandle>,
}

impl Background {
    pub fn load(ctx: &egui::Context) -> Self {
        let max_side = ctx.input(|i| i.max_texture_side);
        let texture = match decode(PAPER_JPEG, max_side) {
            Ok(image) => Some(ctx.load_texture("paper", image, texture_options())),
            Err(err) => {
                eprintln!("paper texture unavailable, using a flat colour: {err}");
                None
            }
        };
        Self { texture }
    }

    pub fn paint(&self, painter: &Painter, view: Rect) {
        match &self.texture {
            Some(texture) => {
                let uv = cover_uv(texture.size_vec2(), view.size());
                painter.image(texture.id(), view, uv, Color32::WHITE);
            }
            None => {
                painter.rect_filled(view, CornerRadius::ZERO, FALLBACK);
            }
        }
    }

    /// Fills a convex polygon with exactly the background behind it: hides
    /// what's under a lifted sheet without a seam.
    pub fn paint_polygon(&self, painter: &Painter, view: Rect, points: &[Pos2]) {
        let mesh = match &self.texture {
            Some(texture) => {
                let uv = cover_uv(texture.size_vec2(), view.size());
                let mut mesh = convex_mesh(points, |pos| {
                    let at = (pos - view.min) / view.size();
                    Vertex {
                        pos,
                        uv: uv.min + at * uv.size(),
                        color: Color32::WHITE,
                    }
                });
                mesh.texture_id = texture.id();
                mesh
            }
            None => convex_mesh(points, |pos| Vertex {
                pos,
                uv: WHITE_UV,
                color: FALLBACK,
            }),
        };
        painter.add(Shape::mesh(mesh));
    }
}

/// Decodes, scaling down to the GPU's `max_side` if needed.
fn decode(bytes: &[u8], max_side: usize) -> image::ImageResult<ColorImage> {
    let mut paper = image::load_from_memory_with_format(bytes, image::ImageFormat::Jpeg)?;
    let max_side = u32::try_from(max_side).unwrap_or(u32::MAX);
    if paper.width().max(paper.height()) > max_side {
        paper = paper.resize(max_side, max_side, image::imageops::FilterType::Triangle);
    }
    let rgb = paper.to_rgb8();
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
