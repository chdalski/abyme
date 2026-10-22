use std::fs;

use abyme_model::{BoardId, ElementId};
use rstest::rstest;

use super::discovery::{current_user, discover, init};
use super::location::Location;
use super::*;

/// A user other than the one running the tests, so that every file the
/// tests create belongs to someone else.
fn other_user() -> u32 {
    current_user().wrapping_add(1)
}

/// Every path below `root` with its content, without following links.
fn listing(root: &Path) -> Vec<(PathBuf, Option<Vec<u8>>)> {
    let mut entries = Vec::new();
    let mut pending = vec![root.to_path_buf()];
    while let Some(directory) = pending.pop() {
        for entry in fs::read_dir(&directory).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                pending.push(path.clone());
                entries.push((path, None));
            } else {
                entries.push((path.clone(), Some(fs::read(&path).unwrap())));
            }
        }
    }
    entries.sort();
    entries
}

#[test]
fn rejects_a_project_of_another_user_also_from_a_nested_repository() {
    let outer = tempfile::tempdir().unwrap();
    init(outer.path(), current_user()).unwrap();
    let start = outer.path().join("repo").join("sub");
    fs::create_dir_all(&start).unwrap();
    fs::create_dir(outer.path().join("repo").join(".git")).unwrap();
    let before = listing(outer.path());

    let discovered = discover(&start, other_user()).unwrap_err();
    let initialized = init(&start, other_user()).unwrap_err();

    let root = fs::canonicalize(outer.path()).unwrap();
    for error in [discovered, initialized] {
        assert!(
            matches!(&error, ProjectError::NotOwned { root: found } if *found == root),
            "{error:?}"
        );
        assert!(error.to_string().contains("belongs to another user"));
    }
    assert_eq!(listing(outer.path()), before);
}

#[test]
fn does_not_initialize_a_directory_of_another_user() {
    let directory = tempfile::tempdir().unwrap();

    let error = init(directory.path(), other_user()).unwrap_err();

    assert!(matches!(error, ProjectError::NotOwned { .. }), "{error:?}");
    assert_eq!(listing(directory.path()), Vec::new());
}

#[test]
fn accepts_a_project_of_the_user_running_abyme() {
    let directory = tempfile::tempdir().unwrap();
    init(directory.path(), current_user()).unwrap();

    let project = discover(directory.path(), current_user()).unwrap();

    assert_eq!(project.root(), fs::canonicalize(directory.path()).unwrap());
}

#[rstest]
#[case::project_file(Location::ProjectFile, "abyme/abyme.toml")]
#[case::elements(Location::ElementsDir, "abyme/elements")]
#[case::boards(Location::BoardsDir, "abyme/boards")]
#[case::ignore_file(Location::IgnoreFile, ".abyme/.gitignore")]
#[case::lock(Location::Lock, ".abyme/lock")]
#[case::temp(Location::Temp { process: 7, counter: 42 }, ".abyme/tmp/7-42")]
fn resolves_fixed_paths(#[case] location: Location, #[case] expected: &str) {
    assert_eq!(location.relative(), PathBuf::from(expected));
}

#[test]
fn resolves_element_and_board_files_from_their_ids() {
    let element: ElementId = "payment-service".parse().unwrap();
    let board: BoardId = "context".parse().unwrap();

    assert_eq!(
        Location::Element(&element).relative(),
        PathBuf::from("abyme/elements/payment-service.md")
    );
    assert_eq!(
        Location::Board(&board).relative(),
        PathBuf::from("abyme/boards/context.board.json")
    );
}
