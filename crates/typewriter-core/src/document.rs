//! The sheets typed so far (filed ones and the one in the machine) and the
//! folder file that saves them.

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use thiserror::Error;

use crate::carriage::Carriage;
use crate::constraints::Constraints;
use crate::notebook::Notebook;
use crate::page::{Page, Shift};
use crate::session::WritingLog;

/// The folder format this build writes. 1.0: format 8 of the numbering
/// before it, pinned.
pub const FORMAT_VERSION: FormatVersion = FormatVersion { major: 1, minor: 0 };

/// A folder format, written `"1.0"`. A minor version only adds what older
/// files lack and read with defaults; a major one needs files converted.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct FormatVersion {
    pub major: u16,
    pub minor: u16,
}

impl FormatVersion {
    /// Whether this build opens a file of this version.
    fn check(self) -> Result<(), FolderError> {
        if self > FORMAT_VERSION {
            Err(FolderError::NewerVersion(self))
        } else if self.major < FORMAT_VERSION.major {
            Err(FolderError::OlderMajor(self))
        } else {
            Ok(())
        }
    }
}

impl fmt::Display for FormatVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}", self.major, self.minor)
    }
}

impl FromStr for FormatVersion {
    type Err = String;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        let not_one = || format!("not a folder format version: {text:?}");
        let (major, minor) = text.split_once('.').ok_or_else(not_one)?;
        Ok(Self {
            major: major.parse().map_err(|_| not_one())?,
            minor: minor.parse().map_err(|_| not_one())?,
        })
    }
}

impl Serialize for FormatVersion {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for FormatVersion {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        String::deserialize(deserializer)?
            .parse()
            .map_err(serde::de::Error::custom)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Document {
    finished: Vec<Page>,
    current: Page,
    notebook: Notebook,
    /// A finished sheet rolled back in goes back here among the finished
    /// ones when fed out.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    returns_to: Option<usize>,
}

impl Document {
    pub fn new(first: Page) -> Self {
        Self {
            finished: Vec::new(),
            current: first,
            notebook: Notebook::default(),
            returns_to: None,
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

    /// Scrunches up finished sheet `index`, for good.
    pub fn remove(&mut self, index: usize) -> Option<Page> {
        if index >= self.finished.len() {
            return None;
        }
        if let Some(slot) = &mut self.returns_to
            && index < *slot
        {
            *slot -= 1;
        }
        Some(self.finished.remove(index))
    }

    /// Moves finished sheet `from` to `to`; those between shift along.
    /// False if either is out of range.
    pub fn move_sheet(&mut self, from: usize, to: usize) -> bool {
        let count = self.finished.len();
        if from >= count || to >= count {
            return false;
        }
        let page = self.finished.remove(from);
        self.finished.insert(to, page);
        // The re-fed sheet keeps its neighbour: the one it was filed before.
        if let Some(slot) = &mut self.returns_to {
            if from < *slot && to >= *slot {
                *slot -= 1;
            } else if from >= *slot && to <= *slot {
                *slot += 1;
            }
        }
        true
    }

    /// Pencils a note on finished sheet `index` (oldest 0). False if none.
    pub fn annotate(&mut self, index: usize, note: &str) -> bool {
        self.finished
            .get_mut(index)
            .map(|page| page.set_note(note))
            .is_some()
    }

    /// Where the sheet in the machine goes back among the finished ones, if
    /// it was rolled back in.
    pub fn returns_to(&self) -> Option<usize> {
        self.returns_to
    }

    pub fn notebook(&self) -> &Notebook {
        &self.notebook
    }

    pub fn notebook_mut(&mut self) -> &mut Notebook {
        &mut self.notebook
    }

    /// Files the sheet and puts `fresh` in. A blank sheet just stays in.
    pub fn feed(&mut self, fresh: Page) {
        if !self.current.is_blank() {
            let out = std::mem::replace(&mut self.current, fresh);
            self.file(out);
        }
    }

    /// Rolls finished sheet `index` back in, `shift` out of line. The sheet
    /// in the machine is filed first, or dropped if blank. False if no such
    /// sheet.
    pub fn roll_in(&mut self, index: usize, shift: Shift) -> bool {
        if index >= self.finished.len() {
            return false;
        }
        let sheet = self.finished.remove(index);
        if let Some(slot) = &mut self.returns_to
            && index < *slot
        {
            *slot -= 1;
        }
        let out = std::mem::replace(&mut self.current, sheet);
        let mut slot = index;
        if !out.is_blank() {
            // Filed ahead of the rolled-in sheet's place, that moves along.
            // Going home to the very same place, it was filed before it; a
            // new sheet filed last is newer, and goes after.
            let going_home = self.returns_to.is_some();
            let at = self.file(out);
            if at < slot || (at == slot && going_home) {
                slot += 1;
            }
        }
        self.returns_to = Some(slot);
        self.current.refeed(shift);
        true
    }

    /// Files `page` where it belongs: back in its place if it was rolled
    /// back in, else last. Returns where.
    fn file(&mut self, page: Page) -> usize {
        let at = self
            .returns_to
            .take()
            .map_or(self.finished.len(), |slot| slot.min(self.finished.len()));
        self.finished.insert(at, page);
        at
    }
}

/// A folder file (`*.typr`): the document plus the machine's state, so
/// typing resumes where it stopped.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub(crate) struct FolderFile {
    pub version: FormatVersion,
    /// Profile name, looked up on load.
    pub profile: String,
    pub constraints: Constraints,
    pub carriage: Carriage,
    pub document: Document,
    pub log: WritingLog,
}

#[derive(Debug, Error)]
pub enum FolderError {
    #[error("not a typewriter folder: {0}")]
    Unreadable(#[from] ron::error::SpannedError),
    #[error("the folder could not be written: {0}")]
    Unwritable(#[from] ron::Error),
    #[error("made by a newer version of Typewriter (folder format {0})")]
    NewerVersion(FormatVersion),
    #[error("folder format {0} needs converting to {FORMAT_VERSION} first")]
    OlderMajor(FormatVersion),
    #[error(
        "saved before folder format 1.0 (format {0}): convert a format 8 file with \
         scripts/convert-format-8.sh"
    )]
    BeforeOne(u32),
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

