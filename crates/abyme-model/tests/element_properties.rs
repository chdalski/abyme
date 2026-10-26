//! Properties of the element file that hold for any element and any input.

use abyme_model::{Element, ElementFields, ElementId, TypeRef};
use proptest::prelude::*;

/// Characters an element file can hold in its values; the same rule as the
/// crate's, written out again so the test does not trust the code it tests.
fn allowed_char() -> impl Strategy<Value = char> {
    any::<char>().prop_filter("not YAML-printable", |character| {
        !matches!(
            character,
            '\u{0}'..='\u{8}'
                | '\u{b}'
                | '\u{c}'
                | '\u{e}'..='\u{1f}'
                | '\u{7f}'..='\u{84}'
                | '\u{86}'..='\u{9f}'
                | '\u{fffe}'
                | '\u{ffff}'
        )
    })
}

/// Text of allowed characters, mixed with the characters the emitter
/// escapes and with `---` lines, so the escapes are exercised often.
fn allowed_text() -> impl Strategy<Value = String> {
    let piece = prop_oneof![
        3 => allowed_char().prop_map(String::from),
        1 => prop::sample::select(vec![
            "\\", "\"", "\t", "\n", "\r", "\r\n", "\u{85}", "\u{2028}", "\u{2029}",
            "\u{feff}", "\n---\n", "...", "#", ": ", "'",
        ])
        .prop_map(str::to_owned),
    ];
    prop::collection::vec(piece, 0..40).prop_map(|pieces| pieces.concat())
}

fn element_id() -> impl Strategy<Value = ElementId> {
    "[a-z0-9]{1,8}(-[a-z0-9]{1,8}){0,3}"
        .prop_filter_map("reserved device name", |text| text.parse().ok())
}

fn type_ref() -> impl Strategy<Value = TypeRef> {
    allowed_text().prop_filter_map("empty type", |text| TypeRef::try_from(text).ok())
}

fn element() -> impl Strategy<Value = Element> {
    (
        element_id(),
        type_ref(),
        allowed_text(),
        proptest::option::of(allowed_text()),
        any::<String>(),
    )
        .prop_map(|(id, type_ref, name, summary, body)| {
            Element::new(
                id,
                ElementFields {
                    type_ref,
                    name,
                    summary,
                    body,
                },
            )
        })
        .prop_filter_map("frontmatter over the limit", Result::ok)
}

proptest! {
    #[test]
    fn reads_back_any_element_unchanged(element in element()) {
        let read = Element::from_markdown(element.id(), &element.to_markdown());

        prop_assert_eq!(read, Ok(element));
    }

    #[test]
    fn rejects_or_reads_any_text_without_panicking(text in any::<String>()) {
        let expected: ElementId = "main".parse().unwrap();

        let _ = Element::from_markdown(&expected, &text);
    }

    #[test]
    fn rejects_or_reads_any_frontmatter_without_panicking(frontmatter in any::<String>()) {
        let expected: ElementId = "main".parse().unwrap();

        let _ = Element::from_markdown(&expected, &format!("---\n{frontmatter}\n---\n"));
    }
}
