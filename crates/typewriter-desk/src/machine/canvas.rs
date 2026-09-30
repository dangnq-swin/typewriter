//! Where a part of the machine is drawn: flat on the painter, over what came
//! before; or in depth, where what stands in front hides what is behind
//! whatever the order. Filled faces hide; what lies on them (edges, marks,
//! print, glass) only shows over them.

use std::cell::RefCell;

use eframe::egui::{Mesh, Painter, Pos2, Shape};
use typewriter_app::draw::depth::{self, Layer, Solid, Solids};
use typewriter_app::draw::warp;

pub(super) struct Canvas<'a> {
    painter: &'a Painter,
    /// `None`: flat.
    solids: Option<RefCell<Solids>>,
}

impl<'a> Canvas<'a> {
    pub(super) fn flat(painter: &'a Painter) -> Self {
        Self {
            painter,
            solids: None,
        }
    }

    /// Drawn when [`Canvas::finish`]ed.
    pub(super) fn depth(painter: &'a Painter) -> Self {
        Self {
            painter,
            solids: Some(RefCell::new(Solids::default())),
        }
    }

    pub(super) fn painter(&self) -> &'a Painter {
        self.painter
    }

    /// `solid` on screen, each vertex at its depth: a face if `layer` is
    /// opaque, else lying on one.
    pub(super) fn mesh(&self, layer: Layer, solid: Solid) {
        match &self.solids {
            None => {
                self.painter.add(Shape::mesh(solid.mesh));
            }
            Some(solids) => {
                let Solid { mesh, depths } = solid;
                let mut depths = depths.into_iter();
                solids
                    .borrow_mut()
                    .add(layer, mesh, |_| Some(depths.next().unwrap_or(1.0)));
            }
        }
    }

    /// `shapes` drawn flat, lying on the machine: `place` puts each point
    /// where it shows and gives its depth. Small, or flat where they lie:
    /// only their points are placed.
    pub(super) fn lay(&self, shapes: Vec<Shape>, place: impl Fn(Pos2) -> (Pos2, f32)) {
        match &self.solids {
            None => {
                let mesh: Mesh = warp(self.painter, shapes, |p| place(p).0);
                self.painter.add(Shape::mesh(mesh));
            }
            Some(solids) => {
                solids.borrow_mut().add_shapes(
                    self.painter,
                    Layer::Decal,
                    shapes,
                    f32::INFINITY,
                    |vertex| {
                        let (pos, depth) = place(vertex.pos);
                        vertex.pos = pos;
                        Some(depth)
                    },
                );
            }
        }
    }

    /// Draws what it gathered in depth.
    pub(super) fn finish(self) {
        if let Some(solids) = self.solids {
            depth::paint(self.painter, solids.into_inner());
        }
    }
}
