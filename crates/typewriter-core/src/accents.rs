//! Accents for machines with dead keys (`Profile::dead_keys`): an accent
//! strikes without moving the carriage, so the letter typed next lands on it.
//!
//! Which letters come apart into a base and an accent is Unicode's own
//! canonical decomposition, not a table we maintain.

use unicode_normalization::UnicodeNormalization;

/// Grave, acute, circumflex, diaeresis, tilde: spacing forms, as on the keys.
pub const ACCENTS: [char; 5] = ['`', '\u{b4}', '^', '\u{a8}', '~'];

/// The combining marks those five dead keys strike, and their spacing forms.
/// A mark absent here (cedilla, ring, caron…) has no dead key on any machine.
const MARKS: [(char, char); 5] = [
    ('\u{300}', '`'),
    ('\u{301}', '\u{b4}'),
    ('\u{302}', '^'),
    ('\u{303}', '~'),
    ('\u{308}', '\u{a8}'),
];

pub fn is_accent(c: char) -> bool {
    ACCENTS.contains(&c)
}

/// A Latin letter with a diacritic (U+00C0 to U+024F): é, ç, å, ő…
pub fn is_accented(c: char) -> bool {
    ('\u{c0}'..'\u{250}').contains(&c) && c.nfd().next() != Some(c)
}

/// `c` as base letter and accent, if a dead key makes it: base and exactly
/// one of the five marks.
pub fn decompose(c: char) -> Option<(char, char)> {
    let mut parts = c.nfd();
    let base = parts.next().filter(|&b| b != c)?;
    let mark = parts.next()?;
    // Two marks (ḧ, ḯ…) are beyond one strike of one dead key.
    if parts.next().is_some() || !base.is_alphabetic() {
        return None;
    }
    MARKS
        .iter()
        .find(|(m, _)| *m == mark)
        .map(|&(_, spacing)| (base, spacing))
}

/// The letter `accent` over `base` reads as, if there is one.
pub fn compose(base: char, accent: char) -> Option<char> {
    let mark = MARKS.iter().find(|(_, s)| *s == accent).map(|&(m, _)| m)?;
    // Unicode composes no letter where two marks would pile up, nor where
    // the base takes no such accent: `None` either way.
    unicode_normalization::char::compose(base, mark)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn letters_come_apart_and_back_together() {
        for c in '\u{c0}'..'\u{250}' {
            if let Some((base, accent)) = decompose(c) {
                assert!(is_accent(accent), "{c}");
                assert!(is_accented(c));
                assert_eq!(compose(base, accent), Some(c), "{c}");
            }
        }
    }

    #[test]
    fn letters_without_a_dead_key_stay_whole() {
        assert_eq!(decompose('e'), None);
        assert_eq!(decompose('\u{e7}'), None, "a cedilla has no dead key");
        assert_eq!(decompose('\u{c5}'), None, "a ring has no dead key");
        assert_eq!(decompose('\u{1cd}'), None, "underdots are not accents");
        assert_eq!(compose('q', '^'), None);
        assert!(is_accented('\u{e7}') && !is_accented('e') && !is_accented('\u{df}'));
    }
}
