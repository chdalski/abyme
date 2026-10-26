//! Showing text from untrusted input in messages, and the characters such
//! text may hide or reorder itself with.

use std::ops::RangeInclusive;

/// The characters of the Unicode general categories Cf (format), Zl (line
/// separator), and Zp (paragraph separator), as of Unicode 15.0.
///
/// They are invisible or move text around, so a reader cannot see them in a
/// review.
pub const FORMAT_AND_SEPARATORS: [RangeInclusive<char>; 21] = [
    '\u{ad}'..='\u{ad}',
    '\u{600}'..='\u{605}',
    '\u{61c}'..='\u{61c}',
    '\u{6dd}'..='\u{6dd}',
    '\u{70f}'..='\u{70f}',
    '\u{890}'..='\u{891}',
    '\u{8e2}'..='\u{8e2}',
    '\u{180e}'..='\u{180e}',
    '\u{200b}'..='\u{200f}',
    '\u{2028}'..='\u{202e}',
    '\u{2060}'..='\u{2064}',
    '\u{2066}'..='\u{206f}',
    '\u{feff}'..='\u{feff}',
    '\u{fff9}'..='\u{fffb}',
    '\u{110bd}'..='\u{110bd}',
    '\u{110cd}'..='\u{110cd}',
    '\u{13430}'..='\u{1343f}',
    '\u{1bca0}'..='\u{1bca3}',
    '\u{1d173}'..='\u{1d17a}',
    '\u{e0001}'..='\u{e0001}',
    '\u{e0020}'..='\u{e007f}',
];

/// Whether a character is a control character (Unicode Cc) or one of
/// [`FORMAT_AND_SEPARATORS`]: a reader cannot see it, or it breaks a line or
/// reorders text.
#[must_use]
pub fn hides_or_breaks_text(character: char) -> bool {
    character.is_control()
        || FORMAT_AND_SEPARATORS
            .iter()
            .any(|range| range.contains(&character))
}

/// Quotes `text` with every control or special character escaped and cuts
/// it after `limit` characters. Use it for every text from untrusted input
/// that a message shows.
///
/// Messages reach the AI and the terminal, so a value from a file must not
/// break their lines, smuggle in terminal escapes, or flood them.
#[must_use]
pub fn shown(text: &str, limit: usize) -> String {
    let mut chars = text.chars();
    let head: String = chars
        .by_ref()
        .take(limit)
        .flat_map(char::escape_debug)
        .collect();
    let ellipsis = if chars.next().is_some() { "…" } else { "" };
    format!("\"{head}{ellipsis}\"")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escapes_line_breaks_and_terminal_escapes() {
        assert_eq!(shown("a\nb\u{1b}[31m", 80), r#""a\nb\u{1b}[31m""#);
    }

    #[test]
    fn cuts_long_text_and_marks_the_cut() {
        assert_eq!(shown("abcdef", 3), "\"abc…\"");
    }

    #[test]
    fn keeps_text_at_the_limit_whole() {
        assert_eq!(shown("abc", 3), "\"abc\"");
    }
}
