//! Properties of the element file that hold for any element and any input.

use abyme_model::{
    Anchor, CommitId, Element, ElementFields, ElementId, RelationKind, Relations, StatementKind,
    TypeRef, Verification, Verifier,
};
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

/// Anchor text built from segments of the characters YAML treats
/// specially, plain ones, spaces, and non-ASCII; whatever the anchor rules
/// reject is filtered out by the caller.
fn anchor_text() -> impl Strategy<Value = String> {
    let piece = prop::sample::select(vec![
        "a", "z", "0", "-", "_", ".", " ", "\"", "#", ": ", "'", "&", "*", "!", "%", "@", "`", "{",
        "}", "[", "]", ",", "|", ">", "?", "é", "→", "x.md",
    ]);
    let segment = prop::collection::vec(piece, 1..6).prop_map(|pieces| pieces.concat());
    prop::collection::vec(segment, 1..4).prop_map(|segments| segments.join("/"))
}

fn anchor() -> impl Strategy<Value = Anchor> {
    anchor_text().prop_filter_map("an anchor the rules reject", |text| {
        Anchor::try_from(text).ok()
    })
}

fn stamp() -> impl Strategy<Value = Verification> {
    (
        prop_oneof!["[0-9a-f]{40}", "[0-9a-f]{64}"],
        prop::sample::select(vec![Verifier::Ai, Verifier::Person]),
    )
        .prop_filter_map("not a commit ID", |(commit, by)| {
            Some(Verification {
                commit: CommitId::try_from(commit).ok()?,
                by,
            })
        })
}

fn element() -> impl Strategy<Value = Element> {
    (
        element_id(),
        type_ref(),
        allowed_text(),
        proptest::option::of(allowed_text()),
        any::<String>(),
        relations(),
        proptest::option::of(anchor()),
        proptest::option::of(stamp()),
        proptest::option::of(prop::sample::select(StatementKind::ALL.as_slice())),
    )
        .prop_map(
            |(id, type_ref, name, summary, body, relations, anchor, verified, statement)| {
                Element::new(
                    id,
                    ElementFields {
                        type_ref,
                        name,
                        summary,
                        body,
                        relations,
                        anchor,
                        statement,
                        verified,
                    },
                )
            },
        )
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

    #[test]
    fn reads_an_anchor_and_stamp_only_when_they_keep_the_rules(
        anchor in prop_oneof![anchor_text(), any::<String>()],
        verified in prop_oneof!["[0-9a-f]{40}", "[0-9a-f]{64}", "[0-9a-fA-F]{38,66}", any::<String>()],
        verified_by in prop_oneof![Just("ai".to_owned()), Just("person".to_owned()), any::<String>()],
    ) {
        let expected: ElementId = "main".parse().unwrap();
        let quoted = |text: &str| format!("{text:?}");
        let text = format!(
            "---\nid: main\ntype: t\nname: n\nanchor: {}\nverified: {}\nverified_by: {}\n---\n",
            quoted(&anchor),
            quoted(&verified),
            quoted(&verified_by),
        );
        let keeps_the_rules = Anchor::try_from(anchor.clone()).is_ok()
            && CommitId::try_from(verified.clone()).is_ok()
            && Verifier::from_name(&verified_by).is_some();

        let read = Element::from_markdown(&expected, &text);

        // Rust's `{:?}` writes what YAML's double quotes read back, except
        // the `\u{..}` escapes, which YAML rejects.
        if keeps_the_rules && !text.contains("\\u{") {
            prop_assert!(read.is_ok());
        }
        if let Ok(element) = read {
            prop_assert_eq!(element.anchor().map(Anchor::as_str), Some(anchor.as_str()));
            let stamp = element.verified();
            prop_assert_eq!(stamp.map(|stamp| stamp.commit.as_str()), Some(verified.as_str()));
            prop_assert_eq!(stamp.map(|stamp| stamp.by.as_str()), Some(verified_by.as_str()));
        }
    }
}
