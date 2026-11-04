//! Tests of deleting elements.

use super::*;

/// `snapshot` with `source` relating to `target` with a secret label.
fn with_relation(
    mut snapshot: Snapshot,
    source: &str,
    kind: RelationKind,
    target: &str,
) -> Snapshot {
    let element = snapshot
        .elements
        .remove(&element_id(source))
        .unwrap_or_else(|| element(source));
    let mut fields = element.into_fields();
    fields
        .relations
        .insert((kind, element_id(target)), Some("SECRET-LABEL".to_owned()));
    "SECRET-NAME".clone_into(&mut fields.name);
    snapshot.elements.insert(
        element_id(source),
        Element::new(element_id(source), fields).unwrap(),
    );
    snapshot
}

#[test]
fn deletes_an_element_with_its_file_and_its_placement_on_every_board() {
    let json = r#"[{"op": "delete_element", "element": {"existing": "orders-db"}}]"#;

    let applied = apply(&snapshot_for_deletion(), &operations(json)).unwrap();

    assert!(matches!(
        applied.writes.as_slice(),
        [Write::Board(_), Write::Board(_), Write::RemoveElement(_)]
    ));
    assert_eq!(removals(&applied), ["orders-db"]);
    for board in ["main", "side"] {
        assert!(
            !written_board(&applied, board)
                .placements
                .contains_key(&element_id("orders-db"))
        );
    }
    assert!(
        written_board(&applied, "main")
            .placements
            .contains_key(&element_id("ghost"))
    );
}

#[test]
fn deletes_an_unplaced_element_with_only_its_file() {
    let json = r#"[{"op": "delete_element", "element": {"existing": "loose"}}]"#;

    let applied = apply(&snapshot(), &operations(json)).unwrap();

    assert_eq!(
        applied.writes,
        vec![Write::RemoveElement(element_id("loose"))]
    );
}

#[test]
fn deletes_a_placement_whose_file_is_missing_without_removing_a_file() {
    let json = r#"[{"op": "delete_element", "element": {"existing": "ghost"}}]"#;

    let applied = apply(&snapshot(), &operations(json)).unwrap();

    assert_eq!(removals(&applied), Vec::<&str>::new());
    let main = written_board(&applied, "main");
    assert!(!main.placements.contains_key(&element_id("ghost")));
    assert!(main.placements.contains_key(&element_id("orders-db")));
}

#[test]
fn deletes_an_element_created_in_the_same_request_without_writing_it() {
    let json = r#"[
        {"op": "create_element", "key": "e", "type_ref": "t", "name": "Fresh"},
        {"op": "place_element", "element": {"new": "e"}, "board": {"existing": "main"}},
        {"op": "delete_element", "element": {"new": "e"}}
    ]"#;

    let applied = apply(&snapshot(), &operations(json)).unwrap();

    assert_eq!(written_elements(&applied), Vec::<&Element>::new());
    assert_eq!(removals(&applied), Vec::<&str>::new());
    assert!(
        !written_board(&applied, "main")
            .placements
            .contains_key(&element_id("fresh"))
    );
}

#[test]
fn rejects_an_element_that_others_still_relate_to_and_shows_only_ids_and_kinds() {
    let snapshot = with_relation(snapshot(), "loose", RelationKind::Uses, "orders-db");
    let json = r#"[{"op": "delete_element", "element": {"existing": "orders-db"}}]"#;

    let error = apply(&snapshot, &operations(json)).unwrap_err();

    assert_eq!(
        error,
        ApplyError::Operation {
            position: 1,
            problem: OperationProblem::StillRelated {
                element: element_id("orders-db"),
                sources: RelationSources(vec![(element_id("loose"), RelationKind::Uses)]),
            }
        }
    );
    let message = error.to_string();
    assert_eq!(
        message,
        "operation 1: element orders-db is still the target of relations from loose (uses); delete those first"
    );
    assert!(!message.contains("SECRET"), "{message}");
}

