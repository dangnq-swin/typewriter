//! What a frame draws, as the app gathers it: meshes named and placed —
//! opaque first, then what lies on them, then text — each placement in
//! absolute machine millimetres, carried by a transform. A standing mesh
//! is uploaded once and moved by its placement alone; the shadow pass
//! draws what registers as its casters.

use glam::{Mat4, Vec3};

use super::mesh::Mesh;

/// Whether triangles hide what is behind them — and where in the frame's
/// order they draw.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Layer {
    Opaque,
    /// Lying on an opaque surface, drawn after: hides nothing, and lifted
    /// toward the eye by the pipeline's depth bias so it shows over the
    /// face it lies on.
    Decal,
    /// Text on a surface, drawn after the decals: hides nothing, as ink
    /// does.
    Text,
}

/// Where a placed mesh stands: the transform from its own millimetres to
/// absolute ones, and whether the lamp's shadow pass draws it — the hook
/// lighting's casters register into.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Placing {
    /// Local millimetres into absolute ones: a translation for a solid
    /// standing where it was built, a turn for a part that moves.
    pub model: Mat4,
    /// The shadow pass draws these faces: an opaque solid does, a decal
    /// lying flat does not.
    pub casts: bool,
}

impl Placing {
    /// Standing at `at`, absolute machine millimetres, unturned: it casts
    /// as any solid does.
    pub fn at(at: Vec3) -> Self {
        Self {
            model: Mat4::from_translation(at),
            casts: true,
        }
    }

    /// Lying on what holds it: casts nothing, as a decal does.
    pub fn flat(at: Vec3) -> Self {
        Self {
            model: Mat4::from_translation(at),
            casts: false,
        }
    }

    /// Says whether the lamp's shadow pass draws it.
    pub fn casting(mut self, casts: bool) -> Self {
        self.casts = casts;
        self
    }
}

/// What a frame draws, gathered afresh each frame. The meshes are
/// borrowed: the app keeps them, the scene says which draw.
#[derive(Debug, Default)]
pub struct Scene<'a> {
    /// `(name, mesh)` in registration order, a later registration under a
    /// name replacing the earlier.
    meshes: Vec<(String, &'a Mesh)>,
    placed: Vec<Placed>,
}

/// A placement in the frame's draw list.
#[derive(Debug)]
pub(super) struct Placed {
    pub(super) mesh: String,
    pub(super) layer: Layer,
    pub(super) placing: Placing,
}

impl<'a> Scene<'a> {
    /// Draws `mesh` as `name`, standing where `placing` puts it, in the
    /// opaque layer. The device keeps a named mesh's triangles uploaded
    /// across frames, and re-uploads only where a frame's mesh differs
    /// from what it holds. Where a frame registers a name more than once,
    /// every placement of the name draws the last mesh registered.
    pub fn opaque(&mut self, name: impl Into<String>, mesh: &'a Mesh, placing: Placing) {
        let name = name.into();
        self.meshes.push((name.clone(), mesh));
        self.placed.push(Placed {
            mesh: name,
            layer: Layer::Opaque,
            placing,
        });
    }

    /// Draws `mesh` as `name`, lying where `placing` puts it, over the
    /// opaque faces: hides nothing, and [`Placing::flat`] casts nothing
    /// unless the placement says otherwise.
    pub fn decal(&mut self, name: impl Into<String>, mesh: &'a Mesh, placing: Placing) {
        let name = name.into();
        self.meshes.push((name.clone(), mesh));
        self.placed.push(Placed {
            mesh: name,
            layer: Layer::Decal,
            placing,
        });
    }

    /// Draws `mesh` as `name` as text lying on a surface: the placeholder
    /// pipeline T4's glyphs grow into, hiding nothing.
    pub fn text(&mut self, name: impl Into<String>, mesh: &'a Mesh, placing: Placing) {
        let name = name.into();
        self.meshes.push((name.clone(), mesh));
        self.placed.push(Placed {
            mesh: name,
            layer: Layer::Text,
            placing,
        });
    }

    /// The frame's meshes, in registration order: a later one under a name
    /// is what the frame holds.
    pub(super) fn meshes(&self) -> &[(String, &'a Mesh)] {
        &self.meshes
    }

    /// The frame's placements, in draw order.
    pub(super) fn placed(&self) -> &[Placed] {
        &self.placed
    }
}

#[cfg(test)]
mod tests {
    use glam::Vec3;

