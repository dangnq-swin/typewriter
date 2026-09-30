//! The notebook: a 48-page stapled memo book beside the machine.

use serde::{Deserialize, Serialize};

pub const PAGES: usize = 48;
/// Page 1 alone, then two pages to a spread, page 48 facing the back cover.
pub const SPREADS: usize = PAGES / 2 + 1;
/// Page 1 of a new book: why the 1 / ! key opens it.
pub const START: &str = "Field notes:\nThere's no 1 or ! on a typewriter";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Notebook {
    /// From page 1. Trailing blank pages are left out.
    pages: Vec<String>,
    /// Where the book lies open.
    spread: usize,
}

impl Default for Notebook {
    fn default() -> Self {
        Self {
            pages: vec![START.to_owned()],
            spread: 0,
        }
    }
}

impl Notebook {
    /// Page `index` (0 is page 1). Blank past the last written.
    pub fn page(&self, index: usize) -> &str {
        self.pages.get(index).map_or("", String::as_str)
    }

    /// Writes page `index`. False if unchanged or no such page.
    pub fn write(&mut self, index: usize, text: &str) -> bool {
        if index >= PAGES || self.page(index) == text {
            return false;
        }
        if self.pages.len() <= index {
            self.pages.resize(index + 1, String::new());
        }
        text.clone_into(&mut self.pages[index]);
        while self.pages.last().is_some_and(String::is_empty) {
            self.pages.pop();
        }
        true
    }

    /// Only page 1's greeting, as in a new book.
    pub fn is_fresh(&self) -> bool {
        self.pages == Self::default().pages
    }

    pub fn spread(&self) -> usize {
        self.spread.min(SPREADS - 1)
    }

    /// Opens the book at `spread`, clamped. False if already there.
    pub fn open_at(&mut self, spread: usize) -> bool {
        let spread = spread.min(SPREADS - 1);
        let changed = self.spread() != spread;
        self.spread = spread;
        changed
    }
}

/// The pages on `spread`'s left and right: `None` is the inside of a cover.
pub fn pages_of(spread: usize) -> [Option<usize>; 2] {
    if spread == 0 {
        return [None, Some(0)];
    }
    let left = 2 * spread - 1;
    [Some(left), (left + 1 < PAGES).then_some(left + 1)]
}

/// The spread page `index` lies on.
pub fn spread_of(index: usize) -> usize {
    index.div_ceil(2)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn page_one_faces_the_front_cover_and_page_48_the_back() {
        assert_eq!(pages_of(0), [None, Some(0)]);
        assert_eq!(pages_of(1), [Some(1), Some(2)]);
        assert_eq!(pages_of(SPREADS - 1), [Some(PAGES - 1), None]);
        for page in 0..PAGES {
            assert!(pages_of(spread_of(page)).contains(&Some(page)));
        }
    }

    #[test]
    fn blank_pages_at_the_end_are_not_kept() {
        let mut pad = Notebook::default();
        assert!(pad.write(5, "later"));
        assert_eq!(pad.pages.len(), 6);
        assert!(pad.write(5, ""));
        assert_eq!(pad.pages.len(), 1);
        assert_eq!(pad.page(40), "");
        assert!(!pad.write(PAGES, "no such page"));
    }

    #[test]
    fn a_new_book_is_fresh_wherever_it_lies_open() {
        let mut pad = Notebook::default();
        assert!(pad.open_at(7));
        assert!(pad.is_fresh());
        assert!(pad.write(0, "mine now"));
        assert!(!pad.is_fresh());
        assert!(pad.open_at(99));
        assert_eq!(pad.spread(), SPREADS - 1);
    }
}
