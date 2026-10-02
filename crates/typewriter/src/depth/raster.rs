//! The CPU twin of the pass, for the snapshot tool where no wgpu renderer
//! is installed: [`rasterize`] projects and divides by hand as the shader
//! does, keeps its own depth buffer through the frame, lifts decals by the
//! same bias as the pipeline, and lights each pixel through
//! [`lighting`](super::lighting)'s twins of `lit()`.

use eframe::egui::epaint::PaintCallback;
use eframe::egui::{Mesh, Rect};
use glam::Vec3;

use super::gpu::{FONTS, decal_bias, texel_scale};
use super::{Layer, NEAR_MM, Pass, Shade};

/// Fills `callback`'s pass inside `clip` in a snapshot, with a depth buffer
/// kept through the frame, each pixel lit as the shader lights it.
pub fn rasterize(
    callback: &PaintCallback,
    clip: Rect,
    raster: &mut typewriter_ui::snapshot::Raster,
) {
    let Some(Pass {
        solids,
        lighting,
        camera,
    }) = callback.callback.downcast_ref::<Pass>()
    else {
        return;
    };
    let [width, height] = raster.size();
    // Cleared once a frame, as the window's.
    let mut buffer = raster
        .frame
        .take()
        .and_then(|frame| frame.downcast::<Vec<f32>>().ok())
        .map_or_else(|| vec![1.0; width * height], |buffer| *buffer);
    for (layer, solids) in solids.layers() {
        for solid in solids {
            let fonts = raster.texture_size(FONTS).unwrap_or([1, 1]);
            let texels = texel_scale(solid.mesh.texture_id, fonts);
            let mut mesh = solid.mesh.clone();
            for vertex in &mut mesh.vertices {
                vertex.uv = (vertex.uv.to_vec2() * texels).to_pos2();
            }
            // The shader's twin: the camera's rows on each vertex's
            // millimetres. `w` clamps at the near plane as `Eye::distance`
            // does; the hardware would clip such a vertex instead, unseen.
            let mut inv_w = Vec::with_capacity(solid.places.len());
            let mut depths = Vec::with_capacity(solid.places.len());
            for &at in &solid.places {
                let at = at.extend(1.0);
                let w = camera.rows[3].dot(at).max(NEAR_MM);
                inv_w.push(1.0 / w);
                depths.push(1.0 - NEAR_MM / w);
            }
            // Perspective-correct blending of what a triangle's own blend
            // cannot give: weight each vertex's attribute by its screen
            // weight over its `w`, and divide by that of `1 / w`.
            let divide = |weights: [f32; 3], triangle: [u32; 3]| {
                (0..3)
                    .map(|k| weights[k] * inv_w[triangle[k] as usize])
                    .sum::<f32>()
            };
            let keep = |pixel: usize, triangle: [u32; 3], weights: [f32; 3]| {
                // Window depth is the one blend that takes no divide: it
                // runs affine in screen space, as the hardware's own
                // interpolation does. Perspective-correcting it would
                // pull big triangles toward the eye mid-face, where the
                // GPU does not, and their coplanar decals would sink.
                let depth: f32 = (0..3)
                    .map(|k| depths[triangle[k] as usize] * weights[k])
                    .sum();
                let there = &mut buffer[pixel];
                match layer {
                    Layer::Opaque if depth < *there => {
                        *there = depth;
                        true
                    }
                    // The decal pipeline's bias, applied as the GPU would.
                    Layer::Decal => {
                        let lift = decal_bias(decal_slope(&mesh, &depths, triangle));
                        depth - lift <= *there
                    }
                    Layer::Opaque => false,
                }
            };
            let shade = |triangle: [u32; 3], weights: [f32; 3], rgba| {
                // The shader blends each vertex's millimetres across the
                // triangle perspective-correctly, through the divide: so
                // this, to find the fragment's place on the lamp.
                let sum = divide(weights, triangle);
                let along = |k: usize| {
                    (0..3)
                        .map(|c| {
                            let vertex = triangle[c] as usize;
                            weights[c] * inv_w[vertex] * solid.places[vertex][k]
                        })
                        .sum::<f32>()
                        / sum
                };
                let at = Vec3::new(along(0), along(1), along(2));
                pixel_shade(&solid.shades, triangle, weights, &inv_w).apply(rgba, lighting, at)
            };
            raster.fill_projected(&mesh, clip, &inv_w, keep, shade);
        }
    }
    raster.frame = Some(Box::new(buffer));
}

