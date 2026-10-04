//! The typing view drawn without a window, to an image or timed frames:
//! for looking at a change where there is no display. Behind the
//! `snapshot` feature; not part of either program.
//!
//! egui lays out and tessellates the frame as it would for the window; the
//! triangles are then filled here, blended as egui's painters blend them.
//! A stage's own paint callbacks it fills itself, on the [`Raster`].

use std::any::Any;
use std::collections::HashMap;

use anyhow::Context;
use eframe::egui::epaint::{ClippedPrimitive, ImageData, Primitive, TextureId};
use eframe::egui::{self, Color32, ColorImage, Mesh, Pos2, Rect, Vec2};
use image::RgbaImage;
use typewriter_core::{Command, Constraints, Typewriter};

use super::TypewriterApp;
use super::fonts;
use super::intent::Intent;
use super::model::Model;
use crate::Stage;
use crate::filing::Filing;
use crate::input::Action;
use crate::machines::Machines;
use crate::render::feed::FeedMotion;
use crate::settings::Settings;

/// Past the first sheet's feed.
const SETTLED: f64 = 5.0;
/// Between keys typed.
const KEY_SECONDS: f64 = 0.1;
/// Timed frames step the clock this, so egui sees a live one: 60 a second.
const FRAME_SECONDS: f64 = 1.0 / 60.0;

/// In [`Shot::text`]: the Backspace key.
pub const BACKSPACE: char = '\u{8}';
/// In [`Shot::text`]: the Erase key, per the correction method.
pub const ERASE: char = '\u{1b}';

/// What to draw.
pub struct Shot<'a> {
    /// The window, in points.
    pub size: Vec2,
    pub zoom_percent: u16,
    /// Typed on the first sheet: a return for each `\n`, Backspace for
    /// [`BACKSPACE`], Erase for [`ERASE`].
    pub text: &'a str,
    /// After the last key: what moves settles.
    pub after_seconds: f64,
}

/// A stage's steady frames, timed: what `ctx.run_ui` plus `ctx.tessellate`
/// cost for one frame of its typing view.
pub struct FrameCost {
    /// Mean over the timed frames, `run_ms` plus `tessellate_ms`.
    pub mean_ms: f64,
    /// Middle of the timed frames, whole: one layout stall rides the mean
    /// and neither end of the range, so the bench reports it beside them.
    pub median_ms: f64,
    /// Mean of `ctx.run_ui` alone: deciding what to draw.
    pub run_ms: f64,
    /// Mean of `ctx.tessellate` alone: making it into triangles.
    pub tessellate_ms: f64,
    /// Fastest and slowest timed frame, whole.
    pub fastest_ms: f64,
    pub slowest_ms: f64,
}

/// `stage`'s typing view for `shot`, a pixel a point.
pub fn render(stage: Box<dyn Stage>, shot: &Shot) -> anyhow::Result<RgbaImage> {
    let ctx = egui::Context::default();
    let (mut app, at) = staged(&ctx, stage, shot)?;
    let mut textures = HashMap::new();
    let mut last = Vec::new();
    // Twice: the first lays out what the second draws.
    for _ in 0..2 {
        let input = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, shot.size)),
            time: Some(at),
            ..Default::default()
        };
        let mut output = ctx.run_ui(input, |ui| {
            app.show(ui, at);
        });
        for (id, deltas) in std::mem::take(&mut output.textures_delta.set) {
            for delta in deltas {
                let ImageData::Color(image) = delta.image;
                match delta.pos {
                    None => {
                        textures.insert(id, (*image).clone());
                    }
                    Some(pos) => {
                        if let Some(texture) = textures.get_mut(&id) {
                            patch(texture, &image, pos);
                        }
                    }
                }
            }
        }
        output.textures_delta.clear();
        last = ctx.tessellate(std::mem::take(&mut output.shapes), output.pixels_per_point);
    }
    rasterize(shot.size, &last, &textures, app.stage.as_ref())
}

/// `stage`'s app on `shot`, drawn into `ctx` (new: it gets the fonts): the
/// keys typed, the model settled, and the instant the frames draw at.
fn staged(
    ctx: &egui::Context,
    stage: Box<dyn Stage>,
    shot: &Shot,
) -> anyhow::Result<(TypewriterApp, f64)> {
    fonts::install(ctx);
    let mut app = TypewriterApp::nowhere(ctx, stage, model(shot)?);
    let mut now = SETTLED;
    for c in shot.text.chars() {
        let command = match c {
            '\n' => Command::Return,
            BACKSPACE => Command::Backspace,
            ERASE => Command::Erase,
            c => Command::Type(c),
        };
        let keys = vec![Action::Machine(command)];
        app.model.update(
            Intent::Input {
                keys,
                wheel: 0.0,
                over_knob: false,
            },
            now,
        );
        now += KEY_SECONDS;
    }
    app.model.tick(now + shot.after_seconds);
    app.model.take_effects();
    Ok((app, now + shot.after_seconds))
}

