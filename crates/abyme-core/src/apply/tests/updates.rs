//! Tests of updating elements.

use super::*;

/// The snapshot with `loose` given a summary, a body, and a relation.
fn snapshot_with_full_loose() -> Snapshot {
    let mut snapshot = snapshot();
    let loose = ElementFixture::new("loose")
        .type_ref("c4/container")
        .name("Loose")
        .summary(Some("first"))
        .body("Body\r\nwith CRLF\n")
        .relations(&[(RelationKind::Uses, "orders-db", Some("SQL"))])
        .build();
    snapshot.elements.insert(element_id("loose"), loose);
    snapshot
}

fn update(snapshot: &Snapshot, json: &str) -> Element {
    let applied = apply(snapshot, &operations(json)).unwrap();
    let written = written_elements(&applied);
    assert_eq!(written.len(), 1, "{applied:?}");
    (*written.first().unwrap()).clone()
}

#[test]
fn updates_only_the_body_and_keeps_every_other_field() {
    let snapshot = snapshot_with_full_loose();
    let before = snapshot.elements.get(&element_id("loose")).unwrap().clone();

    let after = update(
        &snapshot,
        r#"[{"op": "update_element", "element": {"existing": "loose"}, "body": "New"}]"#,
    );

    assert_eq!(after.body(), "New");
    assert_eq!(after.id(), before.id());
    assert_eq!(after.name(), before.name());
    assert_eq!(after.type_ref(), before.type_ref());
    assert_eq!(after.summary(), before.summary());
    assert_eq!(after.relations(), before.relations());
}

#[test]
fn renames_an_element_and_keeps_its_id_and_body_byte_for_byte() {
    let snapshot = snapshot_with_full_loose();

    let after = update(
        &snapshot,
        r#"[{"op": "update_element", "element": {"existing": "loose"}, "name": "Renamed"}]"#,
    );

    assert_eq!(after.id(), &element_id("loose"));
    assert_eq!(after.name(), "Renamed");
    assert_eq!(after.body(), "Body\r\nwith CRLF\n");
}

#[rstest]
#[case::summary_removed(r#""summary": null"#, None)]
#[case::summary_replaced(r#""summary": "second""#, Some("second"))]
fn removes_or_replaces_the_summary(#[case] field: &str, #[case] expected: Option<&str>) {
    let json =
        format!(r#"[{{"op": "update_element", "element": {{"existing": "loose"}}, {field}}}]"#);

    let after = update(&snapshot_with_full_loose(), &json);

    assert_eq!(after.summary(), expected);
}

#[test]
fn changes_the_type() {
    let after = update(
        &snapshot(),
        r#"[{"op": "update_element", "element": {"existing": "loose"}, "type_ref": "c4/component"}]"#,
    );

    assert_eq!(after.type_ref().as_str(), "c4/component");
}

#[test]
fn writes_an_element_created_and_updated_in_one_request_once_as_new() {
    let json = r#"[
        {"op": "create_element", "key": "e", "type_ref": "t", "name": "Fresh"},
        {"op": "update_element", "element": {"new": "e"}, "summary": "updated"},
        {"op": "create_board", "key": "b", "title": "B"}
    ]"#;

    let applied = apply(&snapshot(), &operations(json)).unwrap();

    assert!(matches!(
        applied.writes.as_slice(),
        [Write::Element(_), Write::Board(_)]
    ));
    let written = written_elements(&applied);
    assert_eq!(written.first().unwrap().summary(), Some("updated"));
}

#[test]
fn writes_an_element_updated_twice_once_with_the_last_state() {
    let json = r#"[
        {"op": "update_element", "element": {"existing": "loose"}, "name": "First"},
        {"op": "update_element", "element": {"existing": "loose"}, "name": "Second"}
    ]"#;

    let after = update(&snapshot(), json);

    assert_eq!(after.name(), "Second");
}

#[rstest]
#[case::unknown_element(
    r#"[{"op": "update_element", "element": {"existing": "nope"}, "name": "x"}]"#,
    OperationProblem::UnknownElement(element_id("nope"))
)]
#[case::element_without_a_file(
    r#"[{"op": "update_element", "element": {"existing": "ghost"}, "name": "x"}]"#,
    OperationProblem::UnknownElement(element_id("ghost"))
)]
#[case::board_key(
    r#"[
        {"op": "create_board", "key": "b", "title": "B"},
        {"op": "update_element", "element": {"new": "b"}, "name": "x"}
    ]"#,
    OperationProblem::WrongKind { key: key("b"), expected: Kind::Element, actual: Kind::Board }
)]
#[case::no_field(
    r#"[{"op": "update_element", "element": {"existing": "loose"}}]"#,
    OperationProblem::NothingToUpdate(element_id("loose"))
)]
#[case::empty_type(
    r#"[{"op": "update_element", "element": {"existing": "loose"}, "type_ref": ""}]"#,
    OperationProblem::InvalidElement(ElementProblem::EmptyType)
)]
fn rejects_updates_that_cannot_apply(#[case] json: &str, #[case] expected: OperationProblem) {
    assert_eq!(problem(&snapshot(), json), expected);
}

#[rstest]
#[case::name("name", "name")]
#[case::summary("summary", "summary")]
#[case::type_ref("type_ref", "type")]
fn rejects_a_disallowed_character_without_showing_the_value(
    #[case] field: &str,
    #[case] reported: &'static str,
) {
    let json = format!(
        r#"[{{"op": "update_element", "element": {{"existing": "loose"}}, "{field}": "SECRET\u0000"}}]"#
    );

    let error = apply(&snapshot(), &operations(&json)).unwrap_err();

    assert_eq!(
        error,
        ApplyError::Operation {
            position: 1,
            problem: OperationProblem::InvalidElement(ElementProblem::InvalidCharacter {
                field: reported,
                code: 0
            })
        }
    );
    assert!(!error.to_string().contains("SECRET"), "{error}");
}

#[rstest]
#[case::name("name")]
#[case::summary("summary")]
fn rejects_a_value_that_takes_the_frontmatter_past_the_limit(#[case] field: &str) {
    let value = "a".repeat(abyme_model::MAX_FRONTMATTER_BYTES);
    let json = format!(
        r#"[{{"op": "update_element", "element": {{"existing": "loose"}}, "{field}": "{value}"}}]"#
    );

    assert!(matches!(
        problem(&snapshot(), &json),
        OperationProblem::InvalidElement(ElementProblem::FrontmatterTooLarge { .. })
    ));
}
