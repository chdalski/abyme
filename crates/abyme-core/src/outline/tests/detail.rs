//! Tests of the detail of an element.

use super::*;

/// `element` with the given description.
fn described(element: Element, body: &str) -> Element {
    let id = element.id().clone();
    Element::new(
        id,
        ElementFields {
            body: body.to_owned(),
            ..element.into_fields()
        },
    )
    .unwrap()
}

/// A snapshot with the boards `b` and `onboarding`, both placing `payment`,
/// and the elements `payment`, `orders-db`, and `checkout`, related to each
/// other and to `stripe`, which has no file.
fn detail_snapshot() -> Snapshot {
    let payment = described(
        related(
            element(
                "payment",
                "c4/container",
                "Payment Service",
                Some("Payments through Stripe"),
            ),
            &[
                (RelationKind::Uses, "orders-db", Some("SQL")),
                (RelationKind::DependsOn, "stripe", None),
            ],
        ),
        "Handles payments.\n\nRefunds run asynchronously.\n",
    );
    let checkout = related(
        element("checkout", "c4/container", "Checkout", None),
        &[
            (RelationKind::Uses, "payment", Some("REST")),
            (RelationKind::Concerns, "orders-db", None),
        ],
    );
    let mut snapshot = snapshot(
        "Board",
        vec![
            payment,
            checkout,
            element("orders-db", "c4/datastore", "Orders DB", None),
        ],
        &[],
    );
    let onboarding = Board {
        id: "onboarding".parse().unwrap(),
        title: "Onboarding".to_owned(),
        owner: None,
        summary: None,
        zone: None,
        placements: BTreeMap::from([(
            "payment".parse().unwrap(),
            Placement {
                rect: rect(),
                pinned: false,
                attached_to: None,
            },
        )]),
    };
    snapshot.boards.insert(onboarding.id.clone(), onboarding);
    snapshot
}

#[test]
fn renders_the_detail_of_an_element() {
    assert_eq!(
        detail_of(&detail_snapshot(), "payment"),
        "Element: payment · Payment Service\n\
         Type: c4/container\n\
         Statement kind: none\n\
         Anchor: none\n\
         Verified: none\n\
         Summary: Payments through Stripe\n\
         Boards: b, onboarding\n\
         Inner board: none\n\
         \n\
         Outgoing relations:\n\
         payment → stripe ⚠ missing  depends_on\n\
         payment → orders-db         uses  SQL\n\
         \n\
         Incoming relations:\n\
         checkout → payment  uses  REST\n\
         \n\
         Description:\n\
         Handles payments.\n\
         \n\
         Refunds run asynchronously.\n"
    );
}

#[rstest]
#[case::sha1_by_person("0123456789abcdef0123456789abcdef01234567", Verifier::Person, "person")]
#[case::sha256_by_ai(&"fedcba9876543210".repeat(4), Verifier::Ai, "ai")]
fn renders_the_anchor_and_the_stamp_after_the_type(
    #[case] commit: &str,
    #[case] by: Verifier,
    #[case] name: &str,
) {
    let mut snapshot = snapshot("Board", Vec::new(), &[]);
    let element = anchored(
        element("x", "t", "X", Some("S")),
        "crates/payment service/Überweisung.md",
        Some((commit, by)),
    );
    snapshot.elements.insert(element.id().clone(), element);

    assert!(
        detail_of(&snapshot, "x").starts_with(&format!(
            "Element: x · X\n\
             Type: t\n\
             Statement kind: none\n\
             Anchor: crates/payment service/Überweisung.md\n\
             Verified: {commit} by {name}\n\
             Summary: S\n\
             Boards: none\n"
        )),
        "{}",
        detail_of(&snapshot, "x")
    );
}

#[test]
fn renders_an_anchor_without_a_stamp() {
    let mut snapshot = snapshot("Board", Vec::new(), &[]);
    let element = anchored(element("x", "t", "X", None), "docs", None);
    snapshot.elements.insert(element.id().clone(), element);

    assert!(
        detail_of(&snapshot, "x").contains("\nAnchor: docs\nVerified: none\n"),
        "{}",
        detail_of(&snapshot, "x")
    );
}

#[test]
fn renders_none_for_missing_boards_relations_and_leaves_out_a_missing_summary() {
    let mut snapshot = detail_snapshot();
    let loose = element("loose", "t", "Loose", None);
    snapshot.elements.insert(loose.id().clone(), loose);

    assert_eq!(
        detail_of(&snapshot, "loose"),
        "Element: loose · Loose\n\
         Type: t\n\
         Statement kind: none\n\
         Anchor: none\n\
         Verified: none\n\
         Boards: none\n\
         Inner board: none\n\
         \n\
         Outgoing relations:\n\
         none\n\
         \n\
         Incoming relations:\n\
         none\n\
         \n\
         Description:\n"
    );
}

