//! Tests of the board list.

use super::*;
use std::path::Path;

use abyme_core::{LocalKey, Operation, Project};

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
fn lists_every_board_by_id_with_its_title() {
    let root = tempfile::tempdir().unwrap();
    let snapshot = snapshot_with(
        root.path(),
        &[create_board("Zebra"), create_board("Containers")],
    );

    let titles: Vec<(String, String)> = board_list(&snapshot)
        .into_iter()
        .map(|entry| (entry.id.to_string(), entry.title))
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
