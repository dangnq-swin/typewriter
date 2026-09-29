//! A sheet: a grid of cells, each keeping every mark ever made on it.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::accents;

/// Something applied to a cell, in order. Later marks sit on top.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Mark {
    Glyph(char),
    /// Struck on wet fluid: the ink ran.
    Smudged(char),
    Correction(Correction),
}

/// Covers what was struck before it; that stays in the stack.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Correction {
    /// Rubbed out. A faint ghost of the ink stays.
    Eraser,
    /// Slip chalk in the shape of the character struck through it. Hides only
    /// that character.
    Chalk(char),
    /// A fluid dab, `wet` until dry.
    Fluid { wet: bool },
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Cell {
    marks: Vec<Mark>,
}

impl Cell {
    pub fn marks(&self) -> &[Mark] {
        &self.marks
    }

    /// Glyphs not hidden by a correction, bottom first.
    ///
    /// Eraser and fluid hide the whole cell; chalk hides only its own
    /// character (an overstruck `'` and `.` need both struck again).
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

    /// Wet fluid on top: a strike now smudges.
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

    /// What the cell reads as: its top glyph, except `'` over `.` reads `!`
    /// and an accent over a letter reads as the accented letter.
    pub fn reads_as(&self) -> Option<char> {
        let visible: Vec<char> = self.visible_glyphs().collect();
        if visible.contains(&'\'') && visible.contains(&'.') {
            return Some('!');
        }
        let (dead, letters): (Vec<char>, Vec<char>) =
            visible.iter().partition(|&&c| accents::is_accent(c));
        if let ([accent], [letter]) = (dead.as_slice(), letters.as_slice())
            && let Some(accented) = accents::compose(*letter, *accent)
        {
            return Some(accented);
        }
        visible.last().copied()
    }
}

/// Where a re-fed sheet sits against its first feeding: never quite back in
/// line. Hundredths of a column across and of a half-line down.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Shift {
    pub across: i8,
    pub down: i8,
}

/// Rows are half-line steps, like the platen ratchet.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(into = "PageFile", from = "PageFile")]
pub struct Page {
    columns: u16,
    half_lines: u16,
    cells: BTreeMap<(u16, u16), Cell>,
    /// Pencilled in the top margin once filed, one text line per written
    /// line.
    note: String,
    /// Each feeding after the first, in order.
    refeeds: Vec<Shift>,
    /// The feeding each mark was made in (0: the first), for cells struck
    /// since a re-feed.
    fed: BTreeMap<(u16, u16), Vec<u8>>,
}

/// A sheet in a folder file. Nearly every cell is one plain letter: those
/// are kept as each line's text, and only the rest as full stacks.
#[derive(Serialize, Deserialize)]
struct PageFile {
    columns: u16,
    half_lines: u16,
    /// Every cell, before format 8. Read, never written.
    #[serde(default, skip_serializing)]
    cells: BTreeMap<(u16, u16), Cell>,
    /// Half-line → (first column, text); a space is a blank cell.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    lines: BTreeMap<u16, (u16, String)>,
    /// Cells holding more than one plain letter: overstrikes, corrections.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    stacks: BTreeMap<(u16, u16), Cell>,
    /// Absent before format 3.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    note: String,
    /// Absent before format 6.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    refeeds: Vec<Shift>,
    /// Absent before format 6.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    fed: BTreeMap<(u16, u16), Vec<u8>>,
}

impl From<Page> for PageFile {
    fn from(page: Page) -> Self {
        let mut lines: BTreeMap<u16, (u16, String)> = BTreeMap::new();
        let mut stacks = BTreeMap::new();
        for ((half_line, column), cell) in page.cells {
            let letter = match cell.marks.as_slice() {
                [Mark::Glyph(c)] if *c != ' ' => Some(*c),
                _ => None,
            };
            let Some(letter) = letter else {
                stacks.insert((half_line, column), cell);
                continue;
            };
            // Cells come in order: pad the gap since the last letter.
            let (first, text) = lines
                .entry(half_line)
                .or_insert_with(|| (column, String::new()));
            let written = u16::try_from(text.chars().count()).unwrap_or(u16::MAX);
            let gap = column.saturating_sub(first.saturating_add(written));
            text.extend(std::iter::repeat_n(' ', usize::from(gap)));
            text.push(letter);
        }
        Self {
            columns: page.columns,
            half_lines: page.half_lines,
            cells: BTreeMap::new(),
            lines,
            stacks,
            note: page.note,
            refeeds: page.refeeds,
            fed: page.fed,
        }
    }
}

