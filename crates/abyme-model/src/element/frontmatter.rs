//! Splitting an element file into frontmatter and body, and writing the
//! frontmatter.

use super::{Element, ElementProblem};

/// The line that opens and closes the frontmatter.
const DELIMITER: &str = "---";

/// Splits `text` into the frontmatter, which is the lines between the two
/// `---` lines with their line breaks, and the body, which is everything
/// after the closing line, byte for byte.
///
/// The closing line is `---` followed by a line feed, a carriage return and
/// line feed, or the end of the text.
pub fn split(text: &str) -> Result<(&str, &str), ElementProblem> {
    if text.starts_with('\u{feff}') {
        return Err(ElementProblem::ByteOrderMark);
    }
    let rest = text
        .strip_prefix("---\n")
        .or_else(|| text.strip_prefix("---\r\n"))
        .ok_or(ElementProblem::MissingOpeningLine)?;
    let mut offset = 0;
    for line in rest.split_inclusive('\n') {
        let content = line.strip_suffix('\n').map_or(line, |without_feed| {
            without_feed.strip_suffix('\r').unwrap_or(without_feed)
        });
        if content == DELIMITER {
            let frontmatter = rest.get(..offset).unwrap_or_default();
            let body = rest.get(offset + line.len()..).unwrap_or_default();
            return Ok((frontmatter, body));
        }
        offset += line.len();
    }
    Err(ElementProblem::MissingClosingLine)
}

/// The size of a frontmatter as the emitter would count it: its bytes,
/// with a carriage return and line feed counted as one line break.
pub fn normalized_len(frontmatter: &str) -> usize {
    frontmatter.len() - frontmatter.matches("\r\n").count()
}

/// The frontmatter lines of `element`, each ending with a line feed. The
/// relations, if any, come last, one line each in their sorted order.
pub fn emit(element: &Element) -> String {
    let summary = element
        .summary
        .as_deref()
        .map(|summary| format!("summary: {}\n", quoted(summary)))
        .unwrap_or_default();
    let anchor = element
        .anchor
        .as_ref()
        .map(|anchor| format!("anchor: {}\n", quoted(anchor.as_str())))
        .unwrap_or_default();
    let verified = element
        .verified
        .as_ref()
        .map(|stamp| {
            format!(
                "verified: {}\nverified_by: {}\n",
                quoted(stamp.commit.as_str()),
                quoted(stamp.by.as_str())
            )
        })
        .unwrap_or_default();
    let relations = element
        .relations
        .iter()
        .map(|((kind, to), label)| {
            let label = label
                .as_deref()
                .map(|label| format!(", label: {}", quoted(label)))
                .unwrap_or_default();
            format!(
                "  - {{kind: {}, to: {}{label}}}\n",
                quoted(kind.as_str()),
                quoted(to.as_str())
            )
        })
        .collect::<Vec<_>>()
        .concat();
    let relations = if relations.is_empty() {
        relations
    } else {
        format!("relations:\n{relations}")
    };
    format!(
        "id: {}\ntype: {}\nname: {}\n{summary}{anchor}{verified}{relations}",
        quoted(element.id.as_str()),
        quoted(element.type_ref.as_str()),
        quoted(&element.name),
    )
}

/// `text` as a YAML double-quoted scalar. Only the characters that would
/// end the string or the line, or that YAML would fold or drop, are
/// escaped; every other character is kept as it is.
fn quoted(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 2);
    out.push('"');
    for character in text.chars() {
        match character {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\t' => out.push_str("\\t"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\u{85}' => out.push_str("\\N"),
            '\u{2028}' => out.push_str("\\L"),
            '\u{2029}' => out.push_str("\\P"),
            '\u{feff}' => out.push_str("\\uFEFF"),
            other => out.push(other),
        }
    }
    out.push('"');
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    #[rstest]
    #[case::line_feeds("---\na\n---\nbody", "a\n", "body")]
    #[case::carriage_returns("---\r\na\r\n---\r\nbody\r\n", "a\r\n", "body\r\n")]
    #[case::closing_line_at_the_end("---\na\n---", "a\n", "")]
    #[case::empty_frontmatter("---\n---\nbody", "", "body")]
    #[case::delimiter_lines_in_the_body("---\na\n---\n---\nb\n---\n", "a\n", "---\nb\n---\n")]
    fn splits_frontmatter_and_body(
        #[case] text: &str,
        #[case] frontmatter: &str,
        #[case] body: &str,
    ) {
        assert_eq!(split(text), Ok((frontmatter, body)));
    }

    #[rstest]
    #[case::byte_order_mark("\u{feff}---\na\n---\n", ElementProblem::ByteOrderMark)]
    #[case::no_opening_line("a\n---\n", ElementProblem::MissingOpeningLine)]
    #[case::opening_line_with_text("--- a\n---\n", ElementProblem::MissingOpeningLine)]
    #[case::no_closing_line("---\na\n", ElementProblem::MissingClosingLine)]
    #[case::closing_line_with_lone_carriage_return(
        "---\na\n---\r",
        ElementProblem::MissingClosingLine
    )]
    #[case::closing_line_with_text("---\na\n--- b\n", ElementProblem::MissingClosingLine)]
    fn rejects_files_without_a_frontmatter(#[case] text: &str, #[case] problem: ElementProblem) {
        assert_eq!(split(text), Err(problem));
    }

    #[test]
    fn counts_a_carriage_return_and_line_feed_as_one_byte() {
        assert_eq!(normalized_len("a\r\nb\n"), 4);
    }

    #[rstest]
    #[case::backslash("a\\b", r#""a\\b""#)]
    #[case::double_quote("a\"b", r#""a\"b""#)]
    #[case::tab("a\tb", r#""a\tb""#)]
    #[case::line_feed("a\nb", r#""a\nb""#)]
    #[case::carriage_return("a\rb", r#""a\rb""#)]
    #[case::next_line("a\u{85}b", r#""a\Nb""#)]
    #[case::line_separator("a\u{2028}b", r#""a\Lb""#)]
    #[case::paragraph_separator("a\u{2029}b", r#""a\Pb""#)]
    #[case::byte_order_mark("a\u{feff}b", r#""a\uFEFFb""#)]
    #[case::other_characters_unchanged("é ✓ 😀 '", "\"é ✓ 😀 '\"")]
    fn quotes_and_escapes_values(#[case] text: &str, #[case] expected: &str) {
        assert_eq!(quoted(text), expected);
    }
}
