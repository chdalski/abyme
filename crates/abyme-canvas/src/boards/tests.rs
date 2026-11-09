//! Tests of the board list.

use super::*;
use std::fs;
use std::path::Path;

use abyme_core::{LocalKey, Operation, Project, Ref};
use serde_json::Value;

fn create_board(title: &str) -> Operation {
    Operation::CreateBoard {
        key: LocalKey::try_from(format!("board-{}", title.len())).unwrap(),
        title: title.to_owned(),
        owner: None,
        summary: None,
        zone: None,
    }
}

fn snapshot_with(root: &Path, operations: &[Operation]) -> Snapshot {
    Project::init(root).unwrap();
    let project = Project::discover(root).unwrap();
    project.apply(operations).unwrap();
    project.snapshot().unwrap()
}

#[test]
fn lists_top_level_boards_by_id_with_their_titles() {
    let root = tempfile::tempdir().unwrap();
    let snapshot = snapshot_with(
        root.path(),
        &[create_board("Zebra"), create_board("Containers")],
    );

    let titles: Vec<(String, String)> = board_list(&snapshot)
        .into_iter()
        .map(|listed| (listed.entry.id.to_string(), listed.entry.title))
        .collect();

    assert_eq!(
        titles,
        [
            ("containers".to_owned(), "Containers".to_owned()),
            ("zebra".to_owned(), "Zebra".to_owned()),
        ]
    );
}

#[test]
fn lists_nothing_for_a_project_without_boards() {
    let root = tempfile::tempdir().unwrap();
    let snapshot = snapshot_with(root.path(), &[]);

    assert_eq!(board_list(&snapshot), []);
}

/// A key for an operation that names what it creates.
fn key(name: &str) -> LocalKey {
    LocalKey::try_from(name.to_owned()).unwrap()
}

/// Creates an element `name`, at home on the board `home`, owning a new board
/// `name` inside it, so the new board lies one level below `home`.
fn inner_board(name: &str, home: Ref<BoardId>) -> [Operation; 3] {
    [
        Operation::CreateElement {
            key: key(&format!("{name}-element")),
            type_ref: "c4/container".to_owned(),
            name: format!("{name} element"),
            summary: None,
            body: String::new(),
            anchor: None,
            statement: None,
        },
        Operation::PlaceElement {
            element: Ref::New(key(&format!("{name}-element"))),
            board: home,
        },
        Operation::CreateBoard {
            key: key(name),
            title: name.to_owned(),
            owner: Some(Ref::New(key(&format!("{name}-element")))),
            summary: None,
            zone: None,
        },
    ]
}

fn top_board(name: &str) -> Operation {
    Operation::CreateBoard {
        key: key(name),
        title: name.to_owned(),
        owner: None,
        summary: None,
        zone: None,
    }
}

/// The list as `(depth, id)` pairs.
fn depths(snapshot: &Snapshot) -> Vec<(usize, String)> {
    board_list(snapshot)
        .into_iter()
        .map(|listed| (listed.depth, listed.entry.id.to_string()))
        .collect()
}

/// Boards `context` > `containers` > `components`, and `zebra` and `docs` on
/// their own, `docs` holding `notes`.
fn nested_project(root: &Path) -> Snapshot {
    let operations: Vec<Operation> = [
        vec![top_board("context"), top_board("zebra"), top_board("docs")],
        inner_board("containers", Ref::New(key("context"))).to_vec(),
        inner_board("components", Ref::New(key("containers"))).to_vec(),
        inner_board("notes", Ref::New(key("docs"))).to_vec(),
    ]
    .concat();
    snapshot_with(root, &operations)
}

#[test]
fn lists_the_boards_depth_first_with_their_depths_as_overview_does() {
    let root = tempfile::tempdir().unwrap();
    let snapshot = nested_project(root.path());

    assert_eq!(
        depths(&snapshot),
        [
            (0, "context".to_owned()),
            (1, "containers".to_owned()),
            (2, "components".to_owned()),
            (0, "docs".to_owned()),
            (1, "notes".to_owned()),
            (0, "zebra".to_owned()),
        ]
    );
}

#[test]
fn puts_a_board_whose_owner_has_no_element_file_at_the_top_level_with_its_inner_boards_below() {
    let root = tempfile::tempdir().unwrap();
    nested_project(root.path());
    fs::remove_file(root.path().join("abyme/elements/containers-element.md")).unwrap();
    let snapshot = Project::discover(root.path()).unwrap().snapshot().unwrap();

    assert_eq!(
        depths(&snapshot),
        [
            (0, "containers".to_owned()),
            (1, "components".to_owned()),
            (0, "context".to_owned()),
            (0, "docs".to_owned()),
            (1, "notes".to_owned()),
            (0, "zebra".to_owned()),
        ]
    );
}

#[test]
fn lists_each_board_of_a_cycle_once_at_the_top_level() {
    let root = tempfile::tempdir().unwrap();
    // `first` lies inside `second`'s element and the other way round: no
    // request makes that, a hand edit or a merge can.
    let operations: Vec<Operation> = [
        vec![top_board("first")],
        inner_board("second", Ref::New(key("first"))).to_vec(),
        vec![
            Operation::CreateElement {
                key: key("first-element"),
                type_ref: "c4/container".to_owned(),
                name: "first element".to_owned(),
                summary: None,
                body: String::new(),
                anchor: None,
                statement: None,
            },
            Operation::PlaceElement {
                element: Ref::New(key("first-element")),
                board: Ref::New(key("second")),
            },
        ],
    ]
    .concat();
    snapshot_with(root.path(), &operations);
    let file = root.path().join("abyme/boards/first.board.json");
    let mut board: Value = serde_json::from_str(&fs::read_to_string(&file).unwrap()).unwrap();
    board["owner"] = Value::from("first-element");
    fs::write(&file, serde_json::to_string(&board).unwrap()).unwrap();
    let snapshot = Project::discover(root.path()).unwrap().snapshot().unwrap();

    assert_eq!(
        depths(&snapshot),
        [(0, "first".to_owned()), (0, "second".to_owned())]
    );
}

#[test]
fn an_entry_carries_exactly_its_id_title_and_depth() {
    let root = tempfile::tempdir().unwrap();
    let snapshot = nested_project(root.path());
    let first = board_list(&snapshot).into_iter().next().unwrap();

    let value = serde_json::to_value(first).unwrap();

    assert_eq!(
        value,
        serde_json::json!({"id": "context", "title": "context", "depth": 0})
    );
}
