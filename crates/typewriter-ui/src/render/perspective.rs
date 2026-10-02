//! The desk in perspective: a camera over the desk plane, and flat things
//! lying or standing on it.
//!
//! egui can't draw text in perspective: a flat design is tessellated, then
//! its vertices moved onto the surface ([`warp`]).

use eframe::egui::epaint::{TessellationOptions, Tessellator};
use eframe::egui::{Color32, Mesh, Painter, Pos2, Rect, Shape, Stroke, Vec2, pos2, vec2};
use emath::Rot2;

use super::SHADOW;

const TILT_DEGREES: f32 = 38.0;

/// The calendar's grey board stand; its inside in shade.
const STAND: Color32 = Color32::from_rgb(0x5C, 0x5A, 0x5E);
const STAND_INSIDE: Color32 = Color32::from_rgb(0x46, 0x44, 0x48);
const STAND_EDGE: Color32 = Color32::from_rgb(0x30, 0x2E, 0x32);

/// Projects the desk plane. Plane `y` points away from the viewer; `lift` is
/// height above it.
pub struct Camera {
    centre: Pos2,
    distance: f32,
    sin: f32,
    cos: f32,
}

impl Camera {
    /// Looking down at the desk's `centre` from `distance`, desk units.
    pub fn new(centre: Pos2, distance: f32) -> Self {
        Self::tilted(centre, distance, TILT_DEGREES)
    }

    /// As [`Camera::new`], looking `tilt_degrees` down from level.
    pub fn tilted(centre: Pos2, distance: f32, tilt_degrees: f32) -> Self {
        let (sin, cos) = tilt_degrees.to_radians().sin_cos();
        Self {
            centre,
            distance,
            sin,
            cos,
        }
    }

    pub fn project(&self, plane: Vec2, lift: f32) -> Pos2 {
        let up = plane.y * self.cos + lift * self.sin;
        let depth = self.distance + plane.y * self.sin - lift * self.cos;
        let scale = self.distance / depth.max(1.0);
        pos2(self.centre.x + plane.x * scale, self.centre.y - up * scale)
    }
}

/// Where a flat sheet lies: its centre on the plane, turned, at a height.
#[derive(Debug, Clone, Copy)]
pub struct Placement {
    pub centre: Vec2,
    /// Clockwise radians.
    pub angle: f32,
    pub lift: f32,
}

impl Placement {
    /// A sheet point (`y` down the page) on the desk plane.
    pub fn on_plane(&self, size: Vec2, local: Vec2) -> Vec2 {
        let d = vec2(local.x - size.x / 2.0, size.y / 2.0 - local.y);
        self.centre + Rot2::from_angle(self.angle) * d
    }

    /// The part `min..max` of a sheet `size`, on screen at height `lift`.
    pub fn quad(&self, camera: &Camera, size: Vec2, min: Vec2, max: Vec2, lift: f32) -> [Pos2; 4] {
        [
            vec2(min.x, min.y),
            vec2(max.x, min.y),
            vec2(max.x, max.y),
            vec2(min.x, max.y),
        ]
        .map(|p| camera.project(self.on_plane(size, p), lift))
    }
}

/// Something on the desk, a little askew: the notebook lying flat, the
/// calendar standing.
pub struct OnDesk<'a> {
    camera: &'a Camera,
    size: Vec2,
    /// Lying: its centre. Standing: its foot's.
    centre: Vec2,
    /// Clockwise radians.
    angle: f32,
    /// Standing, leaning back this many radians from upright.
    lean: Option<f32>,
    /// Its corners on screen.
    pub quad: [Pos2; 4],
}

impl<'a> OnDesk<'a> {
    /// `size`, `centre`: desk units.
    pub fn lying(camera: &'a Camera, size: Vec2, centre: Vec2, degrees: f32) -> Self {
        Self::new(camera, size, centre, degrees, None)
    }

    pub fn standing(camera: &'a Camera, size: Vec2, foot: Vec2, degrees: f32, lean: f32) -> Self {
        Self::new(camera, size, foot, degrees, Some(lean.to_radians()))
    }

    fn new(camera: &'a Camera, size: Vec2, centre: Vec2, degrees: f32, lean: Option<f32>) -> Self {
        let mut this = Self {
            camera,
            size,
            centre,
            angle: degrees.to_radians(),
            lean,
            quad: [Pos2::ZERO; 4],
        };
        this.quad = this.on_screen(Rect::from_min_size(Pos2::ZERO, size));
        this
    }

    /// A point on it (from its top-left, y down) on the desk plane, and
    /// its height above it. `shadow`: where it falls, cast from the front.
    fn on_desk(&self, p: Pos2, shadow: bool) -> (Vec2, f32) {
        let across = p.x - self.size.x / 2.0;
        let (d, lift) = match self.lean {
            None => (vec2(across, self.size.y / 2.0 - p.y), 0.0),
            Some(lean) => {
                let up = self.size.y - p.y;
                let (back, lift) = (up * lean.sin(), up * lean.cos());
                if shadow {
                    (vec2(across, back + 0.6 * lift), 0.0)
                } else {
                    (vec2(across, back), lift)
                }
            }
        };
        (self.centre + Rot2::from_angle(self.angle) * d, lift)
    }

    fn to_screen(&self, p: Pos2) -> Pos2 {
        let (plane, lift) = self.on_desk(p, false);
        self.camera.project(plane, lift)
    }

    fn on_screen(&self, rect: Rect) -> [Pos2; 4] {
        corners(rect).map(|p| self.to_screen(p))
    }