/// The steepest depth `triangle` of `mesh` runs per screen pixel, either
/// way it runs: what the GPU takes as the primitive's maximum slope, for
/// the decal bias. A point is a pixel: snapshots render one a window point.
fn decal_slope(mesh: &Mesh, depths: &[f32], triangle: [u32; 3]) -> f32 {
    let [a, b, c] = triangle.map(|i| &mesh.vertices[i as usize]);
    let (ux, uy) = (b.pos.x - a.pos.x, b.pos.y - a.pos.y);
    let (vx, vy) = (c.pos.x - a.pos.x, c.pos.y - a.pos.y);
    let area = ux * vy - uy * vx;
    if area.abs() < 1e-6 {
        return 0.0;
    }
    let [za, zb, zc] = triangle.map(|i| depths[i as usize]);
    let (du, dv) = (zb - za, zc - za);
    // The depth plane through the corners, differentiated.
    let (dzdx, dzdy) = ((du * vy - dv * uy) / area, (dv * ux - du * vx) / area);
    dzdx.abs().max(dzdy.abs())
}

/// The shade at a pixel of screen `weights` across `triangle`, as the shader
/// makes it: the way the surface faces or runs along and the material's
/// parameters blended across it perspective-correctly — screen weights
/// over each vertex's `w` (`inv_w`), divided by their blend — its material
/// and its highlight taken from the first corner, which is what the
/// shader's flat attributes give.
fn pixel_shade(shades: &[Shade], triangle: [u32; 3], weights: [f32; 3], inv_w: &[f32]) -> Shade {
    let corners = triangle.map(|i| shades[i as usize]);
    let first = corners[0];
    if first == Shade::Unlit {
        return first;
    }
    let sum: f32 = (0..3)
        .map(|k| weights[k] * inv_w[triangle[k] as usize])
        .sum();
    let mut vector = Vec3::ZERO;
    let mut spec = 0.0;
    for (k, (shade, weight)) in corners.into_iter().zip(weights).enumerate() {
        let blend = weight * inv_w[triangle[k] as usize] / sum;
        let (_, faced, param, _) = shade.parts();
        vector += faced * blend;
        spec += param * blend;
    }
    let vector = vector.normalize_or_zero();
    let (sharpness, down) = (spec, spec);
    match first {
        Shade::Matte(_) => Shade::Matte(vector),
        Shade::Polished { shine, .. } => Shade::Polished {
            normal: vector,
            shine,
            sharpness,
        },
        Shade::Streak { shine, .. } => Shade::Streak {
            tangent: vector,
            shine,
            sharpness,
        },
        Shade::Chrome(_) => Shade::Chrome(down),
        Shade::Unlit => first,
    }
}

#[cfg(test)]
mod tests {
    use eframe::egui::{Color32, Mesh, pos2};

    use super::*;

    #[test]
    fn a_triangles_depth_slope_is_its_steepest_rate_a_pixel() {
        let mut mesh = Mesh::default();
        for (x, y) in [(0.0, 0.0), (10.0, 0.0), (0.0, 20.0)] {
            mesh.colored_vertex(pos2(x, y), Color32::WHITE);
        }
        // Depth runs down the screen only: 0.1 over 20 points.
        let depths = [0.5, 0.5, 0.6];
        assert!((decal_slope(&mesh, &depths, [0, 1, 2]) - 0.005).abs() < 1e-6);
        // And along the screen's other axis: 0.1 over 10 points.
        let depths = [0.5, 0.6, 0.5];
        assert!((decal_slope(&mesh, &depths, [0, 1, 2]) - 0.01).abs() < 1e-6);
        // Rising toward the eye counts as steep the same way.
        let depths = [0.5, 0.5, 0.4];
        assert!((decal_slope(&mesh, &depths, [0, 1, 2]) - 0.005).abs() < 1e-6);
    }
}
