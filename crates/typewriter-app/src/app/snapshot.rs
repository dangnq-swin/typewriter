//! The typing view drawn without a window, to an image: for looking at a
//! change where there is no display. Behind the `snapshot` feature; not
//! part of either program.
//!
//! egui lays out and tessellates the frame as it would for the window; the
//! triangles are then filled here, blended as egui's painters blend them.

use std::collections::HashMap;

use anyhow::Context;
use eframe::egui::epaint::{ClippedPrimitive, ImageData, Primitive, TextureId};
use eframe::egui::{self, Color32, ColorImage, Mesh, Pos2, Rect, Vec2};
use image::RgbaImage;
use typewriter_core::{Command, Constraints, Typewriter};

use super::TypewriterApp;
use super::desk::Desk;
use super::fonts;
use super::intent::Intent;
use crate::Stage;
use crate::filing::Filing;
use crate::input::Action;
use crate::machines::Machines;
use crate::render::depth::{Layer, Solids};
use crate::render::feed::FeedMotion;
use crate::settings::Settings;

/// Past the first sheet's feed.
const SETTLED: f64 = 5.0;
/// Between keys typed.
const KEY_SECONDS: f64 = 0.1;

/// What to draw.
pub struct Shot<'a> {
    /// The window, in points.
    pub size: Vec2,
    pub zoom_percent: u16,
    /// Typed on the first sheet; a return for each `\n`.
    pub text: &'a str,
    /// After the last key: what moves settles.
    pub after_seconds: f64,
}

/// `stage`'s typing view for `shot`, a pixel a point.
pub fn render(stage: Box<dyn Stage>, shot: &Shot) -> anyhow::Result<RgbaImage> {
    let ctx = egui::Context::default();
    fonts::install(&ctx);
    let mut app = TypewriterApp::nowhere(&ctx, stage, desk(shot)?);
    let mut now = SETTLED;
    for c in shot.text.chars() {
        let command = if c == '\n' {
            Command::Return
        } else {
            Command::Type(c)
        };
        let keys = vec![Action::Machine(command)];
        app.desk.update(
            Intent::Input {
                keys,
                wheel: 0.0,
                over_knob: false,
            },
            now,
        );
        now += KEY_SECONDS;
    }
    app.desk.tick(now + shot.after_seconds);
    app.desk.take_effects();
    let mut textures = HashMap::new();
    let mut last = Vec::new();
    // Twice: the first lays out what the second draws.
    for _ in 0..2 {
        let input = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, shot.size)),
            time: Some(now + shot.after_seconds),
            ..Default::default()
        };
        let mut output = ctx.run_ui(input, |ui| {
            app.show(ui, now + shot.after_seconds);
        });
        for (id, deltas) in std::mem::take(&mut output.textures_delta.set) {
            for delta in deltas {
                let ImageData::Color(image) = delta.image;
                match delta.pos {
                    None => {
                        textures.insert(id, (*image).clone());
                    }
                    Some(at) => {
                        if let Some(texture) = textures.get_mut(&id) {
                            patch(texture, &image, at);
                        }
                    }
                }
            }
        }
        output.textures_delta.clear();
        last = ctx.tessellate(std::mem::take(&mut output.shapes), output.pixels_per_point);
    }
    rasterize(shot.size, &last, &textures)
}

/// A test desk with its first sheet in, at `shot`'s zoom.
fn desk(shot: &Shot) -> anyhow::Result<Desk> {
    let settings = Settings::default();
    let machines = Machines::built_in()?;
    let profile = machines.for_new(&settings.machine.profile);
    let constraints = settings
        .machine
        .rules
        .constraints(Constraints::default().erase);
    let machine = Typewriter::new(profile, constraints)?;
    let mut desk = Desk::new(
        machine,
        Filing::nowhere(),
        settings,
        machines,
        FeedMotion::even(1.0),
    );
    desk.zoom_percent = shot.zoom_percent;
    desk.start_frame(0.0);
    desk.tick(SETTLED);
    desk.take_effects();
    Ok(desk)
}

fn patch(texture: &mut ColorImage, part: &ColorImage, [x, y]: [usize; 2]) {
    let [width, _] = texture.size;
    for row in 0..part.size[1] {
        for column in 0..part.size[0] {
            texture.pixels[(y + row) * width + x + column] =
                part.pixels[row * part.size[0] + column];
        }
    }
}

/// Premultiplied, 0..=1.
type Rgba = [f32; 4];

fn rgba(colour: Color32) -> Rgba {
    colour.to_array().map(|c| f32::from(c) / 255.0)
}

