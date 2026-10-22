use std::collections::BTreeMap;
use std::num::NonZeroU32;

use abyme_model::{Rect, TypeRef};
use proptest::prelude::*;
use rstest::rstest;

use super::*;

/// The six characters or sequences that end a line somewhere.
const LINE_BREAKS: [&str; 6] = ["\n", "\r", "\r\n", "\u{85}", "\u{2028}", "\u{2029}"];

fn rect() -> Rect {
    Rect {
        x: 0,
        y: 0,
        width: NonZeroU32::MIN,
        height: NonZeroU32::MIN,
    }
}

fn element(id: &str, type_ref: &str, name: &str, summary: Option<&str>) -> Element {
    Element::new(
        id.parse().unwrap(),
        TypeRef::try_from(type_ref.to_owned()).unwrap(),
        name.to_owned(),
        summary.map(str::to_owned),
        String::new(),
    )
    .unwrap()
}

/// A snapshot with the board `b` titled `title` that places `elements` and
/// the IDs in `missing`, whose files do not exist.
fn snapshot(title: &str, elements: Vec<Element>, missing: &[&str]) -> Snapshot {
    let placed = elements
        .iter()
        .map(|element| element.id().clone())
        .chain(missing.iter().map(|id| id.parse().unwrap()));
    let board = Board {
        id: "b".parse().unwrap(),
        title: title.to_owned(),
        placements: placed.map(|id| (id, rect())).collect(),
    };
    Snapshot {
        elements: elements
            .into_iter()
            .map(|element| (element.id().clone(), element))
            .collect(),
        boards: BTreeMap::from([(board.id.clone(), board)]),
    }
}

fn outline_of(snapshot: &Snapshot) -> String {
    outline(snapshot, &"b".parse().unwrap()).unwrap()
}

#[test]
fn renders_the_documented_format() {
    let snapshot = snapshot(
        "Container Level",
        vec![
            element(
                "payment-service",
                "c4/container",
                "Payment Service",
                Some("Stripe payments"),
            ),
            element("orders-db", "c4/datastore", "Orders DB", None),
            element(
                "checkout",
                "c4/container",
                "Checkout",
                Some("order completion"),
            ),
        ],
        &["web-shop"],
    );

    assert_eq!(
        outline_of(&snapshot),
        "Board: b · Container Level\n\
         \n\
         checkout         c4/container  Checkout         order completion\n\
         orders-db        c4/datastore  Orders DB\n\
         payment-service  c4/container  Payment Service  Stripe payments\n\
         web-shop         ⚠ missing\n"
    );
}

#[test]
fn renders_a_board_without_placements_as_its_header() {
    assert_eq!(
        outline_of(&snapshot("Empty", Vec::new(), &[])),
        "Board: b · Empty\n\n"
    );
}

#[test]
fn shows_a_missing_element_without_affecting_the_others() {
    let snapshot = snapshot(
        "Board",
        vec![
            element("a", "t", "A", Some("first")),
            element("c", "t", "C", None),
        ],
        &["b"],
    );

    let outline = outline_of(&snapshot);

    assert_eq!(
        outline.lines().collect::<Vec<_>>(),
        [
            "Board: b · Board",
            "",
            "a  t  A  first",
            "b  ⚠ missing",
            "c  t  C"
        ]
    );
}

#[test]
fn names_an_unknown_board() {
    let error = outline(&Snapshot::default(), &"nowhere".parse().unwrap()).unwrap_err();

    assert_eq!(error.to_string(), "no board has the ID nowhere");
}

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

#[test]
fn a_long_name_does_not_pad_the_other_lines() {
    let long = "n".repeat(10_000);
    let mut elements = vec![element("long", "t", &long, Some("s"))];
    elements.extend((0..100).map(|index| element(&format!("e{index}"), "t", "short", Some("s"))));
    let snapshot = snapshot("Board", elements, &[]);

    let outline = outline_of(&snapshot);

    let bound = 3 * (MAX_COLUMN_WIDTH + 2) + "s".len();
    let short_lines: Vec<&str> = outline
        .lines()
        .filter(|line| line.starts_with('e'))
        .collect();
    assert_eq!(short_lines.len(), 100);
    for line in short_lines {
        assert!(
            line.chars().count() <= bound,
            "{} characters",
            line.chars().count()
        );
    }
}

/// An element with arbitrary type, name, and summary, if the element file
/// can hold them.
fn arbitrary_element(index: usize) -> impl Strategy<Value = Option<Element>> {
    (
        any::<String>(),
        any::<String>(),
        proptest::option::of(any::<String>()),
    )
        .prop_map(move |(type_ref, name, summary)| {
            let type_ref = TypeRef::try_from(type_ref).ok()?;
            let id = format!("e{index}").parse().ok()?;
            Element::new(id, type_ref, name, summary, String::new()).ok()
        })
}

proptest! {
    #[test]
    fn writes_one_line_per_element_without_any_escaped_character(
        title in any::<String>(),
        elements in (0..6_usize).prop_flat_map(|count| {
            (0..count).map(arbitrary_element).collect::<Vec<_>>()
        }),
    ) {
        let elements: Vec<Element> = elements.into_iter().flatten().collect();
        let count = elements.len();
        let snapshot = snapshot(&title, elements, &[]);

        let outline = outline_of(&snapshot);

        prop_assert_eq!(outline.lines().count(), 2 + count);
        prop_assert_eq!(outline.matches('\n').count(), 2 + count);
        let inner = outline.replace('\n', "");
        prop_assert!(
            !inner.chars().any(needs_escape),
            "{:?}",
            outline
        );
    }
}

#[test]
fn lists_elements_in_the_given_order_and_marks_missing_ones() {
    let snapshot = snapshot(
        "Board",
        vec![
            element("a", "t", "A", None),
            element("b", "t", "B", Some("second")),
            element("c", "t", "C", None),
        ],
        &[],
    );
    let ids: Vec<ElementId> = ["c", "a", "gone", "b"]
        .iter()
        .map(|id| id.parse().unwrap())
        .collect();

    assert_eq!(
        element_list(&snapshot, &ids),
        "c     t  C\n\
         a     t  A\n\
         gone  ⚠ missing\n\
         b     t  B  second\n"
    );
}

#[test]
fn lists_an_element_given_twice_twice() {
    let snapshot = snapshot("Board", vec![element("a", "t", "A", None)], &[]);
    let id: ElementId = "a".parse().unwrap();

    assert_eq!(element_list(&snapshot, [&id, &id]), "a  t  A\na  t  A\n");
}

#[test]
fn lists_nothing_for_no_ids() {
    let snapshot = snapshot("Board", vec![element("a", "t", "A", None)], &[]);

    assert_eq!(element_list(&snapshot, []), "");
}

#[test]
fn escapes_element_text_in_the_list() {
    let snapshot = snapshot("Board", vec![element("a", "t", "x\ny\u{202e}z", None)], &[]);
    let id: ElementId = "a".parse().unwrap();

    assert_eq!(element_list(&snapshot, [&id]), "a  t  x\\ny\\u{202e}z\n");
}
