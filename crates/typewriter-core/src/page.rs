//! A sheet of paper: a grid of character cells, each holding everything ever
//! struck or painted onto it.

use std::collections::BTreeMap;

/// Something applied to a cell, in order. Later marks sit on top.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mark {
    Glyph(char),
    /// Struck onto correction fluid that had not dried: the ink ran.
    Smudged(char),
    Correction(Correction),
}

/// Covers everything struck before it, which stays in the stack.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Correction {
    /// Rubbed out with a typewriter eraser. A faint ghost of the ink stays.
    Eraser,
    /// Chalk from a correction slip, pressed on in the shape of the character
    /// struck through it. Striking the wrong character again covers it.
    Chalk(char),
    /// A dab of correction fluid, `wet` until it has dried.
    Fluid { wet: bool },
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Cell {
    marks: Vec<Mark>,
}

impl Cell {
    pub fn marks(&self) -> &[Mark] {
        &self.marks
    }

    /// Glyphs not hidden under a correction, bottom first. The eraser and
    /// fluid cover the whole cell; chalk only lands in the shape of the
    /// character struck through the slip, so it hides only that character
    /// (an overstruck `'` and `.` need both struck again).
    pub fn visible_glyphs(&self) -> impl Iterator<Item = char> + '_ {
        let start = self
            .marks
            .iter()
            .rposition(|m| {
                matches!(
                    m,
                    Mark::Correction(Correction::Eraser | Correction::Fluid { .. })
                )
            })
            .map_or(0, |i| i + 1);
        let marks = &self.marks[start..];
        marks.iter().enumerate().filter_map(move |(i, m)| match m {
            Mark::Glyph(c) | Mark::Smudged(c) => {
                let chalked = marks[i + 1..].contains(&Mark::Correction(Correction::Chalk(*c)));
                (!chalked).then_some(*c)
            }
            Mark::Correction(_) => None,
        })
    }

    /// Wet fluid on top of the cell, so a strike now would smudge.
    fn is_wet(&self) -> bool {
        self.marks.iter().rev().find_map(|m| match m {
            Mark::Correction(Correction::Fluid { wet }) => Some(*wet),
            Mark::Correction(_) => Some(false),
            Mark::Glyph(_) | Mark::Smudged(_) => None,
        }) == Some(true)
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

    /// Nothing was ever struck on it (or everything was erased).
    pub fn is_blank(&self) -> bool {
        self.cells.is_empty()
    }

    pub fn cell(&self, half_line: u16, column: u16) -> Option<&Cell> {
        self.cells.get(&(half_line, column))
    }

    /// Non-empty cells in reading order: `((half_line, column), cell)`.
    pub fn cells(&self) -> impl Iterator<Item = ((u16, u16), &Cell)> {
        self.cells.iter().map(|(&pos, cell)| (pos, cell))
    }

    /// Returns false if the strike landed on wet fluid and smudged.
    pub fn strike(&mut self, half_line: u16, column: u16, glyph: char) -> bool {
        let wet = self.cell(half_line, column).is_some_and(Cell::is_wet);
        let mark = if wet {
            Mark::Smudged(glyph)
        } else {
            Mark::Glyph(glyph)
        };
        self.push(half_line, column, mark);
        !wet
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

    /// Correction fluid in the cell has dried.
    pub fn dry(&mut self, half_line: u16, column: u16) {
        if let Some(cell) = self.cells.get_mut(&(half_line, column)) {
            dry_cell(cell);
        }
    }

    pub fn dry_all(&mut self) {
        self.cells.values_mut().for_each(dry_cell);
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

fn dry_cell(cell: &mut Cell) {
    for mark in &mut cell.marks {
        if let Mark::Correction(Correction::Fluid { wet }) = mark {
            *wet = false;
        }
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
        assert!(page.cover(0, 0, Correction::Eraser));
        let cell = page.cell(0, 0).unwrap();
        assert_eq!(cell.top_glyph(), None);
        assert_eq!(cell.marks().len(), 2);

        page.strike(0, 0, 'y');
        assert_eq!(page.cell(0, 0).unwrap().top_glyph(), Some('y'));
    }

    #[test]
    fn covering_blank_paper_does_nothing() {
        let mut page = Page::new(10, 10);
        assert!(!page.cover(0, 0, Correction::Chalk('x')));
        assert!(page.cell(0, 0).is_none());
    }

    #[test]
    fn chalk_hides_only_the_character_struck_through_the_slip() {
        let mut page = Page::new(10, 10);
        // An exclamation mark the typewriter way: apostrophe over full stop.
        page.strike(0, 0, '\'');
        page.strike(0, 0, '.');
        assert!(page.cover(0, 0, Correction::Chalk('.')));
        assert_eq!(page.cell(0, 0).unwrap().top_glyph(), Some('\''));
        assert!(page.cover(0, 0, Correction::Chalk('\'')));
        assert_eq!(page.cell(0, 0).unwrap().top_glyph(), None);
        // Nothing left to whiten.
        assert!(!page.cover(0, 0, Correction::Chalk('.')));

        page.strike(0, 0, '?');
        assert_eq!(page.cell(0, 0).unwrap().top_glyph(), Some('?'));
    }

    #[test]
    fn chalk_of_another_character_hides_nothing() {
        let mut page = Page::new(10, 10);
        page.strike(0, 0, 'a');
        page.cover(0, 0, Correction::Chalk('o'));
        assert_eq!(page.cell(0, 0).unwrap().top_glyph(), Some('a'));
    }

    #[test]
    fn eraser_covers_every_overstruck_character() {
        let mut page = Page::new(10, 10);
        page.strike(0, 0, '\'');
        page.strike(0, 0, '.');
        page.cover(0, 0, Correction::Eraser);
        assert_eq!(page.cell(0, 0).unwrap().top_glyph(), None);
    }

    #[test]
    fn striking_wet_fluid_smudges_until_it_dries() {
        let mut page = Page::new(10, 10);
        page.strike(0, 0, 'x');
        page.cover(0, 0, Correction::Fluid { wet: true });
        assert!(!page.strike(0, 0, 'y'));
        assert_eq!(page.cell(0, 0).unwrap().top_glyph(), Some('y'));
        assert!(!page.strike(0, 0, 'y'), "still wet under the smudge");

        page.dry(0, 0);
        assert!(page.strike(0, 0, 'z'));
        assert_eq!(
            page.cell(0, 0).unwrap().marks().last(),
            Some(&Mark::Glyph('z'))
        );
    }

    #[test]
    fn a_later_correction_seals_wet_fluid() {
        let mut page = Page::new(10, 10);
        page.strike(0, 0, 'x');
        page.cover(0, 0, Correction::Fluid { wet: true });
        page.strike(0, 0, 'y');
        page.cover(0, 0, Correction::Eraser);
        assert!(page.strike(0, 0, 'z'));
    }

    #[test]
    fn dry_all_dries_every_cell() {
        let mut page = Page::new(10, 10);
        for column in 0..3 {
            page.strike(0, column, 'x');
            page.cover(0, column, Correction::Fluid { wet: true });
        }
        page.dry_all();
        assert!((0..3).all(|column| page.strike(0, column, 'y')));
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
