//! Tests of creating and deleting relations.

use super::*;

#[test]
fn creates_a_relation_and_writes_only_its_source() {
    let json = r#"[{"op": "create_relation", "from": {"existing": "loose"}, "kind": "uses", "to": {"existing": "orders-db"}, "label": "SQL"}]"#;

    let applied = apply(&snapshot(), &operations(json)).unwrap();

    let written = written_elements(&applied);
    assert_eq!(written.len(), 1);
    let source = written.first().unwrap();
    assert_eq!(source.id(), &element_id("loose"));
    assert_eq!(
        relations_of(source),
        [(RelationKind::Uses, "orders-db", Some("SQL"))]
    );
    assert_eq!(applied.writes.len(), 1);
}

#[test]
fn writes_a_source_with_two_new_relations_once() {
    let json = r#"[
        {"op": "create_relation", "from": {"existing": "loose"}, "kind": "uses", "to": {"existing": "orders-db"}},
        {"op": "create_relation", "from": {"existing": "loose"}, "kind": "concerns", "to": {"existing": "orders-db"}}
    ]"#;

    let applied = apply(&snapshot(), &operations(json)).unwrap();

    let written = written_elements(&applied);
    assert_eq!(written.len(), 1);
    assert_eq!(
        relations_of(written.first().unwrap()),
        [
            (RelationKind::Concerns, "orders-db", None),
            (RelationKind::Uses, "orders-db", None)
        ]
    );
}

#[test]
fn writes_a_created_element_once_with_its_relations_before_changed_ones_and_boards() {
    let json = r#"[
        {"op": "create_element", "key": "pay", "type_ref": "t", "name": "Payment"},
        {"op": "create_relation", "from": {"new": "pay"}, "kind": "depends_on", "to": {"existing": "orders-db"}},
        {"op": "create_relation", "from": {"existing": "loose"}, "kind": "see_also", "to": {"new": "pay"}},
        {"op": "place_element", "element": {"new": "pay"}, "board": {"existing": "main"}}
    ]"#;

    let applied = apply(&snapshot(), &operations(json)).unwrap();

    assert!(matches!(
        applied.writes.as_slice(),
        [Write::Element(_), Write::Element(_), Write::Board(_)]
    ));
    let written = written_elements(&applied);
    let ids: Vec<&str> = written
        .iter()
        .map(|element| element.id().as_str())
        .collect();
    assert_eq!(ids, ["payment", "loose"]);
    assert_eq!(
        relations_of(written.first().unwrap()),
        [(RelationKind::DependsOn, "orders-db", None)]
    );
}

#[test]
fn deletes_a_relation() {
    let mut snapshot = snapshot();
    let json = r#"[{"op": "create_relation", "from": {"existing": "loose"}, "kind": "uses", "to": {"existing": "orders-db"}}]"#;
    let source = written_elements(&apply(&snapshot, &operations(json)).unwrap())
        .first()
        .copied()
        .cloned()
        .unwrap();
    snapshot.elements.insert(element_id("loose"), source);
    let json = r#"[{"op": "delete_relation", "from": {"existing": "loose"}, "kind": "uses", "to": {"existing": "orders-db"}}]"#;

    let applied = apply(&snapshot, &operations(json)).unwrap();

    let written = written_elements(&applied);
    assert_eq!(written.len(), 1);
    assert_eq!(relations_of(written.first().unwrap()), []);
}

#[test]
fn deletes_a_relation_whose_target_file_is_gone() {
    let mut snapshot = snapshot();
    let source = Element::new(
        element_id("loose"),
        ElementFields {
            relations: Relations::from([((RelationKind::Uses, element_id("gone")), None)]),
            ..element("loose").into_fields()
        },
    )
    .unwrap();
    snapshot.elements.insert(element_id("loose"), source);
    let json = r#"[{"op": "delete_relation", "from": {"existing": "loose"}, "kind": "uses", "to": {"existing": "gone"}}]"#;

    let applied = apply(&snapshot, &operations(json)).unwrap();

    assert_eq!(
        relations_of(written_elements(&applied).first().unwrap()),
        []
    );
}

