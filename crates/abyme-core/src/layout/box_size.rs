//! How large a box must be for the text the canvas shows in it.
//!
//! An estimate by character width: the canvas shows the system font, which no
//! measurement here could match. `.element` in
//! `crates/abyme-canvas/ui/src/canvas.css` draws the text with the line height
//! and the wrapping assumed here; the two change together.

use unicode_width::UnicodeWidthChar;

use super::Size;

/// How many cells of `unicode-width` fit on a line: 162 pixels inside the
/// frame of a box, at 7 pixels per cell, which leaves room in a line of 12-pixel
/// text. A wide character takes two cells.
const LINE_CELLS: usize = 23;
/// The height of a line in pixels.
const LINE_HEIGHT: u32 = 15;
/// The lines that fit a box of the default height, beside 18 pixels of frame and
/// 4 above the summary.
const DEFAULT_LINES: u32 = 4;
/// The lines a box grows to: 300 pixels high. Text beyond them is cut off.
const MAX_LINES: u32 = 18;

/// The size of a box showing an element's type, name, and summary: the default
/// width, and a height that grows by whole lines from the default up to a fixed
/// maximum. Reads no more text than fits that maximum.
#[must_use]
pub fn box_size(type_ref: &str, name: &str, summary: Option<&str>) -> Size {
    let lines = [Some(type_ref), Some(name), summary]
        .into_iter()
        .flatten()
        .fold(0, |lines, text| {
            lines + wrapped_lines(text, MAX_LINES - lines)
        });
    Size {
        width: Size::DEFAULT.width,
        height: Size::DEFAULT
            .height
            .saturating_add(lines.saturating_sub(DEFAULT_LINES) * LINE_HEIGHT),
    }
}

