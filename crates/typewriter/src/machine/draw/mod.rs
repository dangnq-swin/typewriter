//! The machine drawn through `typewriter-render`: the parts, the lamp's
//! light and the sheets, gathered into the render crate's [`Scene`] instead
//! of the old egui paint callback.
//!
//! The old depth pass keeps drawing the desk until T5 switches it over, so
//! these modules build the new path beside it and change none of the old
//! one. The geometry the machine's builders already stand in is carried
//! across unchanged ([`convert`]), a placement's transform moves standing
//! parts, and the shader projects — a frame is millimetres end to end.
//!
//! [`Scene`]: typewriter_render::device::Scene

pub(crate) mod convert;

pub mod lamp;
pub mod parts;
pub mod sheets;
