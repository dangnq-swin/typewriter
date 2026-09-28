//! The sheets typed so far: a folder of finished ones and the one in the
//! machine.

use crate::page::Page;

#[derive(Debug, Clone, PartialEq, Eq)]
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
