//! The CPU twin of the pass, for the snapshot tool where no wgpu renderer
//! is installed: [`rasterize`] projects and divides by hand as the shader
//! does, keeps its own depth buffer through the frame, lifts decals by the
//! same bias as the pipeline, casts the lamp's shadow map with [`cast`] as
//! its lamp pass does, and lights each pixel through
//! [`lighting`](super::lighting)'s twins of `lit()`.

use eframe::egui::epaint::PaintCallback;
use eframe::egui::{Mesh, Rect};
use glam::{Mat2, Vec2, Vec3, Vec3A};

use super::gpu::{FONTS, decal_bias, texel_scale};
use super::lighting::Lighting;
use super::solids::Solids;
use super::{Layer, NEAR_MM, Pass, Shade};

/// The lamp's map, from the pass's opaque solids: what its eye sees, before
/// the frame draws. The twin of the GPU's lamp pass — one triangle at a
/// time, the nearer one holding wherever they both cover a texel. Decals
/// lay flat and hide nothing; they cast nothing either. Vertices run out
/// past the map unclamped, as the GPU clips them: only the covered texels
/// are filled. What stands nearer the head than the map's near plane is
/// dropped as the GPU's clip would: it keeps the clear, casting nothing.
pub(crate) fn cast(solids: &Solids, lighting: &Lighting) -> Vec<f32> {
    let side = lighting.shadow.side;
    let mut spots = vec![];
    let mut map = vec![1.0; (side * side) as usize];
    for solid in solids.layers()[0].1 {
        spots.clear(); // Mutating is better since heap size is allocated
        spots.extend(solid.places.iter().map(|&mm| {
            let mm = mm.extend(1.0);
            let [u, v, d] = lighting.shadow.rows.map(|row| row.dot(mm));
            Vec3A::new((u + 1.0) * 0.5 * side, (1.0 - v) * 0.5 * side, d)
        }));
        for triangle in solid.mesh.triangles() {
            let [a, b, c] = triangle.map(|i| spots[i as usize]);
            let (a2d, b2d, c2d) = (a.truncate(), b.truncate(), c.truncate());
            let area = (b2d - a2d).perp_dot(c2d - a2d);
            if area.abs() < 1e-6 {
                continue;
            }
            // The texels the triangle can cover: its box, clamped to the map.
            let min = a.min(b).min(c).truncate().max(Vec2::ZERO);
            let max = a.max(b).max(c).truncate().ceil().min(Vec2::splat(side));
            let side = side as usize;
            for row in min.y as usize..max.y as usize {
                for column in min.x as usize..max.x as usize {
                    let p = Vec2::new(column as f32 + 0.5, row as f32 + 0.5);
                    // Barycentric weights of b and c: each is the far edge's
                    // cross with the point, over the triangle's own.
                    let w1 = (p - a2d).perp_dot(c2d - a2d) / area;
                    let w2 = (p - a2d).perp_dot(a2d - b2d) / area;
                    let w0 = 1.0 - w1 - w2;
                    if w0 < 0.0 || w1 < 0.0 || w2 < 0.0 {
                        continue;
                    }
                    let d = (a * w0 + b * w1 + c * w2).z;
                    // The GPU's lamp pass clips a fragment nearer the head
                    // than the map's near plane (clip z < 0, w = 1): like
                    // one, a sample before it writes nothing.
                    if d < 0.0 {
                        continue;
                    }
                    let there = &mut map[row * side + column];
                    if d < *there {
                        *there = d;
                    }
                }
            }
        }
    }
    map
}

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
    let map = lighting.shadow.is_on().then(|| cast(solids, lighting));
    let occlusion = |mm: Vec3| match &map {
        Some(map) => lighting.shadow.occlusion(map, mm),
        None => 1.0,
    };
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
                pixel_shade(&solid.shades, triangle, weights, &inv_w).apply(
                    rgba,
                    lighting,
                    at,
                    occlusion(at),
                )
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
    let u = b.pos - a.pos;
    let v = c.pos - a.pos;
    let basis = Mat2::from_cols(Vec2::new(u.x, v.x), Vec2::new(u.y, v.y));
    if basis.determinant().abs() < 1e-6 {
        return 0.0;
    }
    let [za, zb, zc] = triangle.map(|i| depths[i as usize]);
    // The depth plane through the corners, differentiated.
    let gradient = basis.inverse() * Vec2::new(zb - za, zc - za);
    gradient.abs().max_element()
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
        Shade::Catcher(_) => Shade::Catcher(spec),
        Shade::Unlit => first,
    }
}

#[cfg(test)]
mod tests {
    use eframe::egui::{Color32, Mesh, pos2};

    use super::*;

    #[test]
    fn a_caster_nearer_than_the_maps_near_plane_casts_no_shadow() {
        use super::super::lighting::Shadow;
        use super::super::solids::Solid;

        // The lamp's eye: a head 1000 mm above the printing point, the map
        // running from 250 mm off it over 650. The near triangle stands
        // 200 mm under the head, so every sample there has depth below 0:
        // the GPU's lamp pass clips those fragments, and the twin must keep
        // their texels at the clear. The far one is a control that the map
        // fills where it may.
        let shadow = Shadow::lamp(
            Vec3::new(0.0, 0.0, 1000.0),
            0.0,
            Vec3::Y,
            Vec3::ZERO,
            620.0,
            250.0,
            650.0,
            1.0,
        );
        let lighting = Lighting {
            shadow,
            ..Default::default()
        };
        let side = shadow.side as usize;

        let near = [
            Vec3::new(-200.0, 0.0, 800.0),
            Vec3::new(200.0, 0.0, 800.0),
            Vec3::new(0.0, 200.0, 800.0),
        ];
        let far = [
            Vec3::new(300.0, 0.0, 600.0),
            Vec3::new(500.0, 0.0, 600.0),
            Vec3::new(400.0, 200.0, 600.0),
        ];
        let mut mesh = Mesh::default();
        for mm in near.into_iter().chain(far) {
            mesh.colored_vertex(pos2(mm.x, mm.y), Color32::WHITE);
        }
        mesh.add_triangle(0, 1, 2);
        mesh.add_triangle(3, 4, 5);
        let mut solids = Solids::default();
        solids.push(
            Layer::Opaque,
            Solid::unlit(mesh, near.into_iter().chain(far).collect()),
        );

        let map = cast(&solids, &lighting);

        // The texel `cast` would read for a place standing in the map.
        let spot = |mm: Vec3| {
            let [u, v, _] = shadow.rows.map(|row| row.dot(mm.extend(1.0)));
            (
                ((u + 1.0) * 0.5 * shadow.side) as usize,
                ((1.0 - v) * 0.5 * shadow.side) as usize,
            )
        };
        let (column, row) = spot(far.into_iter().sum::<Vec3>() / 3.0);
        let there = map[row * side + column];
        assert!(there > 0.0 && there < 1.0, "far caster holds: {there}");
        // And the near one leaves its whole footprint at the clear.
        let corners = near.map(spot);
        let min = (
            corners.iter().map(|c| c.0).min().unwrap(),
            corners.iter().map(|c| c.1).min().unwrap(),
        );
        let max = (
            corners.iter().map(|c| c.0).max().unwrap(),
            corners.iter().map(|c| c.1).max().unwrap(),
        );
        for row in min.1..=max.1 {
            for column in min.0..=max.0 {
                assert_eq!(
                    map[row * side + column],
                    1.0,
                    "clipped caster cast at {row} {column}"
                );
            }
        }
    }

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
