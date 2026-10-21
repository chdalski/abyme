//! Showing text from untrusted input in messages.

/// Quotes `text` with every control or special character escaped and cuts
/// it after `limit` characters.
///
/// Messages reach the AI and the terminal, so a value from a file must not
/// break their lines, smuggle in terminal escapes, or flood them.
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
