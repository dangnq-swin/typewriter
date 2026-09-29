//! A typist retyping a manuscript: plain paragraphs typed out on the machine.

use crate::accents;
use crate::machine::{Command, Event, Typewriter};

/// Keys the machine lacks, and what a typist strikes instead.
const SUBSTITUTES: [(char, &[Command]); 2] = [
    ('1', &[Command::Type('l')]),
    // Reads as `!`: see `Cell::reads_as`.
    (
        '!',
        &[Command::Type('\''), Command::Backspace, Command::Type('.')],
    ),
];

/// Types `paragraphs` from where the carriage stands: word-wrapped at the
/// margins, a blank line between paragraphs, a fresh sheet on reaching a
/// bottom margin as deep as the top one. A `\n` in a paragraph breaks the
/// line; blank paragraphs are left out. `!` needs backspace allowed.
/// Accented letters are kept even where the machine has no dead key.
pub fn retype(machine: &mut Typewriter, paragraphs: &[impl AsRef<str>]) {
    let carriage = machine.carriage();
    let width = usize::from(carriage.right_margin.saturating_sub(carriage.left_margin)).max(1);
    let mut typist = Typist {
        machine,
        typed: false,
    };
    for paragraph in paragraphs.iter().map(AsRef::as_ref) {
        if paragraph.trim().is_empty() {
            continue;
        }
        let mut returns = 2;
        for line in paragraph.lines().flat_map(|line| wrap(line, width)) {
            typist.type_line(&line, returns);
            returns = 1;
        }
    }
}

struct Typist<'a> {
    machine: &'a mut Typewriter,
    /// Anything typed yet: the first line needs no return before it.
    typed: bool,
}

impl Typist<'_> {
    /// Types `line` after `returns` throws of the lever. Past the bottom
    /// margin, feeds a sheet instead: a new sheet starts with no blank line.
    fn type_line(&mut self, line: &str, returns: usize) {
        if std::mem::replace(&mut self.typed, true) {
            for _ in 0..returns {
                let events = self.machine.apply(Command::Return);
                if events.contains(&Event::PageEnd) || self.past_bottom() {
                    self.machine.apply(Command::FeedSheet);
                    break;
                }
            }
        }
        if self.past_bottom() {
            self.machine.apply(Command::FeedSheet);
        }
        for c in line.chars().filter(|c| !c.is_control()) {
            match SUBSTITUTES.iter().find(|(missing, _)| *missing == c) {
                Some((_, strokes)) => strokes.iter().for_each(|&stroke| {
                    self.machine.apply(stroke);
                }),
                None => {
                    // No dead key for it: keep the manuscript's letter.
                    if self.machine.apply(Command::Type(c)).is_empty() && accents::is_accented(c) {
                        self.machine.strike(c);
                    }
                }
            }
        }
    }

    fn past_bottom(&self) -> bool {
        let margin = self.machine.profile().margins.top_lines * 2;
        self.machine.carriage().half_line + margin >= self.machine.page().half_lines()
    }
}

/// `line` in lines of at most `width` characters, broken at spaces; a word
/// longer than a line is cut. A blank line stays one blank line.
fn wrap(line: &str, width: usize) -> Vec<String> {
    let mut lines = vec![String::new()];
    for word in line.split_whitespace() {
        let word: Vec<char> = word.chars().collect();
        for piece in word.chunks(width) {
            let current = lines.last_mut().map_or(0, |l| l.chars().count());
            let fits = current == 0 || current + 1 + piece.len() <= width;
            if !fits {
                lines.push(String::new());
            }
            if let Some(last) = lines.last_mut() {
                if !last.is_empty() {
                    last.push(' ');
                }
                last.extend(piece);
            }
        }
    }
    lines
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;
    use crate::{Constraints, Profile, export};

    fn sm9() -> Typewriter {
        let profile =
            Profile::from_toml_str(include_str!("../../../profiles/olympia-sm9.toml")).unwrap();
        Typewriter::new(profile, Constraints::default()).unwrap()
    }

    #[test]
    fn lines_break_between_words_and_long_words_are_cut() {
        assert_eq!(wrap("the quick brown fox", 9), ["the quick", "brown fox"]);
        assert_eq!(wrap("abcdefgh ij", 3), ["abc", "def", "gh", "ij"]);
        assert_eq!(wrap("   ", 9), [""]);
    }

    #[test]
    fn paragraphs_are_wrapped_at_the_margins_with_a_blank_line_between() {
        let mut tw = sm9();
        let long = "word ".repeat(30);
        retype(&mut tw, &[long.as_str(), "", "Second\nline"]);
        let page = tw.page();
        let lines: Vec<String> = (0..page.half_lines())
            .map(|row| page.line_text(row))
            .filter(|l| !l.is_empty())
            .collect();
        assert!(
            lines
                .iter()
                .all(|l| l.len() <= 72 && l.starts_with(&" ".repeat(10)))
        );
        let text = export::plain_text(tw.document());
        assert_eq!(text.split_whitespace().filter(|w| *w == "word").count(), 30);
        assert!(text.ends_with("word\n\nSecond\nline\n"));
    }

    #[test]
    fn missing_keys_are_typed_as_a_typist_would() {
        let mut tw = sm9();
        retype(&mut tw, &["Page 1!"]);
        assert_eq!(tw.page().line_text(12).trim(), "Page l!");
        let cell = tw.page().cell(12, 16).unwrap();
        assert_eq!(cell.visible_glyphs().collect::<String>(), "'.");
    }

    #[test]
    fn accented_letters_are_kept_though_the_machine_lacks_them() {
        let mut tw = sm9();
        retype(&mut tw, &["Caf\u{e9} \u{e7}a"]);
        assert_eq!(tw.page().line_text(12).trim(), "Caf\u{e9} \u{e7}a");
        let cell = tw.page().cell(12, 13).unwrap();
        assert_eq!(
            cell.visible_glyphs().collect::<String>(),
            "\u{e9}",
            "one glyph"
        );
    }

    #[test]
    fn a_full_sheet_is_filed_and_the_next_starts_at_the_top_margin() {
        let mut tw = sm9();
        let paragraphs: Vec<String> = (0..40).map(|i| format!("Paragraph {i}")).collect();
        retype(&mut tw, &paragraphs);
        let sheets = tw.document().finished();
        assert!(!sheets.is_empty());
        let top = tw.profile().margins.top_lines * 2;
        for sheet in sheets.iter().chain([tw.page()]) {
            let rows: Vec<u16> = (0..sheet.half_lines())
                .filter(|&row| !sheet.line_text(row).is_empty())
                .collect();
            assert_eq!(rows.first(), Some(&top), "no blank line at the top");
            assert!(rows.iter().all(|&row| row + top < sheet.half_lines()));
        }
        let text = export::plain_text(tw.document());
        assert!(text.contains("Paragraph 39"));
    }
}