#[test]
fn names_at_most_ten_sources_and_counts_the_rest() {
    let mut snapshot = snapshot();
    for index in 0..12 {
        snapshot = with_relation(
            snapshot,
            &format!("s{index:02}"),
            RelationKind::DependsOn,
            "orders-db",
        );
    }
    let json = r#"[{"op": "delete_element", "element": {"existing": "orders-db"}}]"#;

    let message = apply(&snapshot, &operations(json)).unwrap_err().to_string();

    let shown: Vec<String> = (0..10)
        .map(|index| format!("s{index:02} (depends_on)"))
        .collect();
    assert!(
        message.contains(&format!("{}, and 2 more;", shown.join(", "))),
        "{message}"
    );
    assert!(!message.contains("s10"), "{message}");
}

#[rstest]
#[case::relation_deleted_first(
    r#"[
        {"op": "delete_relation", "from": {"existing": "loose"}, "kind": "uses", "to": {"existing": "orders-db"}},
        {"op": "delete_element", "element": {"existing": "orders-db"}}
    ]"#
)]
#[case::source_deleted_first(
    r#"[
        {"op": "delete_element", "element": {"existing": "loose"}},
        {"op": "delete_element", "element": {"existing": "orders-db"}}
    ]"#
)]
fn deletes_an_element_once_the_request_removed_what_related_to_it(#[case] json: &str) {
    let snapshot = with_relation(snapshot(), "loose", RelationKind::Uses, "orders-db");

    let applied = apply(&snapshot, &operations(json)).unwrap();

    assert!(removals(&applied).contains(&"orders-db"));
}

#[test]
fn rejects_a_deletion_while_a_new_element_relates_to_it() {
    let json = r#"[
        {"op": "create_element", "key": "b", "type_ref": "t", "name": "B"},
        {"op": "create_relation", "from": {"new": "b"}, "kind": "uses", "to": {"existing": "loose"}},
        {"op": "delete_element", "element": {"existing": "loose"}}
    ]"#;

    assert_eq!(
        problem(&snapshot(), json),
        OperationProblem::StillRelated {
            element: element_id("loose"),
            sources: RelationSources(vec![(element_id("b"), RelationKind::Uses)]),
        }
    );
}

#[test]
fn rejects_deleting_a_placement_without_a_file_while_an_element_relates_to_it() {
    let snapshot = with_relation(snapshot(), "loose", RelationKind::Concerns, "ghost");

    assert_eq!(
        problem(
            &snapshot,
            r#"[{"op": "delete_element", "element": {"existing": "ghost"}}]"#
        ),
        OperationProblem::StillRelated {
            element: element_id("ghost"),
            sources: RelationSources(vec![(element_id("loose"), RelationKind::Concerns)]),
        }
    );
}

#[rstest]
#[case::update(r#"{"op": "update_element", "element": {"existing": "loose"}, "name": "x"}"#)]
#[case::relation_from(
    r#"{"op": "create_relation", "from": {"existing": "loose"}, "kind": "uses", "to": {"existing": "orders-db"}}"#
)]
#[case::relation_to(
    r#"{"op": "create_relation", "from": {"existing": "orders-db"}, "kind": "uses", "to": {"existing": "loose"}}"#
)]
#[case::relation_deleted(
    r#"{"op": "delete_relation", "from": {"existing": "loose"}, "kind": "uses", "to": {"existing": "orders-db"}}"#
)]
#[case::placement(
    r#"{"op": "place_element", "element": {"existing": "loose"}, "board": {"existing": "main"}}"#
)]
#[case::move_(r#"{"op": "move_element", "element": {"existing": "loose"}, "board": {"existing": "main"}, "x": 0, "y": 0}"#)]
#[case::deletion_again(r#"{"op": "delete_element", "element": {"existing": "loose"}}"#)]
fn knows_a_deleted_element_no_longer(#[case] operation: &str) {
    let json =
        format!(r#"[{{"op": "delete_element", "element": {{"existing": "loose"}}}}, {operation}]"#);

    assert_eq!(
        problem(&snapshot(), &json),
        OperationProblem::UnknownElement(element_id("loose"))
    );
}

#[test]
fn rejects_deleting_an_unknown_element() {
    assert_eq!(
        problem(
            &snapshot(),
            r#"[{"op": "delete_element", "element": {"existing": "nope"}}]"#
        ),
        OperationProblem::UnknownElement(element_id("nope"))
    );
}
