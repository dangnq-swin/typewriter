//! Plain text and Markdown copies of a document, keeping only what reads on
//! the sheets: corrected letters are left out.

use crate::document::Document;
use crate::page::{Cell, Page};

/// Plain text, with a form feed between sheets as a printer would take it.
pub fn plain_text(document: &Document) -> String {
    let sheets: Vec<String> = sheets(document)
        .map(|page| lines(page).join("\n"))
        .collect();
    finish(sheets.join("\n\u{c}\n"))
}

/// Markdown, with `---` between sheets. Typed lines run on into paragraphs;
/// an indented line starts a new one, since Markdown would take a deep
/// indent for code.
pub fn markdown(document: &Document) -> String {
    let sheets: Vec<String> = sheets(document)
        .map(|page| {
            let mut out: Vec<String> = Vec::new();
            for line in lines(page) {
                let text = line.trim_start();
                let indented = text.len() < line.len();
                if indented && out.last().is_some_and(|l| !l.is_empty()) {
                    out.push(String::new());
                }
                out.push(text.to_owned());
            }
            out.join("\n")
        })
        .collect();
    finish(sheets.join("\n\n---\n\n"))
}

fn finish(mut text: String) -> String {
    if !text.is_empty() {
        text.push('\n');
    }
    text
}

/// The filed sheets, then the one in the machine unless it is blank.
pub fn sheets(document: &Document) -> impl Iterator<Item = &Page> {
    let current = document.current();
    document
        .finished()
        .iter()
        .chain((!current.is_blank()).then_some(current))
}

/// A sheet's typed lines, with a blank line for every empty line's worth of
/// space between them, and the left margin trimmed.
fn lines(page: &Page) -> Vec<String> {
    let rows: Vec<(u16, String)> = (0..page.half_lines())
        .map(|half_line| (half_line, row_text(page, half_line)))
        .filter(|(_, text)| !text.is_empty())
        .collect();
    let margin = rows
        .iter()
        .map(|(_, text)| text.len() - text.trim_start().len())
        .min()
        .unwrap_or(0);
    let mut out = Vec::new();
    let mut previous: Option<u16> = None;
    for (half_line, text) in rows {
        if let Some(previous) = previous {
            // A line is two half-lines, so 1.5 spacing adds no blank line.
            let blanks = ((half_line - previous) / 2).saturating_sub(1);
            out.extend((0..blanks).map(|_| String::new()));
        }
        // The margin is spaces only, so it is whole characters.
        out.push(text[margin..].to_owned());
        previous = Some(half_line);
    }
    out
}

fn row_text(page: &Page, half_line: u16) -> String {
    let mut text: String = (0..page.columns())
        .map(|column| {
            page.cell(half_line, column)
                .and_then(cell_char)
                .unwrap_or(' ')
        })
        .collect();
    text.truncate(text.trim_end().len());
    text
}

/// What a cell reads as. Overstrikes read as their top glyph, except the
/// typewriter's exclamation mark: an apostrophe over a full stop.
fn cell_char(cell: &Cell) -> Option<char> {
    let visible: Vec<char> = cell.visible_glyphs().collect();
    if visible.contains(&'\'') && visible.contains(&'.') {
        return Some('!');
    }
    visible.last().copied()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::page::Correction;

    fn page(lines: &[(u16, u16, &str)]) -> Page {
        let mut page = Page::new(40, 20);
        for &(half_line, column, text) in lines {
            for (i, c) in text.chars().enumerate() {
                if c != ' ' {
                    page.strike(half_line, column + i as u16, c);
                }
            }
        }
        page
    }

    fn document(pages: Vec<Page>) -> Document {
        let mut pages = pages.into_iter();
        let mut document = Document::new(pages.next().unwrap());
        for page in pages {
            document.feed(page);
        }
        document
    }

    #[test]
    fn lines_keep_their_spacing_and_lose_the_margin() {
        let doc = document(vec![page(&[
            (2, 5, "One"),
            (4, 5, "two"),
            (8, 10, "three"),
            (11, 5, "half"),
        ])]);
        assert_eq!(plain_text(&doc), "One\ntwo\n\n     three\nhalf\n");
    }

    #[test]
    fn sheets_are_separated_and_a_blank_sheet_in_the_machine_is_left_out() {
        let mut doc = document(vec![page(&[(0, 0, "a")]), page(&[(0, 0, "b")])]);
        assert_eq!(plain_text(&doc), "a\n\u{c}\nb\n");
        doc.feed(Page::new(40, 20));
        assert_eq!(markdown(&doc), "a\n\n---\n\nb\n");
        assert_eq!(plain_text(&document(vec![Page::new(40, 20)])), "");
    }

    #[test]
    fn corrections_are_left_out_and_overstrikes_read_as_one() {
        let mut p = page(&[(0, 0, "cat")]);
        p.cover(0, 1, Correction::Chalk('a'));
        p.strike(0, 1, 'u');
        p.strike(0, 3, '\'');
        p.strike(0, 3, '.');
        p.strike(0, 4, 'x');
        p.cover(0, 4, Correction::Eraser);
        assert_eq!(plain_text(&document(vec![p])), "cut!\n");
    }

    #[test]
    fn markdown_starts_a_paragraph_at_an_indent() {
        let doc = document(vec![page(&[
            (0, 10, "It was a dark"),
            (2, 5, "and stormy night."),
            (4, 10, "Suddenly,"),
            (8, 5, "a shot rang out."),
        ])]);
        assert_eq!(
            markdown(&doc),
            "It was a dark\nand stormy night.\n\nSuddenly,\n\na shot rang out.\n"
        );
    }
}