impl From<PageFile> for Page {
    fn from(file: PageFile) -> Self {
        let mut cells = file.cells;
        for (half_line, (first, text)) in file.lines {
            // Bounded: a damaged file can't run the column past the end.
            for (column, letter) in (first..=u16::MAX).zip(text.chars()) {
                if letter != ' ' {
                    cells.insert(
                        (half_line, column),
                        Cell {
                            marks: vec![Mark::Glyph(letter)],
                        },
                    );
                }
            }
        }
        cells.extend(file.stacks);
        Self {
            columns: file.columns,
            half_lines: file.half_lines,
            cells,
            note: file.note,
            refeeds: file.refeeds,
            fed: file.fed,
        }
    }
}

impl Page {
    pub fn new(columns: u16, half_lines: u16) -> Self {
        Self {
            columns,
            half_lines,
            cells: BTreeMap::new(),
            note: String::new(),
            refeeds: Vec::new(),
            fed: BTreeMap::new(),
        }
    }

    /// Back in the machine, a little out of line: marks from now on sit
    /// `shift` off. After 255 feedings they share the last one's.
    pub fn refeed(&mut self, shift: Shift) {
        if self.refeeds.len() < usize::from(u8::MAX) {
            self.refeeds.push(shift);
        }
    }

    /// How far the cell's `index`th mark sits off, by the feeding it was
    /// made in.
    pub fn shift(&self, half_line: u16, column: u16, index: usize) -> Shift {
        let feeding = self
            .fed
            .get(&(half_line, column))
            .and_then(|feedings| feedings.get(index))
            .copied()
            .unwrap_or(0);
        feeding
            .checked_sub(1)
            .and_then(|i| self.refeeds.get(usize::from(i)))
            .copied()
            .unwrap_or_default()
    }

    /// The note, `""` if none.
    pub fn note(&self) -> &str {
        &self.note
    }

    /// Replaces the note. Drops trailing spaces and trailing blank lines.
    pub fn set_note(&mut self, note: &str) {
        let lines: Vec<&str> = note.lines().map(str::trim_end).collect();
        let written = lines
            .iter()
            .rposition(|l| !l.is_empty())
            .map_or(0, |i| i + 1);
        self.note = lines[..written].join("\n");
    }

    pub fn columns(&self) -> u16 {
        self.columns
    }

    pub fn half_lines(&self) -> u16 {
        self.half_lines
    }

    /// Has this size and nothing outside it. Check after loading.
    pub fn fits(&self, columns: u16, half_lines: u16) -> bool {
        self.columns == columns
            && self.half_lines == half_lines
            && self.cells.iter().all(|(&(half_line, column), cell)| {
                half_line < half_lines && column < columns && !cell.marks.is_empty()
            })
    }

    /// Nothing was ever struck on it.
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

    /// Covers the cell's glyphs. Returns false, changing nothing, if none show.
    pub fn cover(&mut self, half_line: u16, column: u16, correction: Correction) -> bool {
        let has_visible = self
            .cell(half_line, column)
            .is_some_and(|c| c.top_glyph().is_some());
        if has_visible {
            self.push(half_line, column, Mark::Correction(correction));
        }
        has_visible
    }

    /// Empties the cell: strikes and corrections alike.
    pub fn clear(&mut self, half_line: u16, column: u16) {
        self.cells.remove(&(half_line, column));
        self.fed.remove(&(half_line, column));
    }

    /// Marks the cell's fluid dry.
    pub fn dry(&mut self, half_line: u16, column: u16) {
        if let Some(cell) = self.cells.get_mut(&(half_line, column)) {
            dry_cell(cell);
        }
    }

    pub fn dry_all(&mut self) {
        self.cells.values_mut().for_each(dry_cell);
    }

