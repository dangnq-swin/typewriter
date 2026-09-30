//! A simulated writer, for tests: a manuscript typed out over months, with
//! the typos, corrections, notes and re-fed sheets of real work. Seeded:
//! the same seed writes the same project.

#![allow(clippy::unwrap_used)]

use jiff::ToSpan;
use jiff::civil::{Date, date};
use typewriter_core::page::Shift;
use typewriter_core::retype::{self, Typist};
use typewriter_core::{Command, Constraints, EraseMode, Profile, Typewriter};

use crate::render::splitmix64;

const WORDS: &[&str] = &[
    "the",
    "of",
    "and",
    "a",
    "to",
    "in",
    "was",
    "he",
    "she",
    "that",
    "it",
    "his",
    "her",
    "for",
    "on",
    "with",
    "as",
    "at",
    "by",
    "had",
    "be",
    "from",
    "they",
    "this",
    "not",
    "but",
    "or",
    "which",
    "you",
    "one",
    "all",
    "were",
    "there",
    "would",
    "their",
    "we",
    "him",
    "been",
    "when",
    "who",
    "more",
    "no",
    "if",
    "out",
    "so",
    "said",
    "what",
    "up",
    "about",
    "into",
    "than",
    "them",
    "only",
    "other",
    "time",
    "some",
    "could",
    "these",
    "two",
    "first",
    "then",
    "like",
    "now",
    "over",
    "such",
    "man",
    "even",
    "most",
    "after",
    "also",
    "many",
    "before",
    "through",
    "back",
    "years",
    "where",
    "much",
    "way",
    "well",
    "down",
    "should",
    "because",
    "each",
    "just",
    "people",
    "how",
    "little",
    "good",
    "very",
    "world",
    "still",
    "own",
    "see",
    "work",
    "long",
    "here",
    "between",
    "life",
    "being",
    "under",
    "never",
    "day",
    "same",
    "another",
    "know",
    "while",
    "last",
    "might",
    "great",
    "old",
    "year",
    "off",
    "come",
    "since",
    "against",
    "came",
    "right",
    "take",
    "three",
    "house",
    "letter",
    "window",
    "morning",
    "evening",
    "sister",
    "brother",
    "garden",
    "road",
    "rain",
    "silence",
    "table",
    "door",
    "voice",
    "question",
    "answer",
    "journey",
    "promise",
    "afternoon",
    "neighbour",
    "carriage",
    "village",
    "river",
    "remembered",
    "wondered",
    "listened",
    "walked",
    "turned",
    "waited",
    "believed",
    "whispered",
    "quietly",
    "suddenly",
    "perhaps",
    "certainly",
    "almost",
    "already",
    "together",
    "café",
];
const NOTES: &[&str] = &[
    "tighten this",
    "too slow here?",
    "check the dates",
    "move to ch. 3",
    "cut?",
    "more of the river",
    "her voice, not his",
    "reread aloud",
];
const PAD: &[&str] = &[
    "Things to check:\n- the carriage scene\n- her brother's age\n- the letter",
    "Title ideas\nThe Long Road\nAfter the Rain\nA Quiet House",
    "call the printer on Tuesday",
    "chapter order?\n1 morning\n2 the village\n3 the river",
    "ribbon, fluid, paper",
];
/// The typist's first day at the machine.
fn first_day() -> Date {
    date(2025, 2, 3)
}

/// A seeded source of chance.
struct Dice(u64);

impl Dice {
    fn roll(&mut self) -> u64 {
        self.0 = splitmix64(self.0);
        self.0
    }

    /// 0 to `n` - 1.
    fn below(&mut self, n: usize) -> usize {
        (self.roll() % n.max(1) as u64) as usize
    }

    fn one_in(&mut self, n: usize) -> bool {
        self.below(n) == 0
    }

    fn pick<'a, T>(&mut self, items: &'a [T]) -> &'a T {
        &items[self.below(items.len())]
    }
}

