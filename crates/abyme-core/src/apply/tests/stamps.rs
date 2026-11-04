//! Tests of anchors and verification stamps.

use super::*;

#[test]
fn creates_elements_without_an_anchor_or_a_stamp() {
    let json = r#"[{"op": "create_element", "key": "e", "type_ref": "t", "name": "Fresh"}]"#;

    let applied = apply(&snapshot(), &operations(json)).unwrap();

    let created = written_elements(&applied);
    let fresh = created.first().unwrap();
    assert_eq!(fresh.anchor(), None);
    assert_eq!(fresh.verified(), None);
}

#[test]
fn creates_an_element_with_an_anchor_and_no_stamp() {
    let json = r#"[{"op": "create_element", "key": "e", "type_ref": "t", "name": "Fresh", "anchor": "crates/fresh"}]"#;

    let applied = apply(&snapshot(), &operations(json)).unwrap();

    let fresh = written_element(&applied, "fresh").unwrap();
    assert_eq!(fresh.anchor().map(Anchor::as_str), Some("crates/fresh"));
    assert_eq!(fresh.verified(), None);
}

#[rstest]
#[case::name(r#""name": "M""#, Some("crates/anchored"))]
#[case::same_name(r#""name": "N""#, Some("crates/anchored"))]
#[case::type_ref(r#""type_ref": "c4/container""#, Some("crates/anchored"))]
#[case::summary(r#""summary": "S""#, Some("crates/anchored"))]
#[case::summary_removed(r#""summary": null"#, Some("crates/anchored"))]
#[case::body(r#""body": "B""#, Some("crates/anchored"))]
#[case::same_anchor(r#""anchor": "crates/anchored""#, Some("crates/anchored"))]
#[case::other_anchor(r#""anchor": "crates/other""#, Some("crates/other"))]
#[case::other_case(r#""anchor": "Crates/anchored""#, Some("Crates/anchored"))]
#[case::removed_anchor(r#""anchor": null"#, None)]
fn removes_the_stamp_on_every_update(#[case] fields: &str, #[case] anchor: Option<&str>) {
    let json =
        format!(r#"[{{"op": "update_element", "element": {{"existing": "anchored"}}, {fields}}}]"#);

    let applied = apply(&stamped_snapshot(), &operations(&json)).unwrap();

    let updated = written_element(&applied, "anchored").unwrap();
    assert_eq!(updated.anchor().map(Anchor::as_str), anchor, "{fields}");
    assert_eq!(updated.verified(), None, "{fields}");
}

#[rstest]
#[case::first_anchor(r#""anchor": "docs""#)]
#[case::no_anchor_removed(r#""anchor": null"#)]
fn removes_the_stamp_of_an_element_without_an_anchor(#[case] fields: &str) {
    let json =
        format!(r#"[{{"op": "update_element", "element": {{"existing": "native"}}, {fields}}}]"#);

    let applied = apply(&stamped_snapshot(), &operations(&json)).unwrap();

    assert_eq!(
        written_element(&applied, "native").unwrap().verified(),
        None,
        "{fields}"
    );
}

#[rstest]
#[case::created(
    r#"[{"op": "create_relation", "from": {"existing": "anchored"}, "kind": "uses", "to": {"existing": "native"}}]"#
)]
#[case::deleted_to_an_element_without_a_file(
    r#"[{"op": "delete_relation", "from": {"existing": "anchored"}, "kind": "uses", "to": {"existing": "ghost"}}]"#
)]
fn removes_the_stamp_of_the_source_of_a_changed_relation_only(#[case] json: &str) {
    let snapshot = stamped_snapshot();

    let applied = apply(&snapshot, &operations(json)).unwrap();

    let written: Vec<&str> = written_elements(&applied)
        .iter()
        .map(|element| element.id().as_str())
        .collect();
    assert_eq!(written, ["anchored"]);
    assert_eq!(written_elements(&applied).first().unwrap().verified(), None);
}

#[rstest]
#[case::placed_and_moved(
    r#"[{"op": "place_element", "element": {"existing": "anchored"}, "board": {"existing": "main"}},
        {"op": "move_element", "element": {"existing": "anchored"}, "board": {"existing": "main"}, "x": 600, "y": 0}]"#
)]
#[case::other_element_deleted(r#"[{"op": "delete_element", "element": {"existing": "loose"}}]"#)]
fn writes_no_stamped_element_that_the_request_does_not_change(#[case] json: &str) {
    let applied = apply(&stamped_snapshot(), &operations(json)).unwrap();

    assert_eq!(written_elements(&applied), Vec::<&Element>::new());
}

#[test]
fn a_failing_relation_change_writes_nothing() {
    let json = r#"[
        {"op": "update_element", "element": {"existing": "native"}, "name": "M"},
        {"op": "create_relation", "from": {"existing": "anchored"}, "kind": "uses", "to": {"existing": "native"}},
        {"op": "create_relation", "from": {"existing": "anchored"}, "kind": "uses", "to": {"existing": "native"}}
    ]"#;

    assert_eq!(
        problem(&stamped_snapshot(), json),
        OperationProblem::RelationExists {
            from: element_id("anchored"),
            kind: RelationKind::Uses,
            to: element_id("native"),
        }
    );
}

#[test]
fn drops_the_stamp_of_an_anchor_changed_and_changed_back_in_one_request() {
    let json = r#"[
        {"op": "update_element", "element": {"existing": "anchored"}, "anchor": "crates/other"},
        {"op": "update_element", "element": {"existing": "anchored"}, "anchor": "crates/anchored"}
    ]"#;

    let applied = apply(&stamped_snapshot(), &operations(json)).unwrap();

    let updated = written_element(&applied, "anchored").unwrap();
    assert_eq!(
        updated.anchor().map(Anchor::as_str),
        Some("crates/anchored")
    );
    assert_eq!(updated.verified(), None);
}

#[rstest]
#[case::dot_dot_segment("crates/../etc")]
#[case::leading_slash("/etc/passwd")]
#[case::reserved_folder("abyme/elements")]
#[case::git_folder_in_any_case("a/.GIT/b")]
#[case::colon("C:x")]
#[case::right_to_left_override("a\u{202e}b")]
#[case::line_feed("a\nb")]
#[case::too_long(&"a".repeat(4097))]
fn rejects_an_invalid_anchor_in_a_creation_and_an_update(#[case] anchor: &str) {
    let snapshot = stamped_snapshot();
    let requests = [
        serde_json::json!([
            {"op": "create_element", "key": "e", "type_ref": "t", "name": "N", "anchor": anchor}
        ]),
        serde_json::json!([
            {"op": "update_element", "element": {"existing": "anchored"}, "anchor": anchor}
        ]),
    ];

    for request in requests {
        let problem = problem(&snapshot, &request.to_string());

        assert!(
            matches!(
                problem,
                OperationProblem::InvalidElement(ElementProblem::InvalidAnchor { .. })
            ),
            "{problem:?}"
        );
    }
}

#[test]
fn shows_a_rejected_anchor_escaped_and_shortened() {
    let anchor = format!("\u{202e}\u{1b}[31m{}", "x".repeat(10_000));
    let json = serde_json::json!([
        {"op": "update_element", "element": {"existing": "anchored"}, "anchor": anchor}
    ]);

    let error = apply(&stamped_snapshot(), &operations(&json.to_string())).unwrap_err();

    let message = error.to_string();
    assert!(message.len() < 600, "{} bytes", message.len());
    assert!(!message.chars().any(hides_or_breaks_text), "{message:?}");
    assert!(message.contains("\\u{202e}"), "{message:?}");
}
