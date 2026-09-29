//! OpenDocument text (.odt) read as plain paragraphs.

use std::io::{Read, Seek};

use anyhow::{Context, bail};
use quick_xml::Reader;
use quick_xml::escape::resolve_predefined_entity;
use quick_xml::events::Event;

const MIMETYPE: &str = "application/vnd.oasis.opendocument.text";
/// Not the body's text: comments, footnotes, deleted changes.
const ASIDES: [&str; 3] = ["office:annotation", "text:note", "text:tracked-changes"];
const SOFT_HYPHEN: char = '\u{ad}';
/// Far more than any manuscript's text.
const CONTENT_MOST_BYTES: u64 = 64 * 1024 * 1024;
const MIMETYPE_MOST_BYTES: u64 = 1024;

/// The .odt's paragraphs, headings and list items included; a `\n` in one is
/// a line break. Formatting is dropped.
pub fn paragraphs(file: impl Read + Seek) -> anyhow::Result<Vec<String>> {
    let mut archive = zip::ZipArchive::new(file).context("not an OpenDocument file")?;
    let mimetype = archive
        .by_name("mimetype")
        .context("not an OpenDocument file")?;
    let mimetype = read_at_most(mimetype, MIMETYPE_MOST_BYTES, "its mimetype")?;
    if mimetype.trim() != MIMETYPE {
        bail!("not an OpenDocument text document ({})", mimetype.trim());
    }
    let content = archive
        .by_name("content.xml")
        .context("the document has no content")?;
    let content = read_at_most(content, CONTENT_MOST_BYTES, "its content")?;
    parse(&content).context("the document's content is damaged")
}

/// `entry` unpacked, refused past `most` bytes: a small file can unpack to
/// gigabytes (a zip bomb).
fn read_at_most(entry: impl Read, most: u64, what: &str) -> anyhow::Result<String> {
    let mut text = String::new();
    entry.take(most + 1).read_to_string(&mut text)?;
    if text.len() as u64 > most {
        bail!("{what} is over {} MiB", most / (1024 * 1024));
    }
    Ok(text)
}

fn parse(xml: &str) -> anyhow::Result<Vec<String>> {
    let mut reader = Reader::from_str(xml);
    let mut paragraphs = Vec::new();
    // Innermost last: a frame's paragraphs sit inside another. They come out
    // before it.
    let mut open: Vec<String> = Vec::new();
    // Depth inside an aside.
    let mut aside = 0_usize;
    loop {
        let event = reader.read_event()?;
        if aside > 0 {
            match event {
                Event::Start(_) => aside += 1,
                Event::End(_) => aside -= 1,
                Event::Eof => break,
                _ => {}
            }
            continue;
        }
        match event {
            Event::Start(tag) => match tag.name().as_ref() {
                name if ASIDES.contains(&name) => aside = 1,
                "text:p" | "text:h" => open.push(String::new()),
                _ => {}
            },
            Event::End(tag) => {
                if matches!(tag.name().as_ref(), "text:p" | "text:h")
                    && let Some(paragraph) = open.pop()
                {
                    paragraphs.push(paragraph);
                }
            }
            Event::Empty(tag) => {
                if let Some(paragraph) = open.last_mut() {
                    match tag.name().as_ref() {
                        "text:s" | "text:tab" => paragraph.push(' '),
                        "text:line-break" => paragraph.push('\n'),
                        _ => {}
                    }
                }
            }
            Event::Text(text) => {
                if let Some(paragraph) = open.last_mut() {
                    push_text(paragraph, &text.xml10_content());
                }
            }
            Event::GeneralRef(entity) => {
                if let Some(paragraph) = open.last_mut() {
                    match entity.resolve_char_ref()? {
                        Some(c) => push_text(paragraph, c.encode_utf8(&mut [0; 4])),
                        None => push_text(
                            paragraph,
                            resolve_predefined_entity(&entity).unwrap_or_default(),
                        ),
                    }
                }
            }
            Event::Eof => break,
            _ => {}
        }
    }
    Ok(paragraphs)
}

/// Line ends in the source are only spaces: breaks are elements.
fn push_text(paragraph: &mut String, text: &str) {
    paragraph.extend(
        text.chars()
            .filter(|&c| c != SOFT_HYPHEN)
            .map(|c| if c.is_whitespace() { ' ' } else { c }),
    );
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;

    const CONTENT: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<office:document-content xmlns:office="o" xmlns:text="t"><office:body><office:text>
<text:sequence-decls><text:sequence-decl text:name="Table"/></text:sequence-decls>
<text:h text:outline-level="1">Chapter <text:span text:style-name="T1">One</text:span></text:h>
<text:p>Fish &amp; chips<text:note><text:note-citation>1</text:note-citation><text:note-body><text:p>A footnote</text:p></text:note-body></text:note>, 3&#160;&#x2014; hyphen&#173;ated</text:p>
<text:p/>
<text:list><text:list-item><text:p>First<text:tab/>item<text:line-break/>second line</text:p></text:list-item></text:list>
<text:p><office:annotation><text:p>A comment</text:p></office:annotation>Last</text:p>
</office:text></office:body></office:document-content>"#;

    #[test]
    fn paragraphs_keep_their_words_and_lose_footnotes_and_comments() {
        assert_eq!(
            parse(CONTENT).unwrap(),
            [
                "Chapter One",
                "Fish & chips, 3 \u{2014} hyphenated",
                "First item\nsecond line",
                "Last",
            ]
        );
    }

    #[test]
    fn unpacking_stops_at_the_limit() {
        let small = read_at_most(&b"12345"[..], 5, "it").unwrap();
        assert_eq!(small, "12345");
        let error = read_at_most(&[b'x'; 3 * 1024 * 1024][..], 2 * 1024 * 1024, "it")
            .unwrap_err()
            .to_string();
        assert_eq!(error, "it is over 2 MiB");
    }

    #[test]
    fn a_file_that_is_no_odt_is_refused() {
        let error = paragraphs(std::io::Cursor::new(b"plain text".to_vec())).unwrap_err();
        assert_eq!(error.to_string(), "not an OpenDocument file");
    }
}