/// About `words` words of seeded prose, in paragraphs.
pub fn prose(words: usize, seed: u64) -> Vec<String> {
    let mut dice = Dice(seed);
    let mut paragraphs = Vec::new();
    let mut written = 0;
    while written < words {
        let dialogue = dice.one_in(5);
        let mut sentences = Vec::new();
        for _ in 0..2 + dice.below(6) {
            let length = 5 + dice.below(16);
            let mut sentence: Vec<String> = (0..length)
                .map(|_| (*dice.pick(WORDS)).to_owned())
                .collect();
            written += length;
            let mut first = sentence[0].chars();
            sentence[0] = first
                .next()
                .map(|c| c.to_uppercase().chain(first).collect())
                .unwrap_or_default();
            if length > 8 && dice.one_in(2) {
                let at = 3 + dice.below(length - 5);
                sentence[at].push(',');
            }
            let end = *dice.pick(&[".", ".", ".", ".", "?", "!"]);
            sentences.push(sentence.join(" ") + end);
        }
        let paragraph = sentences.join(" ");
        paragraphs.push(if dialogue {
            format!("\"{paragraph}\"")
        } else {
            paragraph
        });
    }
    paragraphs
}

/// A letter a finger might hit instead of `c`: a neighbour on the keyboard.
fn slip_of_the_finger(c: char, dice: &mut Dice) -> char {
    const ROWS: [&str; 3] = ["qwertzuiop", "asdfghjkl", "yxcvbnm"];
    let lower = c.to_ascii_lowercase();
    let neighbour = ROWS.iter().find_map(|row| {
        let at = row.find(lower)?;
        let keys: Vec<char> = row.chars().collect();
        let side = if at == 0 || (at + 1 < keys.len() && dice.one_in(2)) {
            at + 1
        } else {
            at - 1
        };
        Some(keys[side])
    });
    let wrong = neighbour.unwrap_or('e');
    if c.is_ascii_uppercase() {
        wrong.to_ascii_uppercase()
    } else {
        wrong
    }
}

/// Types `paragraphs` on a fresh SM9 as a person would over months: a
/// mistake every few lines, fixed by overstriking, eraser, fluid, the slip or
/// x-ing a word out; a day's quota of words at a time, logged; then notes on
/// some sheets, scratchpad pages and a few sheets rolled back in for an
/// insertion.
pub fn write(paragraphs: &[impl AsRef<str>], seed: u64) -> Typewriter {
    let profile =
        Profile::from_toml_str(include_str!("../../../profiles/olympia-sm9.toml")).unwrap();
    let mut machine = Typewriter::new(profile, Constraints::default()).unwrap();
    let mut dice = Dice(seed ^ 0x7772_6974_6572);
    let width = retype::line_width(&machine);
    let lines = retype::lines(paragraphs, width);

    let mut day = first_day();
    let mut quota = 250 + dice.below(1500);
    let mut today = 0;
    let mut typist = Typist::new(&mut machine);
    for (line, returns) in lines {
        let letters: Vec<char> = line.chars().collect();
        let room = width.saturating_sub(letters.len());
        let at = dice.below(letters.len().max(1));
        let slips = dice.one_in(3) && letters.get(at).is_some_and(char::is_ascii_alphabetic);
        if !slips {
            typist.type_line(&line, returns);
        } else {
            let head: String = letters[..at].iter().collect();
            typist.type_line(&head, returns);
            let rest = mistake(&mut typist, &letters[at..], room, &mut dice);
            typist.type_line(&rest, 0);
        }
        today += line.split_whitespace().count();
        if today >= quota {
            typist.machine().log_words(day, today as i64);
            today = 0;
            quota = 250 + dice.below(1500);
            let gap = match dice.below(10) {
                0..=5 => 1,
                6 | 7 => 2,
                _ => 3 + dice.below(8),
            };
            day = day.checked_add(i64::try_from(gap).unwrap().days()).unwrap();
        }
    }
    if today > 0 {
        typist.machine().log_words(day, today as i64);
    }
    machine.apply(Command::SetEraseMode(Constraints::default().erase));

    let sheets = machine.document().finished().len();
    for sheet in 0..sheets {
        if dice.one_in(6) {
            let mut note = (*dice.pick(NOTES)).to_owned();
            if dice.one_in(3) {
                note = format!("{note}\n{}", dice.pick(NOTES));
            }
            machine.annotate(sheet, &note);
        }
    }
    for page in 0..3 + dice.below(4) {
        machine.scratchpad_mut().write(page, dice.pick(PAD));
    }
    for _ in 0..(sheets / 40).max(1) {
        insert(&mut machine, &mut dice);
    }
    machine
}