/// How many lines `text` takes, at most `limit`, wrapped as the canvas wraps it:
/// whitespace collapses and breaks a line, a word that does not fit moves to the
/// next line, and a word longer than a line breaks anywhere.
fn wrapped_lines(text: &str, limit: u32) -> u32 {
    let mut lines = 0;
    let mut used = 0;
    let mut word = 0;
    for character in text.chars() {
        if lines >= limit {
            break;
        }
        if character.is_whitespace() {
            word = 0;
            continue;
        }
        let cells = UnicodeWidthChar::width(character).unwrap_or(0);
        if cells == 0 {
            continue;
        }
        if lines == 0 {
            lines = 1;
        }
        if word == 0 && used > 0 {
            used += 1;
        }
        if used + cells > LINE_CELLS {
            lines += 1;
            used = if word < used { word } else { 0 };
            word = used;
        }
        used += cells;
        word += cells;
    }
    lines.min(limit)
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;
    use rstest::rstest;

    const MAX_HEIGHT: u32 = 90 + (MAX_LINES - DEFAULT_LINES) * LINE_HEIGHT;
    const MAX_FRONTMATTER: usize = abyme_model::MAX_FRONTMATTER_BYTES;

    /// The height of a box whose type and name take a line each, and whose
    /// summary is `summary`.
    fn height_with_summary(summary: &str) -> u32 {
        box_size("c4/container", "Payments", Some(summary))
            .height
            .get()
    }

    #[test]
    fn text_that_fits_three_lines_gives_the_default_size() {
        assert_eq!(
            box_size("c4/container", "Payment Service", Some("Stripe payments")),
            Size::DEFAULT
        );
    }

    #[test]
    fn no_summary_gives_the_default_size() {
        assert_eq!(
            box_size("c4/container", "Payment Service", None),
            Size::DEFAULT
        );
    }

    #[rstest]
    #[case::two_lines_of_summary(&"word ".repeat(8), 90)]
    #[case::three_lines_of_summary(&"word ".repeat(12), 105)]
    #[case::four_lines_of_summary(&"word ".repeat(16), 120)]
    fn the_height_grows_by_whole_lines(#[case] summary: &str, #[case] height: u32) {
        assert_eq!(height_with_summary(summary), height);
    }

    #[test]
    fn a_word_that_does_not_fit_moves_to_the_next_line() {
        let word_of_a_full_line = "x".repeat(LINE_CELLS);
        let summary = format!("a {word_of_a_full_line} b");

        assert_eq!(wrapped_lines(&summary, MAX_LINES), 3);
    }

    #[test]
    fn a_word_longer_than_a_line_breaks_anywhere() {
        let summary = "x".repeat(LINE_CELLS * 3 + 1);

        assert_eq!(wrapped_lines(&summary, MAX_LINES), 4);
    }

    #[test]
    fn a_word_after_a_word_breaks_where_the_line_ends() {
        let summary = format!("a {}", "x".repeat(LINE_CELLS * 2));

        assert_eq!(wrapped_lines(&summary, MAX_LINES), 3);
    }

    #[test]
    fn a_wide_character_takes_two_cells() {
        let half_a_line = LINE_CELLS / 2;

        assert_eq!(wrapped_lines(&"界".repeat(half_a_line), MAX_LINES), 1);
        assert_eq!(wrapped_lines(&"界".repeat(half_a_line + 1), MAX_LINES), 2);
    }

    #[rstest]
    #[case::whitespace(" \n\t\r\u{85}")]
    #[case::zero_width("\u{200b}\u{202e}\u{0}")]
    #[case::empty("")]
    fn text_without_width_takes_no_line(#[case] text: &str) {
        assert_eq!(wrapped_lines(&text.repeat(1_000), MAX_LINES), 0);
    }

    #[test]
    fn whitespace_collapses_between_words() {
        let summary = format!("a{}b", " ".repeat(1_000));

        assert_eq!(wrapped_lines(&summary, MAX_LINES), 1);
    }

    #[rstest]
    #[case::one_word(&"x".repeat(MAX_FRONTMATTER))]
    #[case::wide_characters(&"界".repeat(MAX_FRONTMATTER / 3))]
    #[case::words(&"x ".repeat(MAX_FRONTMATTER / 2))]
    fn text_far_beyond_the_maximum_gives_the_maximum(#[case] summary: &str) {
        assert_eq!(height_with_summary(summary), MAX_HEIGHT);
    }

    #[rstest]
    #[case::whitespace(&" ".repeat(MAX_FRONTMATTER))]
    #[case::line_breaks(&"\n".repeat(MAX_FRONTMATTER))]
    #[case::zero_width(&"\u{200b}".repeat(MAX_FRONTMATTER / 3))]
    fn long_text_without_width_adds_no_line(#[case] summary: &str) {
        assert_eq!(height_with_summary(summary), 90);
    }

    #[test]
    fn a_text_beyond_the_maximum_and_the_same_text_doubled_give_the_same_height() {
        let summary = "word ".repeat(200);

        assert_eq!(
            height_with_summary(&summary),
            height_with_summary(&summary.repeat(2))
        );
    }

    #[test]
    fn a_name_at_the_maximum_leaves_no_room_for_a_summary() {
        let name = "x".repeat(LINE_CELLS * MAX_LINES as usize);

        assert_eq!(
            box_size("t", &name, Some("summary")),
            box_size("t", &name, None)
        );
        assert_eq!(box_size("t", &name, None).height.get(), MAX_HEIGHT);
    }

    proptest! {
        #[test]
        fn any_text_gives_the_width_and_a_height_of_whole_lines(
            type_ref in any::<String>(),
            name in any::<String>(),
            summary in proptest::option::of(any::<String>()),
            control in "[\\x00-\\x1f\u{200b}\u{202e}a 界]{0,200}",
        ) {
            let size = box_size(&format!("{type_ref}{control}"), &name, summary.as_deref());

            prop_assert_eq!(size.width, Size::DEFAULT.width);
            prop_assert!((90..=MAX_HEIGHT).contains(&size.height.get()));
            prop_assert_eq!((size.height.get() - 90) % LINE_HEIGHT, 0);
        }
    }
}
