//! Tests of the board list and of the UI's generated types.

use super::*;
use std::fs;
use std::path::Path;

use abyme_core::{LocalKey, Operation, Project};
use ts_rs::{Config, TS as _};

/// Where the UI's generated types lie, relative to this crate.
const BINDINGS: &str = "ui/src/bindings.ts";

/// The first lines of the generated file.
const BINDINGS_HEADER: &str = "\
// Generated from the Rust types of abyme-canvas with ts-rs; do not edit. When the test
// `committed_bindings_match_the_rust_types` fails, replace this file with the text it expects.
";

/// The UI's types as the Rust types declare them.
fn bindings() -> String {
    let config = Config::new();
    format!("{BINDINGS_HEADER}\nexport {}\n", BoardEntry::decl(&config))
}

fn create_board(title: &str) -> Operation {
    Operation::CreateBoard {
        key: LocalKey::try_from(format!("board-{}", title.len())).unwrap(),
        title: title.to_owned(),
    }
}

fn snapshot_with(root: &Path, operations: &[Operation]) -> Snapshot {
    Project::init(root).unwrap();
    let project = Project::discover(root).unwrap();
    project.apply(operations).unwrap();
    project.snapshot().unwrap()
}

#[test]
fn committed_bindings_match_the_rust_types() {
    let committed =
        fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join(BINDINGS)).unwrap();
    let expected = bindings();

    assert!(
        committed == expected,
        "{BINDINGS} is stale; the Rust types expect this text:\n{expected}"
    );
}

#[test]
fn a_board_id_crosses_as_a_string() {
    assert!(bindings().contains("id: string"));
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