/// Strikes a wrong key for `rest[0]` and fixes it; `room`: columns spare on
/// the line. Returns what is left to type.
fn mistake(typist: &mut Typist<'_>, rest: &[char], room: usize, dice: &mut Dice) -> String {
    let right = rest[0];
    let wrong = slip_of_the_finger(right, dice);
    let machine = typist.machine();
    let key = |machine: &mut Typewriter, c| {
        machine.apply(Command::Type(c));
    };
    let fix_with = |machine: &mut Typewriter, mode| {
        machine.apply(Command::SetEraseMode(mode));
    };
    // A whole wrong word, x-ed out, needs room for it and a space.
    let word = *dice.pick(WORDS);
    let x_out = word.len() < room && dice.one_in(8);
    if x_out {
        for c in word.chars() {
            key(machine, c);
        }
        for _ in word.chars() {
            machine.apply(Command::Backspace);
        }
        for _ in word.chars() {
            key(machine, 'x');
        }
        key(machine, ' ');
        return rest.iter().collect();
    }
    key(machine, wrong);
    match dice.below(9) {
        // Struck over, uncorrected.
        0..=2 => {
            machine.apply(Command::Backspace);
        }
        3 | 4 => {
            fix_with(machine, EraseMode::Eraser);
            machine.apply(Command::Erase);
        }
        5 | 6 => {
            fix_with(machine, EraseMode::Fluid);
            machine.apply(Command::Erase);
            // Now and then too impatient to let it dry: the ink runs.
            if !dice.one_in(6) {
                let carriage = machine.carriage();
                let (half_line, column) = (carriage.half_line, carriage.column);
                machine.apply(Command::FluidDried { half_line, column });
            }
        }
        _ => {
            fix_with(machine, EraseMode::Paper);
            machine.apply(Command::Backspace);
            machine.apply(Command::Erase);
            key(machine, wrong);
            machine.apply(Command::Erase);
            machine.apply(Command::Backspace);
        }
    }
    rest.iter().collect()
}

