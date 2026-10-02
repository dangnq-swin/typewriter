//! What stands in front hides what is behind: triangles drawn with a depth
//! buffer, in any order, through a paint callback on eframe's wgpu renderer.
//! The machine and the sheets draw so, in one callback a frame: [`begin`]
//! sets its place, parts [`gather`] into it, [`end`] draws it.
//!
//! The pass splits along its four contracts: [`solids`], what a frame draws;
//! [`lighting`], how it takes the light; [`gpu`], the vertex packing and
//! pipelines the GPU reads; and, in tests, `raster`, the CPU twin of the
//! same maths where no renderer runs.
//!
//! Each vertex is egui's, on screen in points, with the machine millimetres
//! it stands at and how it takes the frame's light ([`Shade`]). The frame's
//! [`Camera`] projects those millimetres into the window: the shader does
//! the project and the divide, so uv, colour and millimetres blend
//! perspective-correctly, and lights each fragment from its own millimetres
//! toward the lamp, so a flat face is not lit evenly. Opaque triangles
//! hide what is behind them; decals lie on them (print, edges, glass), hide
//! nothing and are drawn after, lifted toward the eye by the pass's depth
//! bias (`decal_bias`) rather than by standing nearer in millimetres. Where
//! no renderer is installed, as in the snapshot tool, the triangles go out as
//! data for `rasterize` to fill, its CPU twin of the same projection, with
//! the same bias.

pub mod gpu;
pub mod lighting;
pub mod solids;

#[cfg(test)]
mod raster;

#[cfg(test)]
pub use gpu::decal_bias;
pub use gpu::install;
pub use lighting::{CHROME_BANDS, Lighting, Shade};
#[cfg(test)]
pub use raster::rasterize;
pub use solids::{Layer, Placing, Solid, Solids};

use std::sync::Arc;

use eframe::egui::epaint::PaintCallback;
use eframe::egui::layers::ShapeIdx;
use eframe::egui::{Context, Id, Painter, Pos2, Shape, Vec2};
use eframe::egui_wgpu;

use gpu::Gpu;

/// The window's depth buffer, in bits: eframe's `depth_buffer`. One sample
/// a pixel, as egui's: wgpu's OpenGL backend shows nothing with more.
pub const DEPTH_BITS: u8 = 32;

/// Depth runs from this near the eye, out to far off: the near plane the
/// clip rows clamp at, in machine millimetres. `Eye` builds its rows with
/// this, and `rasterize` clamps with it.
pub const NEAR_MM: f32 = 25.4;

/// The camera's four clip rows as `vec4`s: as many as the shader's
/// `r_camera`.
const CAMERA_BYTES: u64 = 4 * 16;

/// The seated eye as clip rows — x, y, z, w — that project absolute
/// machine millimetres into the window: the shader projects vertices with
/// them, and `rasterize`'s CPU twin divides by `w` by hand. `Eye::camera`
/// builds them from the same maths as `Eye::at` and `Eye::depth`, and a
/// test keeps the two from drifting.
#[derive(Debug, Clone, Copy, Default)]
pub struct Camera {
    /// The rows, in order x, y, z, w.
    pub rows: [[f32; 4]; 4],
    /// The window's size in points the rows project into: `on_screen`
    /// divides back out of them.
    pub points: Vec2,
}

impl Camera {
    /// Where the rows show `mm`: the shader's project and divide, by hand.
    fn on_screen(&self, mm: [f32; 3]) -> Pos2 {
        let p = [mm[0], mm[1], mm[2], 1.0];
        let row = |i: usize| self.rows[i].iter().zip(p).map(|(a, b)| a * b).sum::<f32>();
        let w = row(3).max(NEAR_MM);
        Pos2::new(
            (row(0) / w + 1.0) * 0.5 * self.points.x,
            (1.0 - row(1) / w) * 0.5 * self.points.y,
        )
    }

    /// The uniform the shader reads: the rows, one `vec4` each.
    fn uniform(&self) -> [u8; CAMERA_BYTES as usize] {
        let mut out = [0u8; CAMERA_BYTES as usize];
        for (word, row) in out.chunks_mut(16).zip(self.rows) {
            for (byte, value) in word.chunks_mut(4).zip(row) {
                byte.copy_from_slice(&value.to_le_bytes());
            }
        }
        out
    }
}

/// The frame's depth pass, between [`begin`] and [`end`].
#[derive(Clone, Default)]
struct Frame {
    /// Where it draws in the painter's order. `None`: not begun.
    slot: Option<ShapeIdx>,
    solids: Solids,
    /// How its solids take the light.
    lighting: Lighting,
    /// How its solids are seen.
    camera: Camera,
}

/// A frame's depth pass as drawn: its solids, lit by `lighting`, seen
/// through `camera`.
#[derive(Debug, Default)]
pub struct Pass {
    pub solids: Solids,
    /// How its solids take the light.
    pub lighting: Lighting,
    /// How its solids are seen.
    pub camera: Camera,
}

fn frame_id() -> Id {
    Id::new("depth-pass-frame")
}

/// Starts the frame's depth pass, lit by `lighting` and seen through
/// `camera`: what is [`gather`]ed until [`end`] draws over what `painter`
/// has drawn so far, under what it draws after.
pub fn begin(painter: &Painter, lighting: Lighting, camera: Camera) {
    let slot = painter.add(Shape::Noop);
    let frame = Frame {
        slot: Some(slot),
        solids: Solids::default(),
        lighting,
        camera,
    };
    painter
        .ctx()
        .data_mut(|data| data.insert_temp(frame_id(), frame));
}