/// `frames` of `ctx`'s frames at `size`, timed: one `run_ui` plus one
/// `tessellate` each, the clock stepping [`FRAME_SECONDS`] from `at` on so
/// egui sees it live. The first frame is a warm-up: it lays out what the
/// rest draw. Nothing is typed between frames: a steady one is measured.
/// Probes write their own `draw`; [`bench`] times the whole app's.
pub fn time_frames(
    ctx: &egui::Context,
    size: Vec2,
    at: f64,
    frames: usize,
    mut draw: impl FnMut(&mut egui::Ui, f64),
) -> FrameCost {
    let mut now = at;
    let mut frame = || {
        let input = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, size)),
            time: Some(now),
            ..Default::default()
        };
        let started = std::time::Instant::now();
        let mut output = ctx.run_ui(input, |ui| draw(ui, now));
        let run_ms = started.elapsed().as_secs_f64() * 1e3;
        output.textures_delta.clear();
        let started = std::time::Instant::now();
        let _ = ctx.tessellate(std::mem::take(&mut output.shapes), output.pixels_per_point);
        let tessellate_ms = started.elapsed().as_secs_f64() * 1e3;
        now += FRAME_SECONDS;
        (run_ms + tessellate_ms, run_ms, tessellate_ms)
    };
    frame();
    let mut sums = [0.0; 3];
    let mut wholes = Vec::with_capacity(frames);
    let (mut fastest, mut slowest) = (f64::MAX, 0.0f64);
    for _ in 0..frames {
        let (whole, run, tessellate) = frame();
        sums = [sums[0] + whole, sums[1] + run, sums[2] + tessellate];
        fastest = fastest.min(whole);
        slowest = slowest.max(whole);
        wholes.push(whole);
    }
    wholes.sort_unstable_by(|a, b| a.total_cmp(b));
    let frames = frames.max(1) as f64;
    FrameCost {
        mean_ms: sums[0] / frames,
        median_ms: wholes.get(wholes.len() / 2).copied().unwrap_or(0.0),
        run_ms: sums[1] / frames,
        tessellate_ms: sums[2] / frames,
        fastest_ms: fastest,
        slowest_ms: slowest,
    }
}

/// `stage`'s typing view on `shot`, `frames` steady frames timed. `ctx`
/// must be new: a stage drawing in depth leaves its solid and vertex counts in it, for
/// the caller to read back once the frames have run.
pub fn bench(
    ctx: &egui::Context,
    stage: Box<dyn Stage>,
    shot: &Shot,
    frames: usize,
) -> anyhow::Result<FrameCost> {
    let (mut app, at) = staged(ctx, stage, shot)?;
    Ok(time_frames(ctx, shot.size, at, frames, |ui, now| {
        let _ = app.show(ui, now);
    }))
}

/// A test model with its first sheet in, at `shot`'s zoom.
fn model(shot: &Shot) -> anyhow::Result<Model> {
    let settings = Settings::default();
    let machines = Machines::built_in()?;
    let profile = machines.for_new(&settings.machine.profile);
    let constraints = settings
        .machine
        .rules
        .constraints(Constraints::default().erase);
    let machine = Typewriter::new(profile, constraints)?;
    let mut model = Model::new(
        machine,
        Filing::nowhere(),
        settings,
        machines,
        FeedMotion::even(1.0),
    );
    model.zoom_percent = shot.zoom_percent;
    model.start_frame(0.0);
    model.tick(SETTLED);
    model.take_effects();
    Ok(model)
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
pub type Rgba = [f32; 4];

fn rgba(colour: Color32) -> Rgba {
    colour.to_array().map(|c| f32::from(c) / 255.0)
}

fn rasterize(
    size: Vec2,
    primitives: &[ClippedPrimitive],
    textures: &HashMap<TextureId, ColorImage>,
    stage: &dyn Stage,
) -> anyhow::Result<RgbaImage> {
    // Safe casts: a window's size in points.
    let (width, height) = (size.x as usize, size.y as usize);
    let mut raster = Raster {
        pixels: vec![[0.0, 0.0, 0.0, 1.0]; width * height],
        width,
        textures,
        frame: None,
    };
    for primitive in primitives {
        let clip = primitive
            .clip_rect
            .intersect(Rect::from_min_size(Pos2::ZERO, size));
        match &primitive.primitive {
            Primitive::Mesh(mesh) => raster.fill(mesh, clip, |_, _, _| true, |_, _, rgba| rgba),
            Primitive::Callback(callback) => stage.snapshot_callback(callback, clip, &mut raster),
        }
    }
    let bytes = raster
        .pixels
        .iter()
        .flat_map(|[r, g, b, _]| {
            [*r, *g, *b, 1.0].map(|c| (c.clamp(0.0, 1.0) * 255.0).round() as u8)
        })
        .collect();
    RgbaImage::from_raw(width as u32, height as u32, bytes).context("a pixel for each point")
}

/// A frame being filled, a pixel a point: for a stage's own paint
/// callbacks too ([`Stage::snapshot_callback`]).
pub struct Raster<'a> {
    pixels: Vec<Rgba>,
    width: usize,
    textures: &'a HashMap<TextureId, ColorImage>,
    /// The stage's own through the frame, e.g. a depth buffer. `None` at
    /// its start.
    pub frame: Option<Box<dyn Any>>,
}

