//! The sheets typed so far: a folder of finished ones and the one in the
//! machine, and the folder file they are saved in.

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::carriage::Carriage;
use crate::constraints::Constraints;
use crate::page::Page;

/// Bumped whenever the folder file changes in a way older versions cannot
/// read.
pub const FORMAT_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Document {
    finished: Vec<Page>,
    current: Page,
}

impl Document {
    pub fn new(first: Page) -> Self {
        Self {
            finished: Vec::new(),
            current: first,
        }
    }

    /// Sheets taken out of the machine, oldest first.
    pub fn finished(&self) -> &[Page] {
        &self.finished
    }

    /// The sheet in the machine.
    pub fn current(&self) -> &Page {
        &self.current
    }

    pub fn current_mut(&mut self) -> &mut Page {
        &mut self.current
    }

    /// Takes the sheet out and puts `fresh` in. A blank sheet is not worth
    /// keeping, so it simply stays in the machine.
    pub fn feed(&mut self, fresh: Page) {
        if !self.current.is_blank() {
            self.finished
                .push(std::mem::replace(&mut self.current, fresh));
        }
    }
}

/// A folder file (`*.folder.ron`): the document and the state of the
/// machine it is in, so typing carries on where it stopped.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub(crate) struct FolderFile {
    pub version: u32,
    /// The profile's name. Only the built-in machines exist for now.
    pub profile: String,
    pub constraints: Constraints,
    pub carriage: Carriage,
    pub document: Document,
}

#[derive(Debug, Error)]
pub enum FolderError {
    #[error("not a typewriter folder: {0}")]
    Unreadable(#[from] ron::error::SpannedError),
    #[error("the folder could not be written: {0}")]
    Unwritable(#[from] ron::Error),
    #[error("made by a newer version of typewriter (folder format {0})")]
    NewerVersion(u32),
    #[error("typed on a machine this version does not have: {0}")]
    UnknownMachine(String),
    #[error("its sheets do not fit the {0}")]
    DoesNotFit(String),
}

impl FolderFile {
    pub fn to_ron(&self) -> Result<String, FolderError> {
        let pretty = ron::ser::PrettyConfig::new()
            .struct_names(false)
            .compact_arrays(true);
        Ok(ron::ser::to_string_pretty(self, pretty)?)
    }

    pub fn from_ron(text: &str) -> Result<Self, FolderError> {
        /// Read first, so a newer file gets a clear error instead of a
        /// confusing one about a field this version does not know.
        #[derive(Deserialize)]
        struct Version {
            version: u32,
        }
        let Version { version } = ron::from_str(text)?;
        if version > FORMAT_VERSION {
            return Err(FolderError::NewerVersion(version));
        }
        Ok(ron::from_str(text)?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn feeding_files_the_typed_sheet() {
        let mut doc = Document::new(Page::new(10, 10));
        doc.current_mut().strike(0, 0, 'a');
        doc.feed(Page::new(10, 10));
        assert_eq!(doc.finished().len(), 1);
        assert_eq!(doc.finished()[0].line_text(0), "a");
        assert!(doc.current().is_blank());
    }

    #[test]
    fn feeding_over_a_blank_sheet_keeps_nothing() {
        let mut doc = Document::new(Page::new(10, 10));
        doc.feed(Page::new(10, 10));
        assert!(doc.finished().is_empty());
    }
}
