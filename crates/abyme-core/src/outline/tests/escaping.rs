//! Tests of escaping what the repository writes onto one line.

use super::*;

/// The six characters or sequences that end a line somewhere.
const LINE_BREAKS: [&str; 6] = ["\n", "\r", "\r\n", "\u{85}", "\u{2028}", "\u{2029}"];

#[rstest]
fn keeps_each_element_on_one_line_whatever_line_break_its_texts_hold(
    #[values("\n", "\r", "\r\n", "\u{85}", "\u{2028}", "\u{2029}")] line_break: &str,
) {
    let text = format!("x{line_break}y");
    let snapshot = snapshot(
        &text,
        vec![
            element("a", &text, &text, Some(&text)),
            element("b", "t", &text, None),
        ],
        &[],
    );

    let outline = outline_of(&snapshot);

    assert_eq!(outline.lines().count(), 4, "{outline:?}");
    let inner = outline.trim_end_matches('\n');
    for other in LINE_BREAKS {
        assert_eq!(
            inner.matches(other).count(),
            3 * usize::from(other == "\n"),
            "{other:?}"
        );
    }
    let escaped = one_line(line_break);
    assert_eq!(outline.matches(&escaped).count(), 5, "{outline:?}");
}

#[test]
fn escapes_terminal_and_other_control_characters_in_the_title() {
    let snapshot = snapshot("\u{1b}[31mred\u{0}\u{7f}\u{9b}", Vec::new(), &[]);

    assert_eq!(
        outline_of(&snapshot),
        "Board: b · \\u{1b}[31mred\\u{0}\\u{7f}\\u{9b}\n\n"
    );
}

#[rstest]
#[case::right_to_left_override('\u{202e}', "\\u{202e}")]
#[case::left_to_right_isolate('\u{2066}', "\\u{2066}")]
#[case::arabic_letter_mark('\u{61c}', "\\u{61c}")]
#[case::zero_width_space('\u{200b}', "\\u{200b}")]
#[case::byte_order_mark('\u{feff}', "\\u{feff}")]
#[case::soft_hyphen('\u{ad}', "\\u{ad}")]
#[case::nominal_digit_shapes('\u{206f}', "\\u{206f}")]
#[case::interlinear_annotation('\u{fff9}', "\\u{fff9}")]
#[case::tag_language('\u{e0001}', "\\u{e0001}")]
#[case::tag_letter('\u{e0049}', "\\u{e0049}")]
#[case::cancel_tag('\u{e007f}', "\\u{e007f}")]
fn escapes_invisible_and_reordering_characters(#[case] character: char, #[case] expected: &str) {
    let name = format!("a{character}b");
    let snapshot = snapshot("Board", vec![element("a", "t", &name, None)], &[]);

    assert!(
        outline_of(&snapshot).ends_with(&format!("a  t  a{expected}b\n")),
        "{:?}",
        outline_of(&snapshot)
    );
}

#[test]
fn escapes_the_first_and_last_character_of_every_range() {
    for range in &FORMAT_AND_SEPARATORS {
        for character in [*range.start(), *range.end()] {
            assert_eq!(
                one_line(&character.to_string()),
                character.escape_unicode().to_string()
            );
        }
    }
}

#[test]
fn hides_a_tag_sequence_from_no_reader() {
    let smuggled: String = "\u{e0001}\u{e0049}\u{e0067}\u{e006e}\u{e006f}\u{e0072}\u{e0065}".into();

    assert_eq!(
        one_line(&format!("ok{smuggled}")),
        "ok\\u{e0001}\\u{e0049}\\u{e0067}\\u{e006e}\\u{e006f}\\u{e0072}\\u{e0065}"
    );
}

#[test]
fn tells_a_written_backslash_n_from_an_escaped_line_feed() {
    assert_eq!(one_line("a\\nb"), "a\\\\nb");
    assert_eq!(one_line("a\nb"), "a\\nb");
    assert_ne!(one_line("a\\nb"), one_line("a\nb"));
}

#[rstest]
#[case::precomposed_umlaut("Zürich")]
#[case::decomposed_umlaut("Zu\u{308}rich")]
#[case::cjk("支付服务")]
#[case::emoji_with_variation_selector("☎\u{fe0f}")]
#[case::arabic("خدمة الدفع")]
#[case::space("Payment Service")]
fn keeps_ordinary_text_as_it_is(#[case] text: &str) {
    assert_eq!(one_line(text), text);
}