/// Adds `solids` to the frame's depth pass. Their depth decides only among
/// what is drawn in depth.
pub fn gather(ctx: &Context, solids: Solids) {
    ctx.data_mut(|data| {
        data.get_temp_mut_or_default::<Frame>(frame_id())
            .solids
            .append(solids);
    });
}

/// Solids and vertices of the last [`end`]'s pass, for the bench. Test-only:
/// the counts ride in `Context` data and cost a walk of the pass's lengths
/// each frame — nothing the app's own frames should pay.
#[cfg(test)]
pub fn counts(ctx: &Context) -> Option<(usize, usize)> {
    ctx.data(|data| data.get_temp::<Counts>(counts_id()))
        .map(|counts| (counts.solids, counts.vertices))
}

#[cfg(test)]
fn counts_id() -> Id {
    Id::new("depth-pass-counts")
}

#[cfg(test)]
#[derive(Clone, Copy)]
struct Counts {
    solids: usize,
    vertices: usize,
}

/// Records `solids`' counts for [`counts`]: lengths the pass already
/// holds, a few hundred reads.
#[cfg(test)]
fn record_counts(painter: &Painter, solids: &Solids) {
    let counted = solids.layers().map(|(_, layer)| Counts {
        solids: layer.len(),
        vertices: layer.iter().map(|solid| solid.mesh.vertices.len()).sum(),
    });
    painter.ctx().data_mut(|data| {
        data.insert_temp(
            counts_id(),
            Counts {
                solids: counted.iter().map(|c| c.solids).sum(),
                vertices: counted.iter().map(|c| c.vertices).sum(),
            },
        )
    });
}

/// Draws the frame's depth pass where it [`begin`]s, or else now, with
/// `painter`'s clip.
pub fn end(painter: &Painter) {
    let Frame {
        slot,
        solids,
        lighting,
        camera,
    } = painter
        .ctx()
        .data_mut(|data| data.remove_temp::<Frame>(frame_id()))
        .unwrap_or_default();
    #[cfg(test)]
    record_counts(painter, &solids);
    if solids.is_empty() {
        return;
    }
    if cfg!(debug_assertions) {
        solids.check_shown(&camera);
    }
    // The whole window: positions stay the window's.
    let rect = painter.ctx().viewport_rect();
    let pass = Arc::new(Pass {
        solids,
        lighting,
        camera,
    });
    let callback = if is_installed(painter.ctx()) {
        egui_wgpu::Callback::new_paint_callback(
            rect,
            Gpu {
                pass,
                ctx: painter.ctx().clone(),
            },
        )
    } else {
        PaintCallback {
            rect,
            callback: pass,
        }
    };
    match slot {
        Some(slot) => painter.set(slot, Shape::Callback(callback)),
        None => {
            painter.add(Shape::Callback(callback));
        }
    }
}

fn installed_id() -> Id {
    Id::new("depth-pass-installed")
}

fn is_installed(ctx: &Context) -> bool {
    ctx.data(|data| data.get_temp::<bool>(installed_id()))
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use eframe::egui::{Color32, Mesh, Rect, pos2};

    use super::*;

    #[test]
    fn one_pass_a_frame_where_it_began() {
        let ctx = Context::default();
        let triangle = |x: f32| {
            let mut mesh = Mesh::default();
            for (x, y) in [(0.0, 0.0), (10.0, 0.0), (0.0, 10.0)] {
                mesh.colored_vertex(pos2(x, y), Color32::WHITE);
            }
            mesh.add_triangle(0, 1, 2);
            let mut solids = Solids::default();
            solids.add(Layer::Opaque, mesh, |_| {
                Some(Placing {
                    at: [x, 0.0, 0.0],
                    shade: Shade::Unlit,
                })
            });
            solids
        };
        let mut output = ctx.run_ui(Default::default(), |ui| {
            let painter = ui.painter();
            begin(painter, Lighting::default(), Camera::default());
            gather(&ctx, triangle(12.7));
            painter.rect_filled(
                Rect::from_min_size(pos2(0.0, 0.0), Vec2::splat(5.0)),
                0,
                Color32::RED,
            );
            gather(&ctx, triangle(3.5));
            end(painter);
        });
        output.textures_delta.clear();
        let shapes: Vec<_> = output.shapes.iter().map(|s| &s.shape).collect();
        let callbacks: Vec<_> = shapes
            .iter()
            .enumerate()
            .filter_map(|(i, s)| match s {
                Shape::Callback(c) => Some((i, &c.callback.downcast_ref::<Pass>()?.solids)),
                _ => None,
            })
            .collect();
        let [(at, solids)] = callbacks[..] else {
            panic!("{shapes:?}");
        };
        // Under the flat shape drawn after it began, both triangles joined.
        assert!(matches!(shapes[at + 1], Shape::Rect(_)), "{shapes:?}");
        let [(_, opaque), _] = solids.layers();
        assert_eq!(opaque.len(), 1);
        assert_eq!(
            opaque[0].places,
            [[12.7, 0.0, 0.0]; 3]
                .into_iter()
                .chain([[3.5, 0.0, 0.0]; 3])
                .collect::<Vec<_>>()
        );
    }
}