    /// Draws `shapes`, laid out flat on it, over the shadow of `outline`.
    pub fn paint(&self, painter: &Painter, outline: Vec<Pos2>, shapes: Vec<Shape>) {
        let shadow = outline
            .into_iter()
            .map(|p| match self.lean {
                None => self.to_screen(p) + vec2(3.0, 5.0),
                Some(_) => {
                    let (plane, _) = self.on_desk(p, true);
                    self.camera.project(plane, 0.0)
                }
            })
            .collect();
        painter.add(Shape::convex_polygon(shadow, SHADOW, Stroke::NONE));
        if let Some(lean) = self.lean {
            self.paint_stand(painter, lean);
        }
        painter.add(Shape::mesh(warp(painter, shapes, |p| self.to_screen(p))));
    }

    /// A tent stand behind a standing card: a base on the desk, and a back
    /// board leaning in to meet the card at the top. Its outside faces away:
    /// only the inside shows, past the card's open side.
    fn paint_stand(&self, painter: &Painter, lean: f32) {
        let point = |across: f32, back: f32, lift: f32| {
            let plane = self.centre + Rot2::from_angle(self.angle) * vec2(across, back);
            self.camera.project(plane, lift)
        };
        let half = 0.5 * self.size.x;
        let (top_back, top_lift) = (self.size.y * lean.sin(), self.size.y * lean.cos());
        let foot_back = 2.0 * top_back;
        let base = [
            point(-half, 0.0, 0.0),
            point(half, 0.0, 0.0),
            point(half, foot_back, 0.0),
            point(-half, foot_back, 0.0),
        ];
        let board = [
            point(-half, top_back, top_lift),
            point(half, top_back, top_lift),
            point(half, foot_back, 0.0),
            point(-half, foot_back, 0.0),
        ];
        let edge = Stroke::new(1.0, STAND_EDGE);
        painter.add(Shape::convex_polygon(base.to_vec(), STAND, edge));
        painter.add(Shape::convex_polygon(board.to_vec(), STAND_INSIDE, edge));
    }
}

/// `rect`'s corners, clockwise from the top-left.
pub fn corners(rect: Rect) -> [Pos2; 4] {
    [
        rect.left_top(),
        rect.right_top(),
        rect.right_bottom(),
        rect.left_bottom(),
    ]
}

/// `p` lies inside the convex `quad`.
pub fn contains(quad: &[Pos2; 4], p: Pos2) -> bool {
    let side = |a: Pos2, b: Pos2| (b - a).x * (p - a).y - (b - a).y * (p - a).x;
    let signs = [
        side(quad[0], quad[1]),
        side(quad[1], quad[2]),
        side(quad[2], quad[3]),
        side(quad[3], quad[0]),
    ];
    signs.iter().all(|&s| s >= 0.0) || signs.iter().all(|&s| s <= 0.0)
}

pub fn add_quad(mesh: &mut Mesh, quad: [Pos2; 4], color: Color32) {
    let first = mesh.vertices.len() as u32;
    for p in quad {
        mesh.colored_vertex(p, color);
    }
    mesh.add_triangle(first, first + 1, first + 2);
    mesh.add_triangle(first, first + 2, first + 3);
}

/// `shapes` as one mesh, every vertex moved by `to_screen`: draws a flat
/// design, text included, onto a surface in perspective.
pub fn warp(painter: &Painter, shapes: Vec<Shape>, to_screen: impl Fn(Pos2) -> Pos2) -> Mesh {
    let font_image = painter.fonts(|fonts| fonts.font_image_size());
    let mut tessellator = Tessellator::new(
        painter.pixels_per_point(),
        TessellationOptions::default(),
        font_image,
        Vec::new(),
    );
    let mut mesh = Mesh::default();
    for shape in shapes {
        tessellator.tessellate_shape(shape, &mut mesh);
    }
    for vertex in &mut mesh.vertices {
        vertex.pos = to_screen(vertex.pos);
    }
    mesh
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_standing_card_rises_from_its_foot_leaning_back() {
        let camera = Camera::new(pos2(500.0, 500.0), 1000.0);
        let size = vec2(60.0, 45.0);
        let card = OnDesk::standing(&camera, size, vec2(0.0, 100.0), 0.0, 20.0);
        let (foot, foot_lift) = card.on_desk(pos2(30.0, size.y), false);
        assert_eq!((foot, foot_lift), (vec2(0.0, 100.0), 0.0));
        let (top, lift) = card.on_desk(pos2(30.0, 0.0), false);
        assert!(top.y > foot.y && lift > 0.9 * size.y);
        // Its shadow falls further back on the desk.
        let (shadow, shadow_lift) = card.on_desk(pos2(30.0, 0.0), true);
        assert!(shadow.y > top.y && shadow_lift == 0.0);
        let [top_left, .., bottom_left] = card.quad;
        assert!(top_left.y < bottom_left.y, "{top_left:?} {bottom_left:?}");
    }

    #[test]
    fn projection_shrinks_with_distance() {
        let camera = Camera::new(pos2(0.0, 0.0), 300.0);
        let near = camera.project(vec2(100.0, -50.0), 0.0);
        let far = camera.project(vec2(100.0, 50.0), 0.0);
        assert!(near.x > far.x);
        assert!(far.y < near.y);
        assert_eq!(camera.project(Vec2::ZERO, 0.0), pos2(0.0, 0.0));
    }

    #[test]
    fn quad_hit_test() {
        let quad = [
            pos2(0.0, 0.0),
            pos2(10.0, 0.0),
            pos2(12.0, 10.0),
            pos2(-2.0, 10.0),
        ];
        assert!(contains(&quad, pos2(5.0, 5.0)));
        assert!(!contains(&quad, pos2(11.0, 1.0)));
    }
}
