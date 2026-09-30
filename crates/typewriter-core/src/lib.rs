//! Typewriter mechanics, free of GUI, audio and I/O concerns.

pub mod accents;
pub mod carriage;
pub mod constraints;
pub mod document;
pub mod export;
pub mod machine;
pub mod notebook;
pub mod page;
pub mod profile;
pub mod retype;
pub mod session;

pub use carriage::LineSpacing;
pub use constraints::{Constraints, EraseMode};
pub use document::{Document, FolderError};
pub use machine::{BlockReason, Command, Direction, Event, Side, Typewriter};
pub use notebook::Notebook;
pub use profile::{Profile, ProfileError};
pub use session::{Goal, Session, WritingLog};