    use super::*;
    use crate::device::mesh::Texture;
    use crate::device::vertex::{Shade, Vertex};

    /// A one-triangle mesh on the white texel, standing at `at`.
    fn triangle(at: Vec3) -> Mesh {
        let mut mesh = Mesh::new(Texture::WHITE);
        let vertex = |mm| Vertex::placed(mm, [0.0; 2], [0xFF; 4], Shade::Unlit);
        mesh.triangle(vertex(at), vertex(at + Vec3::X), vertex(at + Vec3::Z));
        mesh
    }

    #[test]
    fn a_scene_draws_opaque_then_what_lies_on_it() {
        // Placed in a scrambled order, the frame still draws the layers in
        // the pass's order, and each layer in the order it was placed.
        let mesh = triangle(Vec3::ZERO);
        let mut scene = Scene::default();
        scene.text("glyph", &mesh, Placing::flat(Vec3::ZERO));
        scene.decal("print", &mesh, Placing::flat(Vec3::ZERO));
        scene.opaque("body", &mesh, Placing::at(Vec3::ZERO));
        scene.decal("edge", &mesh, Placing::flat(Vec3::ZERO));
        scene.opaque("case", &mesh, Placing::at(Vec3::ZERO));
        let layers: Vec<_> = scene.placed().iter().map(|p| p.layer).collect();
        assert_eq!(
            layers,
            [
                Layer::Text,
                Layer::Decal,
                Layer::Opaque,
                Layer::Decal,
                Layer::Opaque
            ]
        );
        // What the pass reads, in its own order: opaque, then decals, then
        // text, each layer's order kept.
        let order = |layer| {
            scene
                .placed()
                .iter()
                .filter(|p| p.layer == layer)
                .map(|p| p.mesh.as_str())
                .collect::<Vec<_>>()
        };
        assert_eq!(order(Layer::Opaque), ["body", "case"]);
        assert_eq!(order(Layer::Decal), ["print", "edge"]);
        assert_eq!(order(Layer::Text), ["glyph"]);
    }

    #[test]
    fn a_placement_stands_in_absolute_millimetres_and_casts() {
        let mesh = triangle(Vec3::ZERO);
        let mut scene = Scene::default();
        scene.opaque("body", &mesh, Placing::at(Vec3::new(12.7, 0.0, 0.0)));
        // An opaque solid casts as it stands; a decal lying flat does not,
        // unless it is registered a caster.
        scene.decal("print", &mesh, Placing::flat(Vec3::new(0.0, 0.0, 1.27)));
        scene.decal("ribbon", &mesh, Placing::flat(Vec3::ZERO).casting(true));
        let [body, print, ribbon] = scene.placed() else {
            panic!("{:?}", scene.placed());
        };
        assert_eq!(
            body.placing.model.w_axis.truncate(),
            Vec3::new(12.7, 0.0, 0.0),
            "the placement, absolute"
        );
        assert!(body.placing.casts);
        assert!(!print.placing.casts);
        assert!(ribbon.placing.casts, "the hook took the registration");
    }

    #[test]
    fn a_name_replaces_its_mesh_and_every_placement_draws_the_last() {
        let first = triangle(Vec3::ZERO);
        let second = triangle(Vec3::new(0.0, 1.0, 0.0));
        let mut scene = Scene::default();
        scene.opaque("body", &first, Placing::at(Vec3::ZERO));
        scene.opaque("body", &second, Placing::at(Vec3::ZERO));
        scene.opaque("body", &second, Placing::at(Vec3::X));
        let meshes: Vec<_> = scene
            .meshes()
            .iter()
            .map(|(name, mesh)| (name.as_str(), *mesh))
            .collect();
        assert_eq!(meshes.len(), 3, "registered in order");
        assert_eq!(meshes[1].1, &second, "the second registration replaced");
        assert_eq!(
            meshes[2].1, &second,
            "the last registration is what the frame holds"
        );
        // Every placement of the name draws the last mesh registered: one
        // upload, one content, however many transforms move it.
        assert_eq!(scene.placed().len(), 3);
        assert!(scene.placed().iter().all(|p| p.mesh == "body"));
    }
}
