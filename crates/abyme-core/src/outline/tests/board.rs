//! Tests of a board's outline as a whole.

use super::*;

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
    let error = outline(
        &Snapshot::default(),
        &"nowhere".parse().unwrap(),
        Geometry::Hidden,
        None,
    )
    .unwrap_err();

    assert_eq!(error.to_string(), "no board has the ID nowhere");
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

/// A label made often of the characters that break, hide, or reorder text.
fn arbitrary_label() -> impl Strategy<Value = String> {
    let dangerous = prop::sample::select(vec![
        '\n',
        '\r',
        '\t',
        '\u{85}',
        '\u{2028}',
        '\u{2029}',
        '\u{202e}',
        '\u{2066}',
        '\u{feff}',
        '\u{e0041}',
        '\\',
        'a',
        '→',
        ' ',
    ]);
    prop_oneof![
        any::<String>(),
        prop::collection::vec(dangerous, 0..8)
            .prop_map(|characters| characters.into_iter().collect()),
    ]
}

/// Relations of element `index` to the elements `e0` to `e7`, of which only
/// the ones a test generates lie on the board.
fn arbitrary_relations(index: usize) -> impl Strategy<Value = Relations> {
    let kind = prop::sample::select(vec![
        RelationKind::Concerns,
        RelationKind::DependsOn,
        RelationKind::Describes,
        RelationKind::SeeAlso,
        RelationKind::Uses,
    ]);
    let target = (0..8_usize)
        .prop_filter("a relation to itself", move |target| *target != index)
        .prop_filter_map("invalid ID", |target| format!("e{target}").parse().ok());
    prop::collection::btree_map(
        (kind, target),
        proptest::option::of(arbitrary_label()),
        0..3,
    )
}

/// An element with arbitrary type, name, summary, and relations, if the
/// element file can hold them.
fn arbitrary_element(index: usize) -> impl Strategy<Value = Option<Element>> {
    (
        any::<String>(),
        any::<String>(),
        proptest::option::of(any::<String>()),
        arbitrary_relations(index),
    )
        .prop_map(move |(type_ref, name, summary, relations)| {
            let type_ref = TypeRef::try_from(type_ref).ok()?;
            let id = format!("e{index}").parse().ok()?;
            Element::new(
                id,
                ElementFields {
                    statement: None,
                    type_ref,
                    name,
                    summary,
                    body: String::new(),
                    relations,
                    anchor: None,
                    verified: None,
                },
            )
            .ok()
        })
}

proptest! {
    #[test]
    fn writes_one_line_per_element_and_relation_without_any_escaped_character(
        title in any::<String>(),
        elements in (0..6_usize).prop_flat_map(|count| {
            (0..count).map(arbitrary_element).collect::<Vec<_>>()
        }),
    ) {
        let elements: Vec<Element> = elements.into_iter().flatten().collect();
        let count = elements.len();
        let placed: Vec<&ElementId> = elements.iter().map(Element::id).collect();
        let relations: usize = elements
            .iter()
            .map(|element| {
                element
                    .relations()
                    .keys()
                    .filter(|(_, to)| placed.contains(&to))
                    .count()
            })
            .sum();
        let expected = 2 + count + if relations > 0 { 1 + relations } else { 0 };
        let snapshot = snapshot(&title, elements, &[]);

        let outline = outline_of(&snapshot);

        prop_assert_eq!(outline.lines().count(), expected);
        prop_assert_eq!(outline.matches('\n').count(), expected);
        let inner = outline.replace('\n', "");
        prop_assert!(
            !inner.chars().any(hides_or_breaks_text),
            "{:?}",
            outline
        );
    }
}
