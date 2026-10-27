//! A board of a project on disk, read as an outline through the public API.

use std::fs;

use abyme_core::{Geometry, Init, Operations, Project, outline};
use abyme_model::BoardId;

#[test]
fn outlines_a_board_written_by_apply_and_marks_a_deleted_element() {
    let directory = tempfile::tempdir().unwrap();
    let Init::Created(project) = Project::init(directory.path()).unwrap() else {
        panic!("the temporary directory lies in a project");
    };
    let request: Operations = serde_json::from_str(
        r#"[
            {"op": "create_board", "key": "b", "title": "Container Level"},
            {"op": "create_element", "key": "pay", "type_ref": "c4/container",
             "name": "Payment Service", "summary": "Payments\nthrough Stripe"},
            {"op": "create_element", "key": "db", "type_ref": "c4/datastore", "name": "Orders DB"},
            {"op": "place_element", "element": {"new": "pay"}, "board": {"new": "b"}},
            {"op": "place_element", "element": {"new": "db"}, "board": {"new": "b"}}
        ]"#,
    )
    .unwrap();
    project.apply(&request.into_vec()).unwrap();
    let board: BoardId = "container-level".parse().unwrap();

    assert_eq!(
        outline(&project.snapshot().unwrap(), &board, Geometry::Hidden, None).unwrap(),
        "Board: container-level · Container Level\n\
         \n\
         orders-db        c4/datastore  Orders DB\n\
         payment-service  c4/container  Payment Service  Payments\\nthrough Stripe\n"
    );

    fs::remove_file(directory.path().join("abyme/elements/orders-db.md")).unwrap();

    assert_eq!(
        outline(&project.snapshot().unwrap(), &board, Geometry::Hidden, None).unwrap(),
        "Board: container-level · Container Level\n\
         \n\
         orders-db        ⚠ missing\n\
         payment-service  c4/container  Payment Service  Payments\\nthrough Stripe\n"
    );
}