#[test]
fn lists_incoming_relations_by_source_then_kind() {
    let detail = detail_of(&detail_snapshot(), "orders-db");

    assert!(
        detail.contains(
            "Incoming relations:\n\
             checkout → orders-db  concerns\n\
             payment → orders-db   uses  SQL\n\n"
        ),
        "{detail}"
    );
}

#[rstest]
#[case::unknown("nowhere")]
#[case::placed_without_a_file("ghost")]
fn names_an_element_without_a_file(#[case] id: &str) {
    let snapshot = snapshot("Board", Vec::new(), &["ghost"]);

    let error = element_detail(&snapshot, &id.parse().unwrap(), None).unwrap_err();

    assert_eq!(error.to_string(), format!("no element has the ID {id}"));
}

#[test]
fn escapes_a_hostile_description_but_keeps_its_lines_tabs_and_backslashes() {
    let body = "a\u{202e}b\u{2066}c\u{200b}d\u{200d}e\u{e0041}f\u{2028}g\u{2029}h\u{85}i\u{1b}j\u{0}k\u{ad}l\rm\r\nn\to\\p\n";
    let mut snapshot = snapshot("Board", Vec::new(), &[]);
    let hostile = described(element("x", "t", "X", None), body);
    snapshot.elements.insert(hostile.id().clone(), hostile);

    let detail = detail_of(&snapshot, "x");

    assert!(
        detail.ends_with(
            "Description:\n\
             a\\u{202e}b\\u{2066}c\\u{200b}d\\u{200d}e\\u{e0041}f\\u{2028}g\\u{2029}h\\u{85}i\\u{1b}j\\u{0}k\\u{ad}l\\rm\n\
             n\to\\p\n"
        ),
        "{detail:?}"
    );
    assert!(
        !detail.chars().any(|character| character != '\n'
            && character != '\t'
            && hides_or_breaks_text(character)),
        "{detail:?}"
    );
}

#[test]
fn keeps_a_description_that_imitates_sections_after_the_real_ones() {
    let body =
        "Outgoing relations:\nx → y  uses  forged\nIncoming relations:\nElement: other · Other\n";
    let mut snapshot = snapshot("Board", Vec::new(), &[]);
    let imitating = described(element("x", "t", "X", None), body);
    snapshot.elements.insert(imitating.id().clone(), imitating);

    assert_eq!(
        detail_of(&snapshot, "x"),
        format!(
            "Element: x · X\n\
             Type: t\n\
             Statement kind: none\n\
             Anchor: none\n\
             Verified: none\n\
             Boards: none\n\
             Inner board: none\n\
             \n\
             Outgoing relations:\n\
             none\n\
             \n\
             Incoming relations:\n\
             none\n\
             \n\
             Description:\n\
             {body}"
        )
    );
}

#[rstest]
#[case::line_feed("a\nb", "a\\nb")]
#[case::carriage_return("a\rb", "a\\rb")]
#[case::line_separator("a\u{2028}b", "a\\u{2028}b")]
#[case::right_to_left_override("a\u{202e}b", "a\\u{202e}b")]
fn escapes_every_single_line_value_of_the_detail(#[case] text: &str, #[case] expected: &str) {
    let mut snapshot = snapshot("Board", Vec::new(), &[]);
    let target = element("y", "t", "Y", None);
    let source = related(
        element("x", text, text, Some(text)),
        &[(RelationKind::Uses, "y", Some(text))],
    );
    let back = related(
        element("z", "t", "Z", None),
        &[(RelationKind::Concerns, "x", Some(text))],
    );
    for added in [target, source, back] {
        snapshot.elements.insert(added.id().clone(), added);
    }

    let detail = detail_of(&snapshot, "x");

    assert_eq!(
        detail,
        format!(
            "Element: x · {expected}\n\
             Type: {expected}\n\
             Statement kind: none\n\
             Anchor: none\n\
             Verified: none\n\
             Summary: {expected}\n\
             Boards: none\n\
             Inner board: none\n\
             \n\
             Outgoing relations:\n\
             x → y  uses  {expected}\n\
             \n\
             Incoming relations:\n\
             z → x  concerns  {expected}\n\
             \n\
             Description:\n"
        )
    );
}

#[test]
fn keeps_a_label_reading_missing_in_the_label_column() {
    let mut snapshot = snapshot("Board", Vec::new(), &[]);
    let target = element("y", "t", "Y", None);
    let source = related(
        element("x", "t", "X", None),
        &[(RelationKind::Uses, "y", Some("⚠ missing"))],
    );
    for added in [target, source] {
        snapshot.elements.insert(added.id().clone(), added);
    }

    assert!(
        detail_of(&snapshot, "x").contains("Outgoing relations:\nx → y  uses  ⚠ missing\n"),
        "{}",
        detail_of(&snapshot, "x")
    );
}