fn rasterize(
    size: Vec2,
    primitives: &[ClippedPrimitive],
    textures: &HashMap<TextureId, ColorImage>,
) -> anyhow::Result<RgbaImage> {
    // Safe casts: a window's size in points.
    let (width, height) = (size.x as usize, size.y as usize);
    let mut pixels = vec![[0.0, 0.0, 0.0, 1.0]; width * height];
    // Cleared once a frame, as the window's.
    let mut depths = vec![1.0; width * height];
    for primitive in primitives {
        let clip = primitive
            .clip_rect
            .intersect(Rect::from_min_size(Pos2::ZERO, size));
        match &primitive.primitive {
            Primitive::Mesh(mesh) => {
                let texture = textures.get(&mesh.texture_id);
                fill(&mut pixels, width, mesh, clip, texture, None);
            }
            Primitive::Callback(callback) => {
                let Some(solids) = callback.callback.downcast_ref::<Solids>() else {
                    continue;
                };
                for (layer, solids) in solids.layers() {
                    for solid in solids {
                        let texture = textures.get(&solid.mesh.texture_id);
                        let depth = Depth {
                            of: &solid.depths,
                            buffer: &mut depths,
                            layer,
                        };
                        fill(&mut pixels, width, &solid.mesh, clip, texture, Some(depth));
                    }
                }
            }
        }
    }
    let bytes = pixels
        .iter()
        .flat_map(|[r, g, b, _]| {
            [*r, *g, *b, 1.0].map(|c| (c.clamp(0.0, 1.0) * 255.0).round() as u8)
        })
        .collect();
    RgbaImage::from_raw(width as u32, height as u32, bytes).context("a pixel for each point")
}

/// A solid's vertices' depths, tested against the frame's depth `buffer`.
struct Depth<'a> {
    of: &'a [f32],
    buffer: &'a mut [f32],
    layer: Layer,
}

/// Each triangle of `mesh` inside `clip`, blended over `pixels`; in depth,
/// only where nearer than what is there.
fn fill(
    pixels: &mut [Rgba],
    width: usize,
    mesh: &Mesh,
    clip: Rect,
    texture: Option<&ColorImage>,
    mut depth: Option<Depth>,
) {
    for triangle in mesh.indices.as_chunks::<3>().0 {
        let [a, b, c] = [0, 1, 2].map(|k| &mesh.vertices[triangle[k] as usize]);
        let area = edge(a.pos, b.pos, c.pos);
        if area.abs() < 1e-6 {
            continue;
        }
        let bounds = Rect::from_points(&[a.pos, b.pos, c.pos]).intersect(clip);
        if !bounds.is_positive() {
            continue;
        }
        // Safe casts: inside the window.
        let (x0, x1) = (bounds.min.x.floor() as usize, bounds.max.x.ceil() as usize);
        let (y0, y1) = (bounds.min.y.floor() as usize, bounds.max.y.ceil() as usize);
        for y in y0..y1 {
            for x in x0..x1 {
                let p = Pos2::new(x as f32 + 0.5, y as f32 + 0.5);
                if !clip.contains(p) {
                    continue;
                }
                let w = [
                    edge(b.pos, c.pos, p),
                    edge(c.pos, a.pos, p),
                    edge(a.pos, b.pos, p),
                ]
                .map(|e| e / area);
                if w.iter().any(|&w| w < -1e-4) {
                    continue;
                }
                if let Some(depth) = &mut depth {
                    let z = [0, 1, 2]
                        .map(|k| depth.of[triangle[k] as usize])
                        .iter()
                        .zip(w)
                        .map(|(z, w)| z * w)
                        .sum::<f32>();
                    let there = &mut depth.buffer[y * width + x];
                    match depth.layer {
                        Layer::Opaque if z < *there => *there = z,
                        Layer::Decal if z <= *there => {}
                        _ => continue,
                    }
                }
                let [ca, cb, cc] = [a.color, b.color, c.color].map(rgba);
                let colour: Rgba = [0, 1, 2, 3].map(|k| w[0] * ca[k] + w[1] * cb[k] + w[2] * cc[k]);
                let uv = a.uv.to_vec2() * w[0] + b.uv.to_vec2() * w[1] + c.uv.to_vec2() * w[2];
                let texel = texture.map_or([1.0; 4], |t| sample(t, uv));
                let source: Rgba = [0, 1, 2, 3].map(|k| colour[k] * texel[k]);
                let target = &mut pixels[y * width + x];
                for k in 0..4 {
                    target[k] = source[k] + target[k] * (1.0 - source[3]);
                }
            }
        }
    }
}

fn edge(a: Pos2, b: Pos2, p: Pos2) -> f32 {
    (b.x - a.x) * (p.y - a.y) - (b.y - a.y) * (p.x - a.x)
}

/// Bilinear, clamped at the edges.
fn sample(texture: &ColorImage, uv: Vec2) -> Rgba {
    let [width, height] = texture.size;
    let x = (uv.x * width as f32 - 0.5).clamp(0.0, (width - 1) as f32);
    let y = (uv.y * height as f32 - 0.5).clamp(0.0, (height - 1) as f32);
    let (x0, y0) = (x.floor() as usize, y.floor() as usize);
    let (x1, y1) = ((x0 + 1).min(width - 1), (y0 + 1).min(height - 1));
    let (fx, fy) = (x - x0 as f32, y - y0 as f32);
    let at = |x: usize, y: usize| rgba(texture.pixels[y * width + x]);
    let (p00, p10, p01, p11) = (at(x0, y0), at(x1, y0), at(x0, y1), at(x1, y1));
    [0, 1, 2, 3].map(|k| {
        let top = p00[k] + (p10[k] - p00[k]) * fx;
        let bottom = p01[k] + (p11[k] - p01[k]) * fx;
        top + (bottom - top) * fy
    })
}
