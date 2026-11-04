//! Tests of the statement kind of an element.

use super::*;

#[test]
fn creates_an_element_with_a_statement_or_without_one_on_null() {
    let json = r#"[
        {"op": "create_element", "key": "a", "type_ref": "t", "name": "A", "statement": "question"},
        {"op": "create_element", "key": "b", "type_ref": "t", "name": "B", "statement": null},
        {"op": "create_element", "key": "c", "type_ref": "t", "name": "C"}
    ]"#;

    let applied = apply(&snapshot(), &operations(json)).unwrap();

    let kinds: Vec<Option<StatementKind>> = written_elements(&applied)
        .iter()
        .map(|element| element.statement())
        .collect();
    assert_eq!(kinds, [Some(StatementKind::Question), None, None]);
}

/// What an update with `fields` writes for `loose`, after an earlier update
/// gave it a kind.
fn updated_kind(fields: &str) -> Element {
    let json = format!(
        r#"[
        {{"op": "update_element", "element": {{"existing": "loose"}}, "statement": "idea"}},
        {{"op": "update_element", "element": {{"existing": "loose"}}{fields}}}
    ]"#
    );
    let applied = apply(&snapshot(), &operations(&json)).unwrap();
    written_element(&applied, "loose").unwrap().clone()
}

#[rstest]
#[case::a_new_kind(r#", "statement": "target""#, Some(StatementKind::Target))]
#[case::null_removes_it(r#", "statement": null"#, None)]
#[case::another_field_keeps_it(r#", "summary": "S""#, Some(StatementKind::Idea))]
fn an_update_sets_removes_or_keeps_the_statement(
    #[case] fields: &str,
    #[case] expected: Option<StatementKind>,
) {
    assert_eq!(updated_kind(fields).statement(), expected);
}

#[test]
fn a_relation_keeps_the_statement() {
    let json = r#"[
        {"op": "update_element", "element": {"existing": "loose"}, "statement": "idea"},
        {"op": "create_relation", "from": {"existing": "loose"}, "kind": "uses", "to": {"existing": "orders-db"}},
        {"op": "delete_relation", "from": {"existing": "loose"}, "kind": "uses", "to": {"existing": "orders-db"}}
    ]"#;

    let applied = apply(&snapshot(), &operations(json)).unwrap();

    let kept = written_element(&applied, "loose")
        .is_some_and(|element| element.statement() == Some(StatementKind::Idea));
    assert!(kept, "{:?}", applied.writes);
}

#[rstest]
#[case::a_kind(r#""statement": "idea""#)]
#[case::null_on_an_element_without_one(r#""statement": null"#)]
fn an_update_of_only_the_statement_is_an_update_and_removes_the_stamp(#[case] field: &str) {
    let before = stamped_snapshot();
    let stamped = before
        .elements
        .values()
        .find(|element| element.verified().is_some())
        .unwrap()
        .id()
        .clone();
    let json =
        format!(r#"[{{"op": "update_element", "element": {{"existing": "{stamped}"}}, {field}}}]"#);

    let applied = apply(&before, &operations(&json)).unwrap();

    let written = written_element(&applied, stamped.as_str());
    assert_eq!(written.unwrap().verified(), None);
}

#[test]
fn a_statement_past_the_frontmatter_limit_writes_nothing_of_the_request() {
    let largest = (abyme_model::MAX_FRONTMATTER_BYTES - 200..=abyme_model::MAX_FRONTMATTER_BYTES)
        .rev()
        .find(|length| {
            let json = format!(
                r#"[{{"op": "update_element", "element": {{"existing": "loose"}}, "summary": "{}"}}]"#,
                "a".repeat(*length)
            );
            apply(&snapshot(), &operations(&json)).is_ok()
        })
        .unwrap();
    let json = format!(
        r#"[
        {{"op": "update_element", "element": {{"existing": "loose"}}, "summary": "{}"}},
        {{"op": "update_element", "element": {{"existing": "loose"}}, "statement": "idea"}}
    ]"#,
        "a".repeat(largest)
    );

    assert!(matches!(
        problem(&snapshot(), &json),
        OperationProblem::InvalidElement(ElementProblem::FrontmatterTooLarge { .. })
    ));
}
