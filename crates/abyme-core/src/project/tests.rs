#![expect(
    clippy::disallowed_fields,
    reason = "the tests check the fields as the board file stores them"
)]

use std::fs;
use std::thread;
use std::time::Duration;

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

/// Operations in the format of the MCP tool `apply`.
fn operations(json: &str) -> Vec<Operation> {
    serde_json::from_str(json).unwrap()
}

/// A move of `payment-service` to 400, 200 on the board `context`.
fn a_move() -> Vec<Operation> {
    operations(
        r#"[{"op": "move_element", "element": {"existing": "payment-service"},
             "board": {"existing": "context"}, "x": 400, "y": 200, "pinned": true}]"#,
    )
}

/// A project whose board `context` places `orders` and `payment-service`.
fn project_with_a_board(root: &Path) -> Project {
    init(root, current_user()).unwrap();
    let project = discover(root, current_user()).unwrap();
    project
        .apply(&operations(
            r#"[{"op": "create_board", "key": "b", "title": "Context"},
                {"op": "create_element", "key": "p", "type_ref": "t", "name": "Payment Service"},
                {"op": "create_element", "key": "o", "type_ref": "t", "name": "Orders"},
                {"op": "place_element", "element": {"new": "p"}, "board": {"new": "b"}},
                {"op": "place_element", "element": {"new": "o"}, "board": {"new": "b"}}]"#,
        ))
        .unwrap();
    project
}

fn context_file(root: &Path) -> String {
    fs::read_to_string(root.join("abyme/boards/context.board.json")).unwrap()
}

#[test]
fn a_refusing_check_returns_its_refusal_and_touches_nothing_under_abyme() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path();
    init(root, current_user()).unwrap();
    let project = discover(root, current_user()).unwrap();
    let create = operations(r#"[{"op": "create_board", "key": "b", "title": "Context"}]"#);
    let before = listing(&root.join("abyme"));

    let result = project.apply_checked(&create, |_| Err("refused"));

    assert!(matches!(result, Ok(Err("refused"))), "{result:?}");
    assert_eq!(listing(&root.join("abyme")), before);
    assert!(!root.join("abyme/elements").exists());
    assert!(!root.join("abyme/boards").exists());
    assert!(!root.join(".abyme/tmp").exists());
}

#[test]
fn a_refusal_comes_before_a_rejection_of_the_operations() {
    let directory = tempfile::tempdir().unwrap();
    let project = project_with_a_board(directory.path());
    let unknown = operations(r#"[{"op": "delete_element", "element": {"existing": "nowhere"}}]"#);

    let result = project.apply_checked(&unknown, |_| Err("refused"));

    assert!(matches!(result, Ok(Err("refused"))), "{result:?}");
}

#[test]
fn the_check_decides_from_the_files_as_they_lie_once_the_lock_is_free() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path();
    let project = project_with_a_board(root);
    let orders_to_123 = operations(
        r#"[{"op": "move_element", "element": {"existing": "orders"},
             "board": {"existing": "context"}, "x": 123, "y": 456, "pinned": true}]"#,
    );
    project.apply(&orders_to_123).unwrap();
    let before = context_file(root);
    let changed = before.replace("\"x\": 123,", "\"x\": 777,");
    assert_ne!(changed, before);
    let context: BoardId = "context".parse().unwrap();
    let orders: ElementId = "orders".parse().unwrap();
    let holder = fs::File::open(root.join(Location::Lock.relative())).unwrap();
    holder.lock().unwrap();

    let result = thread::scope(|scope| {
        let waiting = scope.spawn(|| {
            project.apply_checked(&a_move(), |snapshot| {
                Err(snapshot.boards[&context].placements[&orders].rect.x)
            })
        });
        // Lets the request reach the lock. Correct code passes however long
        // this takes, so the wait can only weaken what the test detects.
        thread::sleep(Duration::from_millis(200));
        fs::write(root.join("abyme/boards/context.board.json"), &changed).unwrap();
        holder.unlock().unwrap();
        waiting.join().unwrap()
    });

    assert!(matches!(result, Ok(Err(777))), "{result:?}");
    assert_eq!(context_file(root), changed);
}

#[test]
fn a_passing_check_applies_and_returns_the_created_ids() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path();
    init(root, current_user()).unwrap();
    let project = discover(root, current_user()).unwrap();
    let create = operations(r#"[{"op": "create_board", "key": "b", "title": "Context"}]"#);

    let created = project
        .apply_checked(&create, |_| Ok::<(), &str>(()))
        .unwrap()
        .unwrap();

    assert_eq!(created.len(), 1);
    assert!(root.join("abyme/boards/context.board.json").exists());
}

#[test]
fn the_check_runs_while_the_project_s_lock_is_held() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path();
    let project = project_with_a_board(root);
    let lock = root.join(Location::Lock.relative());

    project
        .apply_checked(&a_move(), |_| {
            let second = fs::File::open(&lock).unwrap();
            assert!(matches!(
                second.try_lock(),
                Err(fs::TryLockError::WouldBlock)
            ));
            Ok::<(), &str>(())
        })
        .unwrap()
        .unwrap();
}

#[test]
fn the_write_starts_from_the_reading_the_check_saw() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path();
    let project = project_with_a_board(root);
    let before = context_file(root);
    let orders_seen: Vec<&str> = before
        .lines()
        .filter(|line| line.contains("\"orders\""))
        .collect();

    // A write past the lock during the check, as an editor can make: it is
    // overwritten, which shows that nothing reads the files again after the
    // check. The overwrite is what this test observes, not something wanted.
    project
        .apply_checked(&a_move(), |snapshot| {
            assert_eq!(snapshot.boards.len(), 1);
            let changed = before.replace("\"orders\", \"x\": ", "\"orders\", \"x\": 1");
            assert_ne!(changed, before);
            fs::write(root.join("abyme/boards/context.board.json"), changed).unwrap();
            Ok::<(), &str>(())
        })
        .unwrap()
        .unwrap();

    let after = context_file(root);
    assert!(after.contains("\"x\": 400, \"y\": 200"), "{after}");
    for line in orders_seen {
        assert!(after.contains(line), "{after}");
    }
}
