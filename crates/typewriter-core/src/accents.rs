//! Accents for machines with dead keys (`Profile::dead_keys`): an accent
//! strikes without moving the carriage, so the letter typed next lands on it.

/// Grave, acute, circumflex, diaeresis, tilde: spacing forms, as on the keys.
pub const ACCENTS: [char; 5] = ['`', '\u{b4}', '^', '\u{a8}', '~'];

/// Latin letters with a diacritic (Unicode canonical decompositions,
/// U+00C0 to U+024F). A machine without the dead key can't type them.
const ACCENTED_LETTERS: &str = "ÀÁÂÃÄÅÇÈÉÊËÌÍÎÏÑÒÓÔÕÖÙÚÛÜÝàáâãäåçèéêëìíîïñòóôõöùúûüýÿĀāĂăĄąĆćĈĉĊċČčĎďĒēĔĕĖėĘęĚěĜĝĞğĠġĢģĤĥĨĩĪīĬĭĮįİĴĵĶķĹĺĻļĽľŃńŅņŇňŌōŎŏŐőŔŕŖŗŘřŚśŜŝŞşŠšŢţŤťŨũŪūŬŭŮůŰűŲųŴŵŶŷŸŹźŻżŽžƠơƯưǍǎǏǐǑǒǓǔǕǖǗǘǙǚǛǜǞǟǠǡǢǣǦǧǨǩǪǫǬǭǮǯǰǴǵǸǹǺǻǼǽǾǿȀȁȂȃȄȅȆȇȈȉȊȋȌȍȎȏȐȑȒȓȔȕȖȗȘșȚțȞȟȦȧȨȩȪȫȬȭȮȯȰȱȲȳ";

/// Letter, base, accent.
const ACCENTED: [(char, char, char); 80] = [
    ('À', 'A', '`'),
    ('Á', 'A', '´'),
    ('Â', 'A', '^'),
    ('Ã', 'A', '~'),
    ('Ä', 'A', '¨'),
    ('È', 'E', '`'),
    ('É', 'E', '´'),
    ('Ê', 'E', '^'),
    ('Ë', 'E', '¨'),
    ('Ì', 'I', '`'),
    ('Í', 'I', '´'),
    ('Î', 'I', '^'),
    ('Ï', 'I', '¨'),
    ('Ñ', 'N', '~'),
    ('Ò', 'O', '`'),
    ('Ó', 'O', '´'),
    ('Ô', 'O', '^'),
    ('Õ', 'O', '~'),
    ('Ö', 'O', '¨'),
    ('Ù', 'U', '`'),
    ('Ú', 'U', '´'),
    ('Û', 'U', '^'),
    ('Ü', 'U', '¨'),
    ('Ý', 'Y', '´'),
    ('à', 'a', '`'),
    ('á', 'a', '´'),
    ('â', 'a', '^'),
    ('ã', 'a', '~'),
    ('ä', 'a', '¨'),
    ('è', 'e', '`'),
    ('é', 'e', '´'),
    ('ê', 'e', '^'),
    ('ë', 'e', '¨'),
    ('ì', 'i', '`'),
    ('í', 'i', '´'),
    ('î', 'i', '^'),
    ('ï', 'i', '¨'),
    ('ñ', 'n', '~'),
    ('ò', 'o', '`'),
    ('ó', 'o', '´'),
    ('ô', 'o', '^'),
    ('õ', 'o', '~'),
    ('ö', 'o', '¨'),
    ('ù', 'u', '`'),
    ('ú', 'u', '´'),
    ('û', 'u', '^'),
    ('ü', 'u', '¨'),
    ('ý', 'y', '´'),
    ('ÿ', 'y', '¨'),
    ('Ć', 'C', '´'),
    ('ć', 'c', '´'),
    ('Ĉ', 'C', '^'),
    ('ĉ', 'c', '^'),
    ('Ĝ', 'G', '^'),
    ('ĝ', 'g', '^'),
    ('Ĥ', 'H', '^'),
    ('ĥ', 'h', '^'),
    ('Ĩ', 'I', '~'),
    ('ĩ', 'i', '~'),
    ('Ĵ', 'J', '^'),
    ('ĵ', 'j', '^'),
    ('Ĺ', 'L', '´'),
    ('ĺ', 'l', '´'),
    ('Ń', 'N', '´'),
    ('ń', 'n', '´'),
    ('Ŕ', 'R', '´'),
    ('ŕ', 'r', '´'),
    ('Ś', 'S', '´'),
    ('ś', 's', '´'),
    ('Ŝ', 'S', '^'),
    ('ŝ', 's', '^'),
    ('Ũ', 'U', '~'),
    ('ũ', 'u', '~'),
    ('Ŵ', 'W', '^'),
    ('ŵ', 'w', '^'),
    ('Ŷ', 'Y', '^'),
    ('ŷ', 'y', '^'),
    ('Ÿ', 'Y', '¨'),
    ('Ź', 'Z', '´'),
    ('ź', 'z', '´'),
];

pub fn is_accent(c: char) -> bool {
    ACCENTS.contains(&c)
}

/// A Latin letter with a diacritic of any kind: é, ç, å, ő…
pub fn is_accented(c: char) -> bool {
    !c.is_ascii() && ACCENTED_LETTERS.contains(c)
}

/// `c` as base letter and accent, if a dead key makes it.
pub fn decompose(c: char) -> Option<(char, char)> {
    ACCENTED
        .iter()
        .find(|&&(letter, _, _)| letter == c)
        .map(|&(_, base, accent)| (base, accent))
}

/// The letter `accent` over `base` reads as, if there is one.
pub fn compose(base: char, accent: char) -> Option<char> {
    ACCENTED
        .iter()
        .find(|&&(_, b, a)| b == base && a == accent)
        .map(|&(letter, _, _)| letter)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn letters_come_apart_and_back_together() {
        for &(letter, base, accent) in &ACCENTED {
            assert!(is_accent(accent));
            assert!(is_accented(letter));
            assert_eq!(decompose(letter), Some((base, accent)));
            assert_eq!(compose(base, accent), Some(letter));
        }
        assert_eq!(decompose('e'), None);
        assert_eq!(decompose('\u{e7}'), None, "a cedilla has no dead key");
        assert_eq!(compose('q', '^'), None);
        assert!(is_accented('\u{e7}') && !is_accented('e') && !is_accented('\u{df}'));
    }
}