/// Rolls a finished sheet back in, types a line into a gap between
/// paragraphs and feeds it back out. Nothing if no sheet has a gap.
fn insert(machine: &mut Typewriter, dice: &mut Dice) {
    let sheets = machine.document().finished();
    let sheet = dice.below(sheets.len());
    let page = &sheets[sheet];
    let written = |row: u16| !page.line_text(row).trim().is_empty();
    let gaps: Vec<u16> = (2..page.half_lines().saturating_sub(2))
        .filter(|&row| !written(row) && written(row - 2) && written(row + 2))
        .collect();
    if gaps.is_empty() {
        return;
    }
    let row = *dice.pick(&gaps);
    let shift = Shift {
        across: dice.below(61) as i8 - 30,
        down: dice.below(41) as i8 - 20,
    };
    machine.apply(Command::RollIn { sheet, shift });
    while machine.carriage().half_line < row {
        machine.apply(Command::PlatenNotch);
    }
    for c in "[Insert: and the rain had stopped by then.]".chars() {
        machine.apply(Command::Type(c));
    }
    machine.apply(Command::FeedSheet);
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    use typewriter_core::page::{Correction, Mark};

    use super::*;

    /// Saved and opened again: the file, the machine, how long each took.
    fn reopened(machine: &Typewriter) -> (String, Typewriter, [Duration; 2]) {
        let start = Instant::now();
        let text = machine.to_folder_ron().unwrap();
        let saved = start.elapsed();
        let profile = machine.profile().clone();
        let start = Instant::now();
        let back = Typewriter::from_folder_ron(&text, |_| Some(profile)).unwrap();
        (text, back, [saved, start.elapsed()])
    }

    #[test]
    fn the_same_seed_writes_the_same_project() {
        let text = prose(600, 1);
        assert_eq!(text, prose(600, 1));
        assert_ne!(text, prose(600, 2));
        let (a, b) = (write(&text, 1), write(&text, 1));
        assert_eq!(a.to_folder_ron().unwrap(), b.to_folder_ron().unwrap());
    }

    /// Guards against loading going superlinear, as ron skipping ignored
    /// fields once made it: opening takes about as long as saving (20 times
    /// as long with that bug), whatever the machine's speed.
    #[test]
    fn a_long_project_with_real_work_on_it_reopens_whole_and_quickly() {
        let machine = write(&prose(30_000, 7), 7);
        let (text, back, [saved, opened]) = reopened(&machine);
        assert!(
            opened < 5 * saved,
            "saved in {saved:?}, opened in {opened:?}"
        );
        assert_eq!(back.document(), machine.document());
        assert_eq!(back.log(), machine.log());

        let document = back.document();
        let sheets = document.finished();
        assert!(sheets.len() > 50, "{} sheets", sheets.len());
        let marks: Vec<&[Mark]> = sheets
            .iter()
            .flat_map(|page| page.cells().map(|(_, cell)| cell.marks()))
            .filter(|marks| marks.len() > 1)
            .collect();
        let has = |wanted: fn(&Mark) -> bool| marks.iter().any(|m| m.iter().any(wanted));
        assert!(has(|m| matches!(m, Mark::Correction(Correction::Eraser))));
        assert!(has(|m| matches!(
            m,
            Mark::Correction(Correction::Fluid { .. })
        )));
        assert!(has(|m| matches!(m, Mark::Correction(Correction::Chalk(_)))));
        assert!(has(|m| matches!(m, Mark::Smudged(_))));
        assert!(
            marks
                .iter()
                .any(|m| m.iter().all(|m| matches!(m, Mark::Glyph(_))))
        );
        let crossed = marks
            .iter()
            .filter(|m| matches!(m, [Mark::Glyph(_), Mark::Glyph('x')]))
            .count();
        assert!(crossed > 20, "{crossed} letters x-ed out");
        assert!(back.log().days().len() > 20);
        assert!(sheets.iter().any(|page| !page.note().is_empty()));
        assert!(!document.scratchpad().is_fresh());
        assert!(text.contains("refeeds"), "a sheet was rolled back in");
    }

    /// A novel's worth, for measuring by hand:
    /// `TYPEWRITER_MANUSCRIPT=novel.odt cargo test -p typewriter-app --release
    /// -- --ignored --nocapture novel`. Without a manuscript, seeded prose.
    #[test]
    #[ignore = "slow: run by hand"]
    fn a_simulated_novel() {
        let paragraphs = match std::env::var_os("TYPEWRITER_MANUSCRIPT") {
            Some(path) => {
                let file = std::fs::File::open(path).unwrap();
                crate::odt::paragraphs(std::io::BufReader::new(file)).unwrap()
            }
            None => prose(120_000, 42),
        };
        let start = Instant::now();
        let machine = write(&paragraphs, 42);
        let typed = start.elapsed();
        let (text, back, [saved, reopen]) = reopened(&machine);
        assert_eq!(back.document(), machine.document());
        let sheets = machine.document().finished();
        let cells: usize = sheets.iter().map(|page| page.cells().count()).sum();
        let stacks = sheets
            .iter()
            .flat_map(|page| page.cells())
            .filter(|(_, cell)| cell.marks().len() > 1)
            .count();
        println!(
            "{} sheets, {cells} cells ({stacks} stacks), {} days, {} words logged",
            sheets.len(),
            machine.log().days().len(),
            machine.log().words()
        );
        println!(
            "{} bytes; typed in {typed:?}, saved in {saved:?}, reopened in {reopen:?}",
            text.len()
        );
    }
}
