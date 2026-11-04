//! Tests of the IDs that new elements and boards get.

use super::*;

#[test]
fn gives_colliding_names_suffixes() {
    let json = r#"[
        {"op": "create_element", "key": "one", "type_ref": "t", "name": "Orders DB"},
        {"op": "create_element", "key": "two", "type_ref": "t", "name": "Orders DB"}
    ]"#;

    let applied = apply(&snapshot(), &operations(json)).unwrap();

    assert_eq!(
        applied.created,
        vec![
            (key("one"), Created::Element(element_id("orders-db-2"))),
            (key("two"), Created::Element(element_id("orders-db-3"))),
        ]
    );
}

#[test]
fn gives_colliding_titles_suffixes() {
    let json = r#"[
        {"op": "create_board", "key": "one", "title": "Main"},
        {"op": "create_board", "key": "two", "title": "Main"}
    ]"#;

    let applied = apply(&snapshot(), &operations(json)).unwrap();

    assert_eq!(
        applied.created,
        vec![
            (key("one"), Created::Board(board_id("main-2"))),
            (key("two"), Created::Board(board_id("main-3"))),
        ]
    );
    assert_eq!(written_board(&applied, "main-2").title, "Main");
    assert_eq!(written_board(&applied, "main-3").title, "Main");
    assert_eq!(applied.writes.len(), 2);
}

#[test]
fn never_takes_over_an_orphan_placement() {
    let json = r#"[{"op": "create_element", "key": "g", "type_ref": "t", "name": "Ghost"}]"#;

    let applied = apply(&snapshot(), &operations(json)).unwrap();

    assert_eq!(
        applied.created,
        vec![(key("g"), Created::Element(element_id("ghost-2")))]
    );
}

#[test]
fn a_note_without_a_letter_or_digit_in_its_name_gets_the_id_note() {
    let json = r#"[
        {"op": "create_element", "key": "a", "type_ref": "note", "name": "?!"},
        {"op": "create_element", "key": "b", "type_ref": "note", "name": "…"}
    ]"#;

    let applied = apply(&snapshot(), &operations(json)).unwrap();

    assert_eq!(
        applied.created,
        vec![
            (key("a"), Created::Element(element_id("note"))),
            (key("b"), Created::Element(element_id("note-2"))),
        ]
    );
}

#[rstest]
#[case::other_type("t")]
#[case::capitalized("Note")]
#[case::upper_case("NOTE")]
#[case::longer("notes")]
#[case::in_a_namespace("c4/note")]
#[case::with_a_space("note ")]
fn another_type_keeps_the_fallback_element(#[case] type_ref: &str) {
    let json = format!(
        r#"[{{"op": "create_element", "key": "a", "type_ref": "{type_ref}", "name": "?!"}}]"#
    );

    let applied = apply(&snapshot(), &operations(&json)).unwrap();

    assert_eq!(
        applied.created,
        vec![(key("a"), Created::Element(element_id("element")))]
    );
}

#[test]
fn shortens_a_long_name_to_a_valid_slug() {
    let json = format!(
        r#"[
            {{"op": "create_element", "key": "a", "type_ref": "t", "name": "{long}"}},
            {{"op": "create_element", "key": "b", "type_ref": "t", "name": "{long}"}}
        ]"#,
        long = "x".repeat(100)
    );

    let applied = apply(&snapshot(), &operations(&json)).unwrap();

    assert_eq!(
        applied.created,
        vec![
            (key("a"), Created::Element(element_id(&"x".repeat(64)))),
            (
                key("b"),
                Created::Element(element_id(&format!("{}-2", "x".repeat(62))))
            ),
        ]
    );
}

#[test]
fn gives_an_element_created_after_a_rename_no_taken_id() {
    let json = r#"[
        {"op": "update_element", "element": {"existing": "loose"}, "name": "Other"},
        {"op": "create_element", "key": "e", "type_ref": "t", "name": "Loose"}
    ]"#;

    let applied = apply(&snapshot(), &operations(json)).unwrap();

    assert_eq!(
        applied.created,
        vec![(key("e"), Created::Element(element_id("loose-2")))]
    );
}

#[test]
fn keeps_the_id_of_a_deleted_element_taken_for_the_rest_of_the_request() {
    let json = r#"[
        {"op": "delete_element", "element": {"existing": "loose"}},
        {"op": "create_element", "key": "e", "type_ref": "t", "name": "Loose"}
    ]"#;

    let applied = apply(&snapshot(), &operations(json)).unwrap();

    assert_eq!(
        applied.created,
        vec![(key("e"), Created::Element(element_id("loose-2")))]
    );
    assert_eq!(removals(&applied), ["loose"]);
}
