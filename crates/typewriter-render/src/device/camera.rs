//! The seated eye as clip rows: what projects the frame's absolute machine
//! millimetres into the target. The rows are the contract — the shader
//! projects by them, and [`Camera::project`] divides by hand the same way,
//! which the readback tests hold the GPU to.

use bytemuck::{Pod, Zeroable};
use glam::{Vec2, Vec3, Vec4};

/// Depth runs from this near the eye, out to far off: the near plane the
/// clip rows clamp at, in machine millimetres. The rows are built with
/// this, and `project` clamps with it.
pub const NEAR_MM: f32 = 25.4;

/// The uniform the shader reads, laid out as `r_camera` in `scene.wgsl`.
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct CameraUniform {
    rows: [Vec4; 4],
}

/// The camera's four clip rows as `vec4`s: as many as the shader's
/// `r_camera`.
pub const CAMERA_BYTES: u64 = std::mem::size_of::<CameraUniform>() as u64;

/// The eye as clip rows — x, y, z, w — that project absolute machine
/// millimetres into the target: the shader projects vertices with them,
/// and `project` divides by hand. Where the rows come from, `depth::Camera`
/// keeps its maths; `Eye::camera` keeps feeding it.
#[derive(Debug, Clone, Copy, Default)]
pub struct Camera {
    /// The rows, in order x, y, z, w.
    pub rows: [Vec4; 4],
    /// The target's size in the units the projection is read in — points
    /// for the window's CPU twin, pixels for the readback tests: only the
    /// ratio decides, and `project` divides back out of the rows by it.
    pub points: Vec2,
}

impl Camera {
    /// Where the rows show `mm`, in `points`' units, y down: the shader's
    /// project and divide, by hand.
    pub fn project(&self, mm: Vec3) -> [f32; 2] {
        let mm = mm.extend(1.0);
        let row = |i: usize| self.rows[i].dot(mm);
        let w = row(3).max(NEAR_MM);
        [
            (row(0) / w + 1.0) * 0.5 * self.points.x,
            (1.0 - row(1) / w) * 0.5 * self.points.y,
        ]
    }

    /// The uniform the shader reads.
    pub fn uniform(&self) -> CameraUniform {
        CameraUniform { rows: self.rows }
    }
}

#[cfg(test)]
mod tests {
    use glam::Vec3;

    use super::super::capture::{self, Rig};
    use super::super::mesh::{Mesh, Texture};
    use super::super::pipelines::LightingUniform;
    use super::super::scene::Placing;
    use super::super::vertex::{Shade, Vertex};

    /// The GPU's project and divide is the CPU's: a triangle spanning the
    /// view at three depths lands where `project` puts its vertices, to a
    /// pixel's coverage.
    #[test]
    fn the_gpu_projects_where_the_rows_say() {
        let mut rig = Rig::new([64, 64]);
        let camera = capture::camera();
        let at = [
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(42.0, -100.0, 0.0),
            Vec3::new(21.0, 0.0, 30.0),
        ];
        let vertex = |mm| Vertex::placed(mm, [0.0; 2], [0xFF; 4], Shade::Unlit);
        let mut mesh = Mesh::new(Texture::WHITE);
        mesh.triangle(vertex(at[0]), vertex(at[1]), vertex(at[2]));
        let scene = capture::scene_of("pin", &mesh, Placing::at(Vec3::ZERO));
        rig.draw(
            &scene,
            &camera,
            &LightingUniform::default(),
            wgpu::Color::BLACK,
        );
        // The triangle's pixels, as a box: the rasterizer's coverage keeps
        // to a pixel of the projected bounds. The clear's alpha is 1, so
        // only the colour channels say what was drawn.
        let drawn: Vec<[u32; 2]> = rig
            .pixels()
            .iter()
            .enumerate()
            .filter(|(_, texel)| texel[..3].iter().any(|c| *c >= 128))
            .map(|(at, _)| {
                let at = at as u32;
                [at % 64, at / 64]
            })
            .collect();
        assert!(drawn.len() > 100, "the triangle drew {}", drawn.len());
        let (min_x, max_x) = bounds(at.iter().map(|&mm| camera.project(mm)[0]));
        let (min_y, max_y) = bounds(at.iter().map(|&mm| camera.project(mm)[1]));
        let ((gx0, gx1), (gy0, gy1)) = (
            bounds(drawn.iter().map(|at| at[0] as f32)),
            bounds(drawn.iter().map(|at| at[1] as f32)),
        );
        let close = |got: f32, want: f32| (got - want).abs() <= 1.5;
        assert!(close(gx0, min_x), "left {gx0} of {min_x}");
        assert!(close(gx1, max_x), "right {gx1} of {max_x}");
        assert!(close(gy0, min_y), "top {gy0} of {min_y}");
        assert!(close(gy1, max_y), "bottom {gy1} of {max_y}");
    }

    /// The lowest and highest of `of`'s values.
    fn bounds(of: impl Iterator<Item = f32>) -> (f32, f32) {
        of.fold((f32::MAX, f32::MIN), |(lo, hi), v| (lo.min(v), hi.max(v)))
    }

    /// A standing mesh is moved by its placement's transform alone: the
    /// same mesh, standing where each placement puts it and nowhere else.
    #[test]
    fn a_placement_moves_the_mesh_it_stands_in() {
        let mut rig = Rig::new([64, 64]);
        let camera = capture::camera();
        let mesh = capture::wall(Vec3::ZERO, 10.0, [0xFF; 4]);
        let clear = wgpu::Color::BLACK;
        let lit = LightingUniform::default();
        // A hand's width left of the printing point, then the same right.
        let moves = [
            (Vec3::new(-25.0, 0.0, 0.0), 12, 32),
            (Vec3::new(25.0, 0.0, 0.0), 52, 12),
        ];
        for (at, standing, cleared) in moves {
            let scene = capture::scene_of("wall", &mesh, Placing::at(at));
            rig.draw(&scene, &camera, &lit, clear);
            assert_eq!(rig.pixel(standing, 32), [0xFF; 4], "standing at {at:?}");
            assert_eq!(rig.pixel(cleared, 32), [0, 0, 0, 0xFF], "moved off of");
        }
    }
}
