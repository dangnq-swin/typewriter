//! Typewriter mechanics, free of GUI, audio and I/O concerns.

pub mod carriage;
pub mod constraints;
pub mod document;
pub mod machine;
pub mod page;
pub mod profile;

pub use carriage::LineSpacing;
pub use constraints::{Constraints, EraseMode};
pub use document::Document;
pub use machine::{BlockReason, Command, Direction, Event, Typewriter};
pub use profile::{Profile, ProfileError};