impl Raster<'_> {
    /// Width and height, in pixels.
    pub fn size(&self) -> [usize; 2] {
        [self.width, self.pixels.len() / self.width.max(1)]
    }

    /// `texture`'s size in texels, if it is there.
    pub fn texture_size(&self, texture: TextureId) -> Option<[usize; 2]> {
        self.textures.get(&texture).map(|image| image.size)
    }

    /// Each triangle of `mesh` inside `clip`, blended over what is there.
    /// `keep` may refuse a pixel, given its index in the frame, its
    /// triangle's vertices, and their weights there; `shade` then turns its
    /// colour, given the same triangle and weights.
    pub fn fill(
        &mut self,
        mesh: &Mesh,
        clip: Rect,
        keep: impl FnMut(usize, [u32; 3], [f32; 3]) -> bool,
        shade: impl Fn([u32; 3], [f32; 3], Rgba) -> Rgba,
    ) {
        self.fill_at(mesh, clip, None, keep, shade);
    }

    /// [`Raster::fill`] with `inv_w`, one over each vertex's clip `w`:
    /// uv and colour blend perspective-correctly, as the depth shader's
    /// divide makes them. `keep` and `shade` still get the screen weights.
    pub fn fill_projected(
        &mut self,
        mesh: &Mesh,
        clip: Rect,
        inv_w: &[f32],
        keep: impl FnMut(usize, [u32; 3], [f32; 3]) -> bool,
        shade: impl Fn([u32; 3], [f32; 3], Rgba) -> Rgba,
    ) {
        self.fill_at(mesh, clip, Some(inv_w), keep, shade);
    }

    fn fill_at(
        &mut self,
        mesh: &Mesh,
        clip: Rect,
        inv_w: Option<&[f32]>,
        mut keep: impl FnMut(usize, [u32; 3], [f32; 3]) -> bool,
        shade: impl Fn([u32; 3], [f32; 3], Rgba) -> Rgba,
    ) {
        let (width, texture) = (self.width, self.textures.get(&mesh.texture_id));
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
            let over_w = inv_w.map(|w| [0, 1, 2].map(|k| w[triangle[k] as usize]));
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
                    if w.iter().any(|&w| w < -1e-4) || !keep(y * width + x, *triangle, w) {
                        continue;
                    }
                    let [ca, cb, cc] = [a.color, b.color, c.color].map(rgba);
                    let (colour, uv) = match over_w {
                        None => (
                            [0, 1, 2, 3].map(|k| w[0] * ca[k] + w[1] * cb[k] + w[2] * cc[k]),
                            a.uv.to_vec2() * w[0] + b.uv.to_vec2() * w[1] + c.uv.to_vec2() * w[2],
                        ),
                        Some([ia, ib, ic]) => {
                            // Perspective-correct: blend each attribute
                            // over its own `w`, and divide by that of `1 / w`.
                            let sum = w[0] * ia + w[1] * ib + w[2] * ic;
                            (
                                [0, 1, 2, 3].map(|k| {
                                    (w[0] * ia * ca[k] + w[1] * ib * cb[k] + w[2] * ic * cc[k])
                                        / sum
                                }),
                                (a.uv.to_vec2() * (w[0] * ia)
                                    + b.uv.to_vec2() * (w[1] * ib)
                                    + c.uv.to_vec2() * (w[2] * ic))
                                    / sum,
                            )
                        }
                    };
                    let texel = texture.map_or([1.0; 4], |t| sample(t, uv));
                    let source = shade(*triangle, w, [0, 1, 2, 3].map(|k| colour[k] * texel[k]));
                    let target = &mut self.pixels[y * width + x];
                    for k in 0..4 {
                        target[k] = source[k] + target[k] * (1.0 - source[3]);
                    }
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