    /// A file this build can't open says why, whether or not it parses.
    pub fn from_ron(text: &str) -> Result<Self, FolderError> {
        // Never parse just the version: ron skips the rest of the file in
        // quadratic time, hours for a novel.
        match ron::from_str::<Self>(text) {
            Ok(file) => file.version.check().map(|()| file),
            Err(err) => Err(match leading_version(text) {
                Some(Leading::Numbered(format)) => FolderError::BeforeOne(format),
                Some(Leading::Version(version)) => match version.check() {
                    Err(refused) => refused,
                    Ok(()) => err.into(),
                },
                None => err.into(),
            }),
        }
    }
}

/// A folder file's opening version, as written.
#[derive(Debug, PartialEq, Eq)]
enum Leading {
    /// Before 1.0: `version: 8`.
    Numbered(u32),
    Version(FormatVersion),
}

/// The version from a folder file's opening `(version: …`.
fn leading_version(text: &str) -> Option<Leading> {
    let rest = text.trim_start().strip_prefix('(')?.trim_start();
    let rest = rest
        .strip_prefix("version")?
        .trim_start()
        .strip_prefix(':')?
        .trim_start();
    if let Some(quoted) = rest.strip_prefix('"') {
        let (version, _) = quoted.split_once('"')?;
        return version.parse().ok().map(Leading::Version);
    }
    let digits = rest
        .find(|c: char| !c.is_ascii_digit())
        .unwrap_or(rest.len());
    rest[..digits].parse().ok().map(Leading::Numbered)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn versions_are_written_major_dot_minor() {
        assert_eq!(FORMAT_VERSION.to_string(), "1.0");
        let v = |text: &str| text.parse::<FormatVersion>();
        assert_eq!(
            v("1.12"),
            Ok(FormatVersion {
                major: 1,
                minor: 12
            })
        );
        assert!(v("1").is_err() && v("1.x").is_err() && v("").is_err());
        assert!(v("1.10").unwrap() > v("1.9").unwrap(), "numbers, not text");
    }

    #[test]
    fn a_file_this_build_cannot_open_says_why_even_unparsed() {
        let refused = |text: &str| FolderFile::from_ron(text).err().map(|e| e.to_string());
        let newer = "(\n    version: \"1.1\",\n    sheets: [\"something new\"],\n)";
        assert_eq!(
            refused(newer).unwrap(),
            "made by a newer version of Typewriter (folder format 1.1)"
        );
        assert!(
            refused("(version: \"2.0\", x: 1)")
                .unwrap()
                .contains("format 2.0")
        );
        let old = "(\n    version: 8,\n    profile: \"Olympia SM9\",\n)";
        assert!(matches!(
            FolderFile::from_ron(old),
            Err(FolderError::BeforeOne(8))
        ));
        let broken = format!("(version: \"{FORMAT_VERSION}\", sheets: 1)");
        assert!(matches!(
            FolderFile::from_ron(&broken),
            Err(FolderError::Unreadable(_))
        ));
    }

    #[test]
    fn an_older_major_needs_converting() {
        let older = FormatVersion { major: 0, minor: 9 };
        assert!(matches!(older.check(), Err(FolderError::OlderMajor(_))));
        assert!(FORMAT_VERSION.check().is_ok());
    }

    #[test]
    fn the_opening_version_is_read_as_written() {
        assert_eq!(
            leading_version("  ( version :12, x"),
            Some(Leading::Numbered(12))
        );
        assert_eq!(
            leading_version("(\n    version: \"1.3\","),
            Some(Leading::Version(FormatVersion { major: 1, minor: 3 }))
        );
        assert_eq!(leading_version("(profile: \"x\", version: 3)"), None);
    }

    #[test]
    fn feeding_files_the_typed_sheet() {
        let mut doc = Document::new(Page::new(10, 10));
        doc.current_mut().strike(0, 0, 'a');
        doc.feed(Page::new(10, 10));
        assert_eq!(doc.finished().len(), 1);
        assert_eq!(doc.finished()[0].line_text(0), "a");
        assert!(doc.current().is_blank());
    }

    fn filed(texts: &[&str]) -> Document {
        let mut doc = Document::new(Page::new(10, 10));
        for text in texts {
            for (i, c) in text.chars().enumerate() {
                doc.current_mut().strike(0, i as u16, c);
            }
            doc.feed(Page::new(10, 10));
        }
        doc
    }

    fn order(doc: &Document) -> Vec<String> {
        doc.finished().iter().map(|p| p.line_text(0)).collect()
    }

    #[test]
    fn a_sheet_moves_and_the_others_shift_along() {
        let mut doc = filed(&["a", "b", "c", "d"]);
        assert!(doc.move_sheet(3, 1));
        assert_eq!(order(&doc), ["a", "d", "b", "c"]);
        assert!(doc.move_sheet(0, 3));
        assert_eq!(order(&doc), ["d", "b", "c", "a"]);
        assert!(!doc.move_sheet(0, 4));
    }

    #[test]
    fn a_scrunched_sheet_is_gone() {
        let mut doc = filed(&["a", "b", "c"]);
        assert_eq!(doc.remove(1).map(|p| p.line_text(0)), Some("b".to_owned()));
        assert_eq!(order(&doc), ["a", "c"]);
        assert!(doc.remove(2).is_none());
        assert!(doc.current().is_blank(), "the sheet in the machine stays");
    }

    const SHIFT: Shift = Shift {
        across: 10,
        down: 10,
    };

    #[test]
    fn a_rolled_in_sheet_goes_back_to_its_place() {
        let mut doc = filed(&["a", "b", "c"]);
        doc.current_mut().strike(0, 0, 'd');
        assert!(doc.roll_in(1, SHIFT));
        assert_eq!(doc.current().line_text(0), "b");
        assert_eq!(
            order(&doc),
            ["a", "c", "d"],
            "the sheet in the machine is filed"
        );
        doc.current_mut().strike(0, 1, '+');
        doc.feed(Page::new(10, 10));
        assert_eq!(order(&doc), ["a", "b+", "c", "d"]);
        assert!(!doc.roll_in(9, SHIFT));
    }

    #[test]
    fn rolling_in_another_files_the_first_back_in_its_place() {
        let mut doc = filed(&["a", "b", "c"]);
        assert!(doc.roll_in(2, SHIFT));
        assert!(doc.roll_in(0, SHIFT), "c goes home, a comes in");
        assert_eq!(order(&doc), ["b", "c"]);
        doc.feed(Page::new(10, 10));
        assert_eq!(order(&doc), ["a", "b", "c"]);
        assert!(doc.roll_in(0, SHIFT));
        assert!(doc.roll_in(1, SHIFT), "a goes home before b");
        doc.feed(Page::new(10, 10));
        assert_eq!(order(&doc), ["a", "b", "c"]);
        assert!(doc.roll_in(1, SHIFT));
        assert!(doc.roll_in(1, SHIFT), "b goes home to c's old place");
        doc.feed(Page::new(10, 10));
        assert_eq!(order(&doc), ["a", "b", "c"]);
    }

    #[test]
    fn scrunching_or_moving_others_keeps_the_place() {
        let mut doc = filed(&["a", "b", "c", "d"]);
        assert!(doc.roll_in(2, SHIFT));
        assert!(doc.remove(0).is_some());
        doc.feed(Page::new(10, 10));
        assert_eq!(order(&doc), ["b", "c", "d"]);
        assert!(doc.roll_in(1, SHIFT));
        assert!(doc.move_sheet(0, 1), "b moves after d");
        doc.feed(Page::new(10, 10));
        assert_eq!(order(&doc), ["c", "d", "b"], "c still before d");
    }

    #[test]
    fn feeding_over_a_blank_sheet_keeps_nothing() {
        let mut doc = Document::new(Page::new(10, 10));
        doc.feed(Page::new(10, 10));
        assert!(doc.finished().is_empty());
    }
}