    /// One half-line as read ([`Cell::reads_as`]), blanks as spaces, trailing
    /// blanks trimmed.
    pub fn line_text(&self, half_line: u16) -> String {
        let mut text: String = (0..self.columns)
            .map(|column| {
                self.cell(half_line, column)
                    .and_then(Cell::reads_as)
                    .unwrap_or(' ')
            })
            .collect();
        text.truncate(text.trim_end().len());
        text
    }

    fn push(&mut self, half_line: u16, column: u16, mark: Mark) {
        debug_assert!(half_line < self.half_lines && column < self.columns);
        let marks = &mut self.cells.entry((half_line, column)).or_default().marks;
        marks.push(mark);
        // Safe cast: `refeed` stops at 255.
        let feeding = self.refeeds.len() as u8;
        if feeding > 0 {
            let fed = self.fed.entry((half_line, column)).or_default();
            fed.resize(marks.len() - 1, 0);
            fed.push(feeding);
        }
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
    fn marks_after_a_refeed_sit_off_by_its_shift() {
        let mut page = Page::new(10, 10);
        page.strike(0, 0, 'a');
        let first = Shift {
            across: 20,
            down: -10,
        };
        page.refeed(first);
        page.strike(0, 0, 'b');
        page.strike(0, 1, 'c');
        let second = Shift {
            across: -5,
            down: 30,
        };
        page.refeed(second);
        page.strike(0, 1, 'd');
        assert_eq!(page.shift(0, 0, 0), Shift::default());
        assert_eq!(page.shift(0, 0, 1), first);
        assert_eq!(page.shift(0, 1, 0), first);
        assert_eq!(page.shift(0, 1, 1), second);
        page.clear(0, 1);
        page.strike(0, 1, 'e');
        assert_eq!(page.shift(0, 1, 0), second);
    }

    #[test]
    fn plain_letters_save_as_line_text_and_the_rest_as_stacks() {
        let mut page = Page::new(40, 10);
        for (column, c) in (3..).zip("Say \"hi\\\" now".chars()) {
            if c != ' ' {
                page.strike(2, column, c);
            }
        }
        page.strike(4, 5, '\'');
        page.strike(4, 5, '.');
        page.strike(4, 6, 'x');
        page.cover(4, 7, Correction::Eraser);
        page.strike(6, 0, 'é');
        page.set_note("check");
        page.refeed(Shift {
            across: 3,
            down: -2,
        });
        page.strike(8, 1, 'z');

        let text = ron::to_string(&page).unwrap();
        assert!(text.contains(r#"2:(3,"Say \"hi\\\" now")"#), "{text}");
        assert!(!text.contains("cells"), "{text}");
        assert!(text.contains("Glyph('.')"), "{text}");
        let back: Page = ron::from_str(&text).unwrap();
        assert_eq!(back, page);
    }

    #[test]
    fn a_line_running_off_the_end_stops_there() {
        let page: Page =
            ron::from_str(r#"(columns: 10, half_lines: 10, lines: {0: (65535, "abc")})"#).unwrap();
        assert_eq!(page.cells().count(), 1);
        assert!(!page.fits(10, 10));
    }

    #[test]
    fn a_sheet_from_before_line_text_opens() {
        let old = "(columns: 10, half_lines: 10, cells: {(0, 1): [Glyph('a')], \
                   (0, 2): [Glyph('b'), Correction(Eraser)]})";
        let page: Page = ron::from_str(old).unwrap();
        assert_eq!(page.cell(0, 1).unwrap().marks(), [Mark::Glyph('a')]);
        assert_eq!(page.cell(0, 2).unwrap().marks().len(), 2);
        let text = ron::to_string(&page).unwrap();
        assert!(text.contains(r#"lines:{0:(1,"a")}"#), "{text}");
    }

    #[test]
    fn a_sheet_never_refed_saves_as_before() {
        let mut page = Page::new(10, 10);
        page.strike(0, 0, 'a');
        let text = ron::to_string(&page).unwrap();
        assert!(!text.contains("refeeds") && !text.contains("fed"), "{text}");
    }

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
    fn a_note_keeps_its_lines_without_trailing_blanks() {
        let mut page = Page::new(10, 10);
        page.set_note("Rewrite this  \n\nbetter\n\n");
        assert_eq!(page.note(), "Rewrite this\n\nbetter");
        page.set_note(" \n");
        assert_eq!(page.note(), "");
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
