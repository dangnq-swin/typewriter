//! Where a part of the machine is drawn: flat on the painter, over what came
//! before; or in depth, where what stands in front hides what is behind
//! whatever the order. Filled faces hide; what lies on them (edges, marks,
//! print, glass) only shows over them.

use std::cell::RefCell;

use super::light::frame;
use crate::depth::{self, Layer, Placing, Shade, Solid, Solids};
use eframe::egui::epaint::{Vertex, WHITE_UV};
use eframe::egui::{Color32, Mesh, Painter, Shape};
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

    /// Gathered into the frame's depth pass when [`Canvas::finish`]ed.
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
                let Solid {
                    mut mesh,
                    places,
                    shades,
                    ..
                } = solid;
                let painted = mesh.vertices.iter_mut().zip(shades.into_iter().zip(places));
                for (vertex, (shade, at)) in painted {
                    vertex.color = shade.colour(vertex.color, &frame(), at);
                }
                self.painter.add(Shape::mesh(mesh));
            }
            Some(solids) => solids.borrow_mut().push(layer, solid),
        }
    }

    /// `shapes` drawn flat, lying on the machine: `place` puts each point
    /// where it shows and says its [`Placing`]: depth, millimetres and
    /// shade. Small, or flat where they lie: only their points are placed.
    pub(super) fn lay(
        &self,
        shapes: Vec<Shape>,
        place: impl FnMut(&mut Vertex) -> Option<Placing>,
    ) {
        match &self.solids {
            None => {
                let place = RefCell::new(place);
                let mut mesh: Mesh = warp(self.painter, shapes, |p| {
                    let mut spot = Vertex {
                        pos: p,
                        uv: WHITE_UV,
                        color: Color32::WHITE,
                    };
                    match (*place.borrow_mut())(&mut spot) {
                        Some(_) => spot.pos,
                        None => p,
                    }
                });
                // The depth shader's twin: flat canvases light what takes a
                // shade on the CPU, from the millimetres it stands at.
                for vertex in &mut mesh.vertices {
                    let mut spot = *vertex;
                    if let Some(Placing { at, shade, .. }) = (*place.borrow_mut())(&mut spot)
                        && shade != Shade::Unlit
                    {
                        vertex.color = shade.colour(vertex.color, &frame(), at);
                    }
                }
                self.painter.add(Shape::mesh(mesh));
            }
            Some(solids) => solids.borrow_mut().add_shapes(
                self.painter,
                Layer::Decal,
                shapes,
                f32::INFINITY,
                place,
            ),
        }
    }

    /// Hands what it gathered to the frame's depth pass.
    pub(super) fn finish(self) {
        if let Some(solids) = self.solids {
            depth::gather(self.painter.ctx(), solids.into_inner());
        }
    }
}