#[rstest]
#[case::unknown_source(
    r#"[{"op": "create_relation", "from": {"existing": "nope"}, "kind": "uses", "to": {"existing": "loose"}}]"#,
    OperationProblem::UnknownElement(element_id("nope"))
)]
#[case::unknown_target(
    r#"[{"op": "create_relation", "from": {"existing": "loose"}, "kind": "uses", "to": {"existing": "nope"}}]"#,
    OperationProblem::UnknownElement(element_id("nope"))
)]
#[case::target_without_a_file(
    r#"[{"op": "create_relation", "from": {"existing": "loose"}, "kind": "uses", "to": {"existing": "ghost"}}]"#,
    OperationProblem::UnknownElement(element_id("ghost"))
)]
#[case::board_key_as_target(
    r#"[
        {"op": "create_board", "key": "b", "title": "B"},
        {"op": "create_relation", "from": {"existing": "loose"}, "kind": "uses", "to": {"new": "b"}}
    ]"#,
    OperationProblem::WrongKind { key: key("b"), expected: Kind::Element, actual: Kind::Board }
)]
#[case::board_key_as_target_of_a_deletion(
    r#"[
        {"op": "create_board", "key": "b", "title": "B"},
        {"op": "delete_relation", "from": {"existing": "loose"}, "kind": "uses", "to": {"new": "b"}}
    ]"#,
    OperationProblem::WrongKind { key: key("b"), expected: Kind::Element, actual: Kind::Board }
)]
#[case::relation_twice(
    r#"[
        {"op": "create_relation", "from": {"existing": "loose"}, "kind": "uses", "to": {"existing": "orders-db"}, "label": "A"},
        {"op": "create_relation", "from": {"existing": "loose"}, "kind": "uses", "to": {"existing": "orders-db"}, "label": "B"}
    ]"#,
    OperationProblem::RelationExists { from: element_id("loose"), kind: RelationKind::Uses, to: element_id("orders-db") }
)]
#[case::relation_to_itself(
    r#"[{"op": "create_relation", "from": {"existing": "loose"}, "kind": "uses", "to": {"existing": "loose"}}]"#,
    OperationProblem::InvalidElement(ElementProblem::SelfRelation { kind: RelationKind::Uses })
)]
#[case::label_with_a_control_character(
    r#"[{"op": "create_relation", "from": {"existing": "loose"}, "kind": "uses", "to": {"existing": "orders-db"}, "label": "a\u0000b"}]"#,
    OperationProblem::InvalidElement(ElementProblem::InvalidCharacter { field: "label", code: 0 })
)]
#[case::missing_relation(
    r#"[{"op": "delete_relation", "from": {"existing": "loose"}, "kind": "uses", "to": {"existing": "orders-db"}}]"#,
    OperationProblem::NoSuchRelation { from: element_id("loose"), kind: RelationKind::Uses, to: element_id("orders-db") }
)]
#[case::unknown_source_of_a_deletion(
    r#"[{"op": "delete_relation", "from": {"existing": "ghost"}, "kind": "uses", "to": {"existing": "loose"}}]"#,
    OperationProblem::UnknownElement(element_id("ghost"))
)]
fn rejects_relations_that_cannot_apply(#[case] json: &str, #[case] expected: OperationProblem) {
    assert_eq!(problem(&snapshot(), json), expected);
}

#[test]
fn rejects_a_label_that_takes_the_frontmatter_past_the_limit() {
    let label = "a".repeat(abyme_model::MAX_FRONTMATTER_BYTES);
    let json = format!(
        r#"[{{"op": "create_relation", "from": {{"existing": "loose"}}, "kind": "uses", "to": {{"existing": "orders-db"}}, "label": "{label}"}}]"#
    );

    assert!(matches!(
        problem(&snapshot(), &json),
        OperationProblem::InvalidElement(ElementProblem::FrontmatterTooLarge { .. })
    ));
}

#[test]
fn shows_neither_label_nor_name_when_it_rejects_a_relation() {
    let json = r#"[
        {"op": "create_element", "key": "e", "type_ref": "t", "name": "SECRET-NAME"},
        {"op": "create_relation", "from": {"new": "e"}, "kind": "uses", "to": {"existing": "loose"}, "label": "SECRET-LABEL"},
        {"op": "create_relation", "from": {"new": "e"}, "kind": "uses", "to": {"existing": "loose"}, "label": "SECRET-LABEL"}
    ]"#;

    let message = apply(&snapshot(), &operations(json))
        .unwrap_err()
        .to_string();

    assert!(!message.contains("SECRET"), "{message}");
}

#[test]
fn deletes_a_relation_between_elements_created_in_the_same_request() {
    let json = r#"[
        {"op": "create_element", "key": "pay", "type_ref": "t", "name": "Payment"},
        {"op": "create_element", "key": "db", "type_ref": "t", "name": "Db"},
        {"op": "create_relation", "from": {"new": "pay"}, "kind": "uses", "to": {"new": "db"}},
        {"op": "create_relation", "from": {"new": "pay"}, "kind": "concerns", "to": {"existing": "orders-db"}},
        {"op": "delete_relation", "from": {"new": "pay"}, "kind": "uses", "to": {"new": "db"}}
    ]"#;

    let applied = apply(&snapshot(), &operations(json)).unwrap();

    let written = written_elements(&applied);
    let payment: Vec<&&Element> = written
        .iter()
        .filter(|element| element.id().as_str() == "payment")
        .collect();
    assert_eq!(payment.len(), 1);
    assert_eq!(
        relations_of(payment.first().unwrap()),
        [(RelationKind::Concerns, "orders-db", None)]
    );
}

#[test]
fn deletes_a_relation_from_an_existing_element_to_one_created_in_the_same_request() {
    let json = r#"[
        {"op": "create_element", "key": "db", "type_ref": "t", "name": "Db"},
        {"op": "create_relation", "from": {"existing": "loose"}, "kind": "uses", "to": {"new": "db"}},
        {"op": "create_relation", "from": {"existing": "loose"}, "kind": "see_also", "to": {"existing": "orders-db"}},
        {"op": "delete_relation", "from": {"existing": "loose"}, "kind": "uses", "to": {"new": "db"}}
    ]"#;

    let applied = apply(&snapshot(), &operations(json)).unwrap();

    let written = written_elements(&applied);
    let ids: Vec<&str> = written
        .iter()
        .map(|element| element.id().as_str())
        .collect();
    assert_eq!(ids, ["db", "loose"]);
    assert_eq!(
        relations_of(written.get(1).unwrap()),
        [(RelationKind::SeeAlso, "orders-db", None)]
    );
}
