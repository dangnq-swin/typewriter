//! Plain text and Markdown exports. Only what reads on the sheets: corrected
//! letters are left out.

use crate::document::Document;
use crate::page::Page;

/// Plain text: a form feed between sheets, each note first in brackets.
pub fn plain_text(document: &Document) -> String {
    let sheets: Vec<String> = sheets(document)
        .map(|page| {
            let text = lines(page).join("\n");
            match page.note() {
                "" => text,
                note => format!("[{note}]\n\n{text}"),
            }
        })
        .collect();
    finish(sheets.join("\n\u{c}\n"))
}

/// Markdown: `---` between sheets, each note first as a quote. Lines run on
/// into paragraphs; an indent starts a new one (never code: indents are cut).
pub fn markdown(document: &Document) -> String {
    let sheets: Vec<String> = sheets(document)
        .map(|page| {
            let mut out: Vec<String> = Vec::new();
            if !page.note().is_empty() {
                out.extend(
                    page.note()
                        .lines()
                        .map(|line| format!("> {line}").trim_end().to_owned()),
                );
                out.push(String::new());
            }
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

/// The filed sheets, then the one in the machine unless blank.
pub fn sheets(document: &Document) -> impl Iterator<Item = &Page> {
    let current = document.current();
    document
        .finished()
        .iter()
        .chain((!current.is_blank()).then_some(current))
}

/// Typed lines, a blank per empty line of space between, margin trimmed.
fn lines(page: &Page) -> Vec<String> {
    let rows: Vec<(u16, String)> = (0..page.half_lines())
        .map(|half_line| (half_line, page.line_text(half_line)))
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
            // Two half-lines per line: 1.5 spacing adds no blank.
            let blanks = ((half_line - previous) / 2).saturating_sub(1);
            out.extend((0..blanks).map(|_| String::new()));
        }
        // Safe to slice: the margin is ASCII spaces.
        out.push(text[margin..].to_owned());
        previous = Some(half_line);
    }
    out
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
    fn a_note_comes_before_its_sheet() {
        let mut doc = document(vec![page(&[(0, 0, "Once")]), page(&[(0, 0, "twice")])]);
        doc.annotate(0, "too short?\nexpand");
        assert_eq!(
            plain_text(&doc),
            "[too short?\nexpand]\n\nOnce\n\u{c}\ntwice\n"
        );
        assert_eq!(
            markdown(&doc),
            "> too short?\n> expand\n\nOnce\n\n---\n\ntwice\n"
        );
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
