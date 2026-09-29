//! Typewriter mechanics, free of GUI, audio and I/O concerns.

pub mod carriage;
pub mod constraints;
pub mod document;
pub mod export;
pub mod machine;
pub mod page;
pub mod profile;
pub mod retype;
pub mod scratchpad;
pub mod session;

pub use carriage::LineSpacing;
pub use constraints::{Constraints, EraseMode};
pub use document::{Document, FolderError};
pub use machine::{BlockReason, Command, Direction, Event, Typewriter};
pub use profile::{Profile, ProfileError};
pub use scratchpad::Scratchpad;
pub use session::{Goal, Session, SessionStats};
