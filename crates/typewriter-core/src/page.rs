//! A sheet of paper: a grid of character cells, each holding everything ever
//! struck or painted onto it.

use std::collections::BTreeMap;

/// Something applied to a cell, in order. Later marks sit on top.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mark {
    Glyph(char),
    Correction(Correction),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Correction {
    WhiteOut,
    CorrectionTape,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Cell {
    marks: Vec<Mark>,
}

impl Cell {
    pub fn marks(&self) -> &[Mark] {
        &self.marks
    }

    /// Glyphs not hidden under a correction, bottom first.
    pub fn visible_glyphs(&self) -> impl Iterator<Item = char> + '_ {
        let start = self
            .marks
            .iter()
            .rposition(|m| matches!(m, Mark::Correction(_)))
            .map_or(0, |i| i + 1);
        self.marks[start..].iter().filter_map(|m| match m {
            Mark::Glyph(c) => Some(*c),
            Mark::Correction(_) => None,
        })
    }

    pub fn top_glyph(&self) -> Option<char> {
        self.visible_glyphs().last()
    }
}

/// Rows are addressed in half-line steps, matching the platen ratchet.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Page {
    columns: u16,
    half_lines: u16,
    cells: BTreeMap<(u16, u16), Cell>,
}

impl Page {
    pub fn new(columns: u16, half_lines: u16) -> Self {
        Self {
            columns,
            half_lines,
            cells: BTreeMap::new(),
        }
    }

    pub fn columns(&self) -> u16 {
        self.columns
    }

    pub fn half_lines(&self) -> u16 {
        self.half_lines
    }

    pub fn cell(&self, half_line: u16, column: u16) -> Option<&Cell> {
        self.cells.get(&(half_line, column))
    }

    /// Non-empty cells in reading order: `((half_line, column), cell)`.
    pub fn cells(&self) -> impl Iterator<Item = ((u16, u16), &Cell)> {
        self.cells.iter().map(|(&pos, cell)| (pos, cell))
    }

    pub fn strike(&mut self, half_line: u16, column: u16, glyph: char) {
        self.push(half_line, column, Mark::Glyph(glyph));
    }

    /// Covers the cell's glyphs. Returns false (and does nothing) if there was
    /// nothing visible to cover.
    pub fn cover(&mut self, half_line: u16, column: u16, correction: Correction) -> bool {
        let has_visible = self
            .cell(half_line, column)
            .is_some_and(|c| c.top_glyph().is_some());
        if has_visible {
            self.push(half_line, column, Mark::Correction(correction));
        }
        has_visible
    }

    pub fn erase(&mut self, half_line: u16, column: u16) {
        self.cells.remove(&(half_line, column));
    }

    /// Top visible glyph per column for one half-line, blanks as spaces and
    /// trailing blanks trimmed.
    pub fn line_text(&self, half_line: u16) -> String {
        let mut text = String::new();
        for column in 0..self.columns {
            let glyph = self.cell(half_line, column).and_then(Cell::top_glyph);
            text.push(glyph.unwrap_or(' '));
        }
        text.truncate(text.trim_end().len());
        text
    }

    fn push(&mut self, half_line: u16, column: u16, mark: Mark) {
        debug_assert!(half_line < self.half_lines && column < self.columns);
        self.cells
            .entry((half_line, column))
            .or_default()
            .marks
            .push(mark);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn overtyping_stacks_glyphs() {
        let mut page = Page::new(10, 10);
        page.strike(0, 0, 'a');
        page.strike(0, 0, '-');
        let cell = page.cell(0, 0).unwrap();
        assert_eq!(cell.visible_glyphs().collect::<String>(), "a-");
        assert_eq!(cell.top_glyph(), Some('-'));
    }

    #[test]
    fn correction_hides_glyphs_below_but_keeps_them() {
        let mut page = Page::new(10, 10);
        page.strike(0, 0, 'x');
        assert!(page.cover(0, 0, Correction::WhiteOut));
        let cell = page.cell(0, 0).unwrap();
        assert_eq!(cell.top_glyph(), None);
        assert_eq!(cell.marks().len(), 2);

        page.strike(0, 0, 'y');
        assert_eq!(page.cell(0, 0).unwrap().top_glyph(), Some('y'));
    }

    #[test]
    fn covering_blank_paper_does_nothing() {
        let mut page = Page::new(10, 10);
        assert!(!page.cover(0, 0, Correction::CorrectionTape));
        assert!(page.cell(0, 0).is_none());
    }

    #[test]
    fn erase_removes_the_cell() {
        let mut page = Page::new(10, 10);
        page.strike(0, 0, 'x');
        page.erase(0, 0);
        assert!(page.cell(0, 0).is_none());
    }

    #[test]
    fn line_text_fills_gaps_and_trims() {
        let mut page = Page::new(10, 10);
        page.strike(2, 1, 'h');
        page.strike(2, 3, 'i');
        assert_eq!(page.line_text(2), " h i");
        assert_eq!(page.line_text(0), "");
    }
}
