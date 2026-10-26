//! Properties of the element file that hold for any element and any input.

use abyme_model::{Element, ElementFields, ElementId, RelationKind, Relations, TypeRef};
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

fn relation_kind() -> impl Strategy<Value = RelationKind> {
    prop::sample::select(vec![
        RelationKind::Concerns,
        RelationKind::DependsOn,
        RelationKind::Describes,
        RelationKind::SeeAlso,
        RelationKind::Uses,
    ])
}

fn relations() -> impl Strategy<Value = Relations> {
    prop::collection::btree_map(
        (relation_kind(), element_id()),
        proptest::option::of(allowed_text()),
        0..4,
    )
}

fn element() -> impl Strategy<Value = Element> {
    (
        element_id(),
        type_ref(),
        allowed_text(),
        proptest::option::of(allowed_text()),
        any::<String>(),
        relations(),
    )
        .prop_map(|(id, type_ref, name, summary, body, relations)| {
            Element::new(
                id,
                ElementFields {
                    type_ref,
                    name,
                    summary,
                    body,
                    relations,
                },
            )
        })
        .prop_filter_map(
            "frontmatter over the limit or a relation to itself",
            Result::ok,
        )
}

/// Frontmatter built from the pieces that relations are written with,
/// mixed with the YAML that the reader must reject, so arbitrary text
/// reaches the reading of relations.
fn relation_like_frontmatter() -> impl Strategy<Value = String> {
    let piece = prop::sample::select(vec![
        "relations:",
        "relations: ",
        "\n",
        "\n  - ",
        "- ",
        "{",
        "}",
        "[",
        "]",
        ", ",
        ": ",
        "kind",
        "to",
        "label",
        "\"uses\"",
        "uses",
        "\"main\"",
        "other",
        "\"a\\\"b\"",
        "&a ",
        "*a",
        "!!str ",
        "!!seq ",
        "!!map ",
        "#",
        "'x'",
        "|",
        ">",
        "? ",
        "\"\"",
        "  ",
        "\t",
        "\u{2028}",
        "\u{feff}",
        "...",
        "---",
    ]);
    prop::collection::vec(piece, 0..40)
        .prop_map(|pieces| format!("id: main\ntype: t\nname: n\n{}\n", pieces.concat()))
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

    #[test]
    fn rejects_or_reads_any_relation_like_frontmatter_without_panicking(
        frontmatter in relation_like_frontmatter(),
    ) {
        let expected: ElementId = "main".parse().unwrap();

        let _ = Element::from_markdown(&expected, &format!("---\n{frontmatter}---\n"));
    }
}
