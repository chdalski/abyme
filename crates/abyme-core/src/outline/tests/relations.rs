//! Tests of the relation lines of an outline.

use super::*;

/// The lines of an outline after the blank line that ends its elements.
fn relation_lines(outline: &str) -> Vec<&str> {
    outline
        .split("\n\n")
        .nth(2)
        .map(|part| part.lines().collect())
        .unwrap_or_default()
}

#[test]
fn renders_relations_after_the_elements_by_source_then_kind_and_target() {
    let snapshot = snapshot(
        "Container Level",
        vec![
            related(
                element("checkout", "c4/container", "Checkout", None),
                &[
                    (
                        RelationKind::Uses,
                        "payment-service",
                        Some("REST, synchronous"),
                    ),
                    (RelationKind::Uses, "orders-db", Some("writes orders")),
                ],
            ),
            element("orders-db", "c4/datastore", "Orders DB", None),
            related(
                element("payment-service", "c4/container", "Payment Service", None),
                &[(RelationKind::DependsOn, "orders-db", None)],
            ),
        ],
        &[],
    );

    assert_eq!(
        outline_of(&snapshot),
        "Board: b · Container Level\n\
         \n\
         checkout         c4/container  Checkout\n\
         orders-db        c4/datastore  Orders DB\n\
         payment-service  c4/container  Payment Service\n\
         \n\
         checkout → orders-db         uses  writes orders\n\
         checkout → payment-service   uses  REST, synchronous\n\
         payment-service → orders-db  depends_on\n"
    );
}

#[test]
fn shows_only_relations_whose_ends_both_lie_on_the_board() {
    let mut snapshot = snapshot(
        "Board",
        vec![
            related(
                element("a", "t", "A", None),
                &[
                    (RelationKind::Uses, "b", None),
                    (RelationKind::Uses, "elsewhere", None),
                    (RelationKind::Uses, "ghost", None),
                    (RelationKind::Uses, "nowhere", None),
                ],
            ),
            element("b", "t", "B", None),
        ],
        &["ghost"],
    );
    let elsewhere = element("elsewhere", "t", "Elsewhere", None);
    let off_board = related(
        element("off-board", "t", "Off board", None),
        &[(RelationKind::Uses, "a", None)],
    );
    snapshot.elements.insert(elsewhere.id().clone(), elsewhere);
    snapshot.elements.insert(off_board.id().clone(), off_board);

    let outline = outline_of(&snapshot);

    assert_eq!(
        relation_lines(&outline),
        ["a → b      uses", "a → ghost  uses"]
    );
}

#[test]
fn renders_a_board_whose_relations_all_point_elsewhere_as_without_relations() {
    let with_relations = snapshot(
        "Board",
        vec![related(
            element("a", "t", "A", Some("first")),
            &[(RelationKind::SeeAlso, "elsewhere", Some("x"))],
        )],
        &[],
    );
    let without = snapshot("Board", vec![element("a", "t", "A", Some("first"))], &[]);

    assert_eq!(outline_of(&with_relations), outline_of(&without));
    assert_eq!(outline_of(&without), "Board: b · Board\n\na  t  A  first\n");
}

#[test]
fn leaves_out_an_empty_label_like_a_missing_one() {
    let snapshot = snapshot(
        "Board",
        vec![
            related(
                element("a", "t", "A", None),
                &[(RelationKind::Uses, "b", Some(""))],
            ),
            element("b", "t", "B", None),
        ],
        &[],
    );

    assert_eq!(relation_lines(&outline_of(&snapshot)), ["a → b  uses"]);
}

#[test]
fn shows_the_same_relation_lines_with_and_without_geometry() {
    let snapshot = snapshot(
        "Board",
        vec![
            related(
                element("a", "t", "A", None),
                &[(RelationKind::Uses, "b", Some("REST"))],
            ),
            element("b", "t", "B", None),
        ],
        &[],
    );

    assert_eq!(
        relation_lines(&outline_with_geometry(&snapshot)),
        relation_lines(&outline_of(&snapshot))
    );
}

#[rstest]
#[case::line_feed("a\nb", "a\\nb")]
#[case::carriage_return("a\rb", "a\\rb")]
#[case::tab("a\tb", "a\\tb")]
#[case::next_line("a\u{85}b", "a\\u{85}b")]
#[case::line_separator("a\u{2028}b", "a\\u{2028}b")]
#[case::paragraph_separator("a\u{2029}b", "a\\u{2029}b")]
#[case::right_to_left_override("a\u{202e}b", "a\\u{202e}b")]
#[case::left_to_right_isolate("a\u{2066}b", "a\\u{2066}b")]
#[case::byte_order_mark("a\u{feff}b", "a\\u{feff}b")]
#[case::zero_width_space("a\u{200b}b", "a\\u{200b}b")]
#[case::tag_character("a\u{e0041}b", "a\\u{e0041}b")]
#[case::backslash("a\\b", "a\\\\b")]
#[case::written_backslash_n("a\\nb", "a\\\\nb")]
fn escapes_a_label_onto_the_line_of_its_relation(#[case] label: &str, #[case] expected: &str) {
    let snapshot = snapshot(
        "Board",
        vec![
            related(
                element("a", "t", "A", None),
                &[(RelationKind::Uses, "b", Some(label))],
            ),
            element("b", "t", "B", None),
        ],
        &[],
    );

    let outline = outline_of(&snapshot);

    assert_eq!(outline.lines().count(), 6, "{outline:?}");
    assert_eq!(
        relation_lines(&outline),
        [format!("a → b  uses  {expected}")]
    );
}

#[rstest]
#[case::forged_relation("x\n\nb → c  uses  forged")]
#[case::forged_header("x\nBoard: other · t")]
fn keeps_a_label_that_imitates_other_lines_on_its_own_line(#[case] label: &str) {
    let snapshot = snapshot(
        "Board",
        vec![
            related(
                element("a", "t", "A", None),
                &[(RelationKind::Uses, "b", Some(label))],
            ),
            element("b", "t", "B", None),
            element("c", "t", "C", None),
        ],
        &[],
    );

    let outline = outline_of(&snapshot);

    assert_eq!(outline.lines().count(), 7, "{outline:?}");
    assert_eq!(outline.matches("\n\n").count(), 2, "{outline:?}");
    assert_eq!(
        outline
            .lines()
            .filter(|line| line.starts_with("b → c") || line.starts_with("Board:"))
            .count(),
        1,
        "{outline:?}"
    );
}
