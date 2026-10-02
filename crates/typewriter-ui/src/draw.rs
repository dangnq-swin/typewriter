//! What normal mode draws with: the hooks' types, the page geometry,
//! and the drawing helpers it shares with the app. Everything else in the
//! app stays private.

pub use crate::filing::{Keeping, WriteStatus};
pub use crate::render::feed::convex_mesh;
pub use crate::render::perspective::{Camera, warp};
pub use crate::render::platen::glide_seconds;
pub use crate::render::{HIGHLIGHT, Metrics, points_per_inch, smoothstep, splitmix64, unit};
pub use crate::stage::{Controls, FlatSheet, PaperTable, Return, Scene};

/// The scale, which the machine's bail prints, and the plates' readings, which
/// its panel shows too.
pub mod ruler {
    pub use crate::render::ruler::{
        HEIGHT, SAVED, Scale, autosave_state, correction_method, goal_reading, scale_marks,
    };
}
