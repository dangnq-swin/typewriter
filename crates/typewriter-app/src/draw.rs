//! What the desk edition draws with: the hooks' types, the page geometry,
//! and the drawing helpers it shares with the app. Everything else in the
//! app stays private.

pub use crate::filing::{Keeping, WriteStatus};
pub use crate::render::feed::convex_mesh;
pub use crate::render::knob::Knob;
pub use crate::render::perspective::{Camera, warp};
pub use crate::render::platen::glide_seconds;
pub use crate::render::{HIGHLIGHT, Metrics, points_per_inch, smoothstep, splitmix64, unit};
pub use crate::stage::{Controls, PaperTable, Placed, Platen, Return, Scene, SheetWay};

/// Drawing in depth: what stands in front hides what is behind.
pub mod depth {
    pub use crate::render::depth::{Layer, Solid, Solids, paint, subdivided};
}

/// The scale's height, and the plates' readings, which the desk's panel
/// shows too.
pub mod ruler {
    pub use crate::render::ruler::{
        HEIGHT, SAVED, autosave_state, correction_method, goal_reading,
    };
}
