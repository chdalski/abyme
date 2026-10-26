//! A project on disk: finding and initializing it, reading its files, and
//! writing a request's changes, through the public API.

use std::collections::BTreeMap;
use std::env;
use std::error::Error;
use std::ffi::OsStr;
use std::fs;
use std::io;
use std::os::unix::ffi::OsStrExt as _;
use std::os::unix::fs::symlink;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use abyme_core::{
    ApplyError, Created, EmbeddingModel, EntryKind, Init, MAX_FILE_BYTES, MAX_PROJECT_FILE_BYTES,
    Operation, Operations, Project, ProjectError, Snapshot, Write, apply,
};
use abyme_model::{BoardId, ElementId};
use rstest::rstest;
use tempfile::TempDir;

/// The environment variables that turn `child_applies_elements` into the
/// body of a child process.
const CHILD_ROOT: &str = "ABYME_TEST_CHILD_ROOT";
const CHILD_PREFIX: &str = "ABYME_TEST_CHILD_PREFIX";

/// How many requests each child process applies.
const CHILD_ROUNDS: usize = 20;

/// What lies at a path, without following links.
#[derive(Debug, PartialEq, Eq)]
enum Entry {
    Directory,
    File(Vec<u8>),
    Link(PathBuf),
    Other,
}

/// Every entry below `root`, relative to it, with its content.
fn tree(root: &Path) -> io::Result<BTreeMap<PathBuf, Entry>> {
    let mut entries = BTreeMap::new();
    let mut pending = vec![PathBuf::new()];
    while let Some(relative) = pending.pop() {
        for entry in fs::read_dir(root.join(&relative))? {
            let entry = entry?;
            let path = relative.join(entry.file_name());
            let kind = entry.file_type()?;
            let value = if kind.is_dir() {
                pending.push(path.clone());
                Entry::Directory
            } else if kind.is_file() {
                Entry::File(fs::read(entry.path())?)
            } else if kind.is_symlink() {
                Entry::Link(fs::read_link(entry.path())?)
            } else {
                Entry::Other
            };
            entries.insert(path, value);
        }
    }
    Ok(entries)
}

/// A new project in a temporary directory.
fn new_project() -> Result<(TempDir, Project), Box<dyn Error>> {
    let directory = tempfile::tempdir()?;
    match Project::init(directory.path())? {
        Init::Created(project) => Ok((directory, project)),
        Init::Existing(_) => Err("the temporary directory lies in a project".into()),
    }
}

fn operations(json: &str) -> serde_json::Result<Vec<Operation>> {
    serde_json::from_str::<Operations>(json).map(Operations::into_vec)
}

/// A request that creates the board `main` and places a new element on it.
fn board_with_element() -> serde_json::Result<Vec<Operation>> {
    operations(
        r#"[
            {"op": "create_board", "key": "b", "title": "Main"},
            {"op": "create_element", "key": "e", "type_ref": "c4/container", "name": "Payment Service",
             "summary": "Payments", "body": "Handles payments.\n"},
            {"op": "place_element", "element": {"new": "e"}, "board": {"new": "b"}}
        ]"#,
    )
}

fn element_text(id: &str) -> String {
    format!("---\nid: \"{id}\"\ntype: \"t\"\nname: \"{id}\"\n---\n")
}

fn board_text(id: &str) -> String {
    format!(r#"{{"format": 1, "id": "{id}", "title": "{id}", "placements": []}}"#)
}

/// Writes a file, creating the directories it lies in.
fn write(path: &Path, content: impl AsRef<[u8]>) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, content)
}

/// Checks that an error names paths only relative to the project's root.
fn assert_names_no_absolute_path(error: &ProjectError, directory: &Path) {
    let message = error.to_string();
    assert!(
        !message.contains(&*directory.to_string_lossy()),
        "{message}"
    );
}

#[test]
fn finds_the_root_from_a_subdirectory() {
    let (directory, project) = new_project().unwrap();
    let start = directory.path().join("src").join("deep");
    fs::create_dir_all(&start).unwrap();

    let found = Project::discover(&start).unwrap();

    assert_eq!(found, project);
    assert_eq!(found.root(), fs::canonicalize(directory.path()).unwrap());
}

#[test]
fn finds_the_outer_project_from_nested_git_repositories() {
    let (directory, project) = new_project().unwrap();
    let clone = directory.path().join("clone").join("src");
    fs::create_dir_all(&clone).unwrap();
    fs::create_dir(directory.path().join("clone").join(".git")).unwrap();
    let submodule = directory.path().join("submodule");
    write(
        &submodule.join(".git"),
        "gitdir: ../.git/modules/submodule\n",
    )
    .unwrap();

    assert_eq!(Project::discover(&clone).unwrap(), project);
    assert_eq!(Project::discover(&submodule).unwrap(), project);
    assert_eq!(Project::init(&clone).unwrap(), Init::Existing(project));
}

#[test]
fn finds_the_nearest_project_and_ignores_the_ones_above() {
    let outer = tempfile::tempdir().unwrap();
    let inner_directory = outer.path().join("inner");
    fs::create_dir(&inner_directory).unwrap();
    let Init::Created(inner) = Project::init(&inner_directory).unwrap() else {
        panic!("inner lies in a project");
    };
    // A project file above that every call would reject.
    write(
        &outer.path().join("abyme").join("abyme.toml"),
        "format = 99\n",
    )
    .unwrap();
    let start = inner_directory.join("x");
    fs::create_dir(&start).unwrap();

    assert_eq!(Project::discover(&start).unwrap(), inner);
}

#[test]
fn without_a_project_every_call_asks_for_init_and_writes_nothing() {
    let directory = tempfile::tempdir().unwrap();

    let error = Project::discover(directory.path()).unwrap_err();

    assert!(matches!(error, ProjectError::NotFound), "{error:?}");
    assert!(error.to_string().contains("abyme-mcp init"), "{error}");
    assert!(!error.to_string().contains('/'), "{error}");
    assert!(tree(directory.path()).unwrap().is_empty());
}

#[rstest]
#[case::without_a_model("format = 1\n", EmbeddingModel::BgeSmallEnV15)]
#[case::english_model(
    "format = 1\nembedding_model = \"bge-small-en-v1.5\"\n",
    EmbeddingModel::BgeSmallEnV15
)]
#[case::multilingual_model(
    "format = 1\nembedding_model = \"paraphrase-multilingual-minilm-l12-v2\"\n",
    EmbeddingModel::ParaphraseMultilingualMiniLmL12V2
)]
fn reads_the_embedding_model_from_the_project_file(
    #[case] content: &str,
    #[case] expected: EmbeddingModel,
) {
    let (directory, _) = new_project().unwrap();
    fs::write(directory.path().join("abyme/abyme.toml"), content).unwrap();

    let project = Project::discover(directory.path()).unwrap();

    assert_eq!(project.embedding_model(), expected);
}

#[test]
fn init_chooses_the_default_model_without_writing_the_key() {
    let (directory, project) = new_project().unwrap();

    assert_eq!(project.embedding_model(), EmbeddingModel::BgeSmallEnV15);
    assert_eq!(
        fs::read_to_string(directory.path().join("abyme/abyme.toml")).unwrap(),
        "format = 1\n"
    );
}

#[rstest]
#[case::not_toml("format = \n")]
#[case::missing_format("")]
#[case::unknown_format("format = 2\n")]
#[case::format_as_text("format = \"1\"\n")]
#[case::unknown_key("format = 1\ntype_packs = []\n")]
#[case::unknown_model("format = 1\nembedding_model = \"gpt\"\n")]
#[case::model_as_path("format = 1\nembedding_model = \"../x\"\n")]
#[case::model_in_capitals("format = 1\nembedding_model = \"BGE-SMALL-EN-V1.5\"\n")]
#[case::model_with_a_trailing_space("format = 1\nembedding_model = \"bge-small-en-v1.5 \"\n")]
#[case::empty_model("format = 1\nembedding_model = \"\"\n")]
#[case::model_as_number("format = 1\nembedding_model = 1\n")]
#[case::model_as_table("format = 1\nembedding_model = { name = \"bge-small-en-v1.5\" }\n")]
#[case::model_as_array("format = 1\nembedding_model = [\"bge-small-en-v1.5\"]\n")]
#[case::model_in_a_subtable("format = 1\n[x]\nembedding_model = \"bge-small-en-v1.5\"\n")]
fn rejects_an_invalid_project_file_in_every_call(#[case] content: &str) {
    let (directory, _) = new_project().unwrap();
    let project_file = directory.path().join("abyme").join("abyme.toml");
    fs::write(&project_file, content).unwrap();

    let discovered = Project::discover(directory.path()).unwrap_err();
    let initialized = Project::init(directory.path()).unwrap_err();

    for error in [discovered, initialized] {
        assert!(
            matches!(error, ProjectError::InvalidProjectFile { .. }),
            "{error:?}"
        );
        assert!(
            error.to_string().starts_with("abyme/abyme.toml: "),
            "{error}"
        );
    }
    assert_eq!(fs::read_to_string(&project_file).unwrap(), content);
}

#[rstest]
#[case::control_characters(
    "format = 1\nembedding_model = \"a\\nb\\u001b[31m\"\n",
    "a\\nb\\u{1b}[31m"
)]
#[case::long_value(&format!("format = 1\nembedding_model = \"{}\"\n", "x".repeat(300)), "…")]
fn shows_a_rejected_model_name_escaped_and_shortened(
    #[case] content: &str,
    #[case] expected: &str,
) {
    let (directory, _) = new_project().unwrap();
    fs::write(directory.path().join("abyme/abyme.toml"), content).unwrap();

    let message = Project::discover(directory.path()).unwrap_err().to_string();

    assert!(message.starts_with("abyme/abyme.toml: "), "{message}");
    assert!(message.contains(expected), "{message}");
    assert!(
        !message.contains('\n') && !message.contains('\u{1b}'),
        "{message}"
    );
}

#[test]
fn reads_a_project_file_up_to_its_limit() {
    let (directory, _) = new_project().unwrap();
    let project_file = directory.path().join("abyme").join("abyme.toml");
    let head = "format = 1\n#";
    let at_limit = format!("{head}{}", "x".repeat(MAX_PROJECT_FILE_BYTES - head.len()));
    fs::write(&project_file, &at_limit).unwrap();

    assert!(Project::discover(directory.path()).is_ok());

    fs::write(&project_file, format!("{at_limit}x")).unwrap();
    let error = Project::discover(directory.path()).unwrap_err();
    assert!(
        matches!(&error, ProjectError::TooLarge { path, limit }
            if *path == Path::new("abyme/abyme.toml") && *limit == MAX_PROJECT_FILE_BYTES),
        "{error:?}"
    );
}

#[test]
fn init_creates_the_project_file_and_the_self_ignoring_local_directory() {
    let directory = tempfile::tempdir().unwrap();

    let result = Project::init(directory.path()).unwrap();

    let root = fs::canonicalize(directory.path()).unwrap();
    assert!(matches!(&result, Init::Created(project) if project.root() == root));
    assert_eq!(
        tree(directory.path()).unwrap(),
        BTreeMap::from([
            (PathBuf::from("abyme"), Entry::Directory),
            (
                PathBuf::from("abyme/abyme.toml"),
                Entry::File(b"format = 1\n".to_vec())
            ),
            (PathBuf::from(".abyme"), Entry::Directory),
            (
                PathBuf::from(".abyme/.gitignore"),
                Entry::File(b"*\n".to_vec())
            ),
        ])
    );
}

#[test]
fn init_in_a_project_changes_nothing_and_names_its_root() {
    let (directory, project) = new_project().unwrap();
    let subdirectory = directory.path().join("src");
    fs::create_dir(&subdirectory).unwrap();
    let before = tree(directory.path()).unwrap();

    let again = Project::init(directory.path()).unwrap();
    let below = Project::init(&subdirectory).unwrap();

    assert_eq!(again, Init::Existing(project.clone()));
    assert_eq!(below, Init::Existing(project));
    assert_eq!(tree(directory.path()).unwrap(), before);
}

#[test]
fn init_completes_a_project_that_has_only_its_local_directory() {
    let directory = tempfile::tempdir().unwrap();
    write(&directory.path().join(".abyme").join(".gitignore"), "*\n").unwrap();

    let result = Project::init(directory.path()).unwrap();

    assert!(matches!(result, Init::Created(_)));
    assert_eq!(
        fs::read_to_string(directory.path().join("abyme").join("abyme.toml")).unwrap(),
        "format = 1\n"
    );
}

#[test]
fn a_fresh_clone_reads_and_its_first_write_creates_the_local_directory() {
    let (directory, project) = new_project().unwrap();
    fs::remove_dir_all(directory.path().join(".abyme")).unwrap();

    assert_eq!(project.snapshot().unwrap(), Snapshot::default());

    project.apply(&board_with_element().unwrap()).unwrap();

    let entries = tree(directory.path()).unwrap();
    assert_eq!(
        entries.get(Path::new(".abyme/.gitignore")),
        Some(&Entry::File(b"*\n".to_vec()))
    );
    for path in [
        ".abyme/lock",
        ".abyme/tmp",
        "abyme/elements",
        "abyme/boards",
    ] {
        assert!(entries.contains_key(Path::new(path)), "{path} missing");
    }
}

#[test]
fn ignores_entries_that_are_not_abyme_files() {
    let (directory, project) = new_project().unwrap();
    let outside = tempfile::tempdir().unwrap();
    let elements = directory.path().join("abyme").join("elements");
    let boards = directory.path().join("abyme").join("boards");
    fs::create_dir_all(elements.join("directory.md")).unwrap();
    fs::create_dir_all(boards.join("directory.board.json")).unwrap();
    for name in ["X.md", "x.md~", "..md", "x.board.json", "x"] {
        write(&elements.join(name), "not an element").unwrap();
    }
    write(&elements.join(OsStr::from_bytes(b"\xff.md")), "not UTF-8").unwrap();
    write(&boards.join("x.md"), "not a board").unwrap();
    let fifo = outside.path().join("fifo");
    let status = Command::new("mkfifo").arg(&fifo).status().unwrap();
    assert!(status.success());
    symlink(&fifo, elements.join("fifo.md")).unwrap();
    symlink("/dev/zero", elements.join("zero.md")).unwrap();
    let local_fifo = elements.join("local-fifo.md");
    let status = Command::new("mkfifo").arg(&local_fifo).status().unwrap();
    assert!(status.success());

    // Opening a FIFO or reading /dev/zero would never end, so a reader that
    // follows them makes this test fail instead of hang.
    let (sender, receiver) = mpsc::channel();
    thread::spawn(move || sender.send(project.snapshot().map_err(|error| error.to_string())));
    let snapshot = receiver
        .recv_timeout(Duration::from_secs(10))
        .expect("reading hangs on an entry that is not a regular file");
    assert_eq!(snapshot.unwrap(), Snapshot::default());
}

#[test]
fn apply_writes_what_the_pure_apply_decided() {
    let (directory, project) = new_project().unwrap();
    project.apply(&board_with_element().unwrap()).unwrap();
    let before = project.snapshot().unwrap();
    let request = operations(
        r#"[
            {"op": "create_element", "key": "db", "type_ref": "c4/database", "name": "Orders DB"},
            {"op": "place_element", "element": {"new": "db"}, "board": {"existing": "main"}}
        ]"#,
    )
    .unwrap();
    let expected = apply(&before, &request).unwrap();

    let created = project.apply(&request).unwrap();

    assert_eq!(created, expected.created);
    let mut after = before;
    for write in &expected.writes {
        match write {
            Write::Element(element) => {
                let path = directory
                    .path()
                    .join(format!("abyme/elements/{}.md", element.id()));
                assert_eq!(fs::read_to_string(path).unwrap(), element.to_markdown());
                after.elements.insert(element.id().clone(), element.clone());
            }
            Write::Board(board) => {
                let path = directory
                    .path()
                    .join(format!("abyme/boards/{}.board.json", board.id));
                assert_eq!(fs::read_to_string(path).unwrap(), board.to_json());
                after.boards.insert(board.id.clone(), board.clone());
            }
        }
    }
    assert_eq!(project.snapshot().unwrap(), after);
}

#[test]
fn a_rejected_request_changes_no_file() {
    let (directory, project) = new_project().unwrap();
    project.apply(&board_with_element().unwrap()).unwrap();
    let before = tree(directory.path()).unwrap();
    let request = operations(
        r#"[
            {"op": "create_element", "key": "e", "type_ref": "t", "name": "New"},
            {"op": "place_element", "element": {"new": "e"}, "board": {"existing": "unknown"}}
        ]"#,
    )
    .unwrap();

    let error = project.apply(&request).unwrap_err();

    assert!(
        matches!(
            error,
            ProjectError::Rejected(ApplyError::Operation { position: 2, .. })
        ),
        "{error:?}"
    );
    assert_eq!(tree(directory.path()).unwrap(), before);
}

#[rstest]
#[case::element_body(
    r#"[{"op": "create_element", "key": "e", "type_ref": "t", "name": "Big", "body": "BODY"}]"#,
    "abyme/elements/big.md"
)]
#[case::board_title(
    r#"[{"op": "create_board", "key": "b", "title": "BigBODY"}]"#,
    "abyme/boards/big.board.json"
)]
fn a_request_that_would_write_a_too_large_file_changes_no_file(
    #[case] template: &str,
    #[case] expected_path: &str,
) {
    let (directory, project) = new_project().unwrap();
    project.apply(&board_with_element().unwrap()).unwrap();
    write(&directory.path().join(".abyme/tmp/1-0"), "leftover").unwrap();
    let before = tree(directory.path()).unwrap();
    let request = operations(&template.replace("BODY", &" ".repeat(MAX_FILE_BYTES))).unwrap();

    let error = project.apply(&request).unwrap_err();

    assert!(
        matches!(&error, ProjectError::WriteTooLarge { path, limit, .. }
            if *path == Path::new(expected_path) && *limit == MAX_FILE_BYTES),
        "{error:?}"
    );
    assert_names_no_absolute_path(&error, directory.path());
    assert_eq!(tree(directory.path()).unwrap(), before);
}

#[test]
fn a_relation_that_takes_an_existing_element_file_past_the_limit_changes_no_file() {
    let (directory, project) = new_project().unwrap();
    project.apply(&board_with_element().unwrap()).unwrap();
    let head = element_text("x");
    write(
        &directory.path().join("abyme/elements/x.md"),
        format!("{head}{}", " ".repeat(MAX_FILE_BYTES - head.len())),
    )
    .unwrap();
    let before = tree(directory.path()).unwrap();
    let request = operations(
        r#"[{"op": "create_relation", "from": {"existing": "x"}, "kind": "uses",
             "to": {"existing": "payment-service"}, "label": "REST"}]"#,
    )
    .unwrap();

    let error = project.apply(&request).unwrap_err();

    assert!(
        matches!(&error, ProjectError::WriteTooLarge { path, .. }
            if *path == Path::new("abyme/elements/x.md")),
        "{error:?}"
    );
    assert_eq!(tree(directory.path()).unwrap(), before);
}

#[test]
fn reads_element_and_board_files_up_to_the_limit() {
    let (directory, project) = new_project().unwrap();
    let element = directory.path().join("abyme/elements/x.md");
    let head = element_text("x");
    write(
        &element,
        format!("{head}{}", "a".repeat(MAX_FILE_BYTES - head.len())),
    )
    .unwrap();
    let board = directory.path().join("abyme/boards/b.board.json");
    let empty = board_text("b");
    let title = format!("b{}", " ".repeat(MAX_FILE_BYTES - empty.len()));
    write(
        &board,
        empty.replace(r#""title": "b""#, &format!(r#""title": "{title}""#)),
    )
    .unwrap();
    assert_eq!(fs::metadata(&element).unwrap().len(), MAX_FILE_BYTES as u64);
    assert_eq!(fs::metadata(&board).unwrap().len(), MAX_FILE_BYTES as u64);

    let snapshot = project.snapshot().unwrap();
    assert_eq!(snapshot.elements.len(), 1);
    assert_eq!(snapshot.boards.len(), 1);

    for (path, relative) in [
        (&element, "abyme/elements/x.md"),
        (&board, "abyme/boards/b.board.json"),
    ] {
        let content = fs::read(path).unwrap();
        fs::write(path, [content.as_slice(), b" "].concat()).unwrap();

        let error = project.snapshot().unwrap_err();

        assert!(
            matches!(&error, ProjectError::TooLarge { path, limit }
                if *path == Path::new(relative) && *limit == MAX_FILE_BYTES),
            "{error:?}"
        );
        assert_names_no_absolute_path(&error, directory.path());
        fs::write(path, content).unwrap();
    }
}

#[test]
fn rejects_element_files_that_are_not_utf8() {
    let (directory, project) = new_project().unwrap();
    write(
        &directory.path().join("abyme/elements/x.md"),
        [element_text("x").as_bytes(), b"\xff"].concat(),
    )
    .unwrap();

    let error = project.snapshot().unwrap_err();

    assert!(
        matches!(&error, ProjectError::NotUtf8 { path } if *path == Path::new("abyme/elements/x.md")),
        "{error:?}"
    );
}

/// Not a test of its own: the body of the child processes that
/// `applies_from_two_processes_without_losing_a_change` starts.
#[test]
fn child_applies_elements() {
    let (Ok(root), Ok(prefix)) = (env::var(CHILD_ROOT), env::var(CHILD_PREFIX)) else {
        return;
    };
    let project = Project::discover(Path::new(&root)).unwrap();
    for round in 0..CHILD_ROUNDS {
        let request = operations(&format!(
            r#"[
                {{"op": "create_element", "key": "e", "type_ref": "t", "name": "{prefix} {round}"}},
                {{"op": "place_element", "element": {{"new": "e"}}, "board": {{"existing": "main"}}}}
            ]"#
        )).unwrap();
        project.apply(&request).unwrap();
    }
}

#[test]
fn applies_from_two_processes_without_losing_a_change() {
    let (directory, project) = new_project().unwrap();
    project
        .apply(&operations(r#"[{"op": "create_board", "key": "b", "title": "Main"}]"#).unwrap())
        .unwrap();

    let children: Vec<_> = ["left", "right"]
        .into_iter()
        .map(|prefix| {
            Command::new(env::current_exe().unwrap())
                .args(["child_applies_elements", "--exact", "--test-threads=1"])
                .env(CHILD_ROOT, directory.path())
                .env(CHILD_PREFIX, prefix)
                .spawn()
                .unwrap()
        })
        .collect();
    for mut child in children {
        assert!(child.wait().unwrap().success());
    }

    let snapshot = project.snapshot().unwrap();
    let main: BoardId = "main".parse().unwrap();
    assert_eq!(snapshot.elements.len(), 2 * CHILD_ROUNDS);
    assert_eq!(snapshot.boards[&main].placements.len(), 2 * CHILD_ROUNDS);
}

#[test]
fn removes_leftover_files_before_writing_and_keeps_directories() {
    let (directory, project) = new_project().unwrap();
    let outside = tempfile::tempdir().unwrap();
    let target = outside.path().join("target");
    write(&target, "keep me").unwrap();
    let temp = directory.path().join(".abyme").join("tmp");
    write(&temp.join("1234-0"), "leftover").unwrap();
    symlink(&target, temp.join("link")).unwrap();
    write(&temp.join("directory").join("file"), "kept").unwrap();

    project.apply(&board_with_element().unwrap()).unwrap();

    assert_eq!(
        tree(&temp).unwrap(),
        BTreeMap::from([
            (PathBuf::from("directory"), Entry::Directory),
            (
                PathBuf::from("directory/file"),
                Entry::File(b"kept".to_vec())
            ),
        ])
    );
    assert_eq!(fs::read_to_string(&target).unwrap(), "keep me");
}

#[rstest]
#[case::temp_directory(".abyme/tmp", EntryKind::Directory)]
#[case::local_directory(".abyme", EntryKind::Directory)]
#[case::elements_directory("abyme/elements", EntryKind::Directory)]
#[case::boards_directory("abyme/boards", EntryKind::Directory)]
#[case::lock(".abyme/lock", EntryKind::File)]
fn refuses_a_symbolic_link_at_a_fixed_path_when_writing(
    #[case] relative: &str,
    #[case] expected: EntryKind,
) {
    let (directory, project) = new_project().unwrap();
    let outside = tempfile::tempdir().unwrap();
    let target = outside.path().join("target");
    if expected == EntryKind::Directory {
        write(&target.join("file"), "outside").unwrap();
    }
    let link = directory.path().join(relative);
    fs::create_dir_all(link.parent().unwrap()).unwrap();
    if link.is_dir() {
        fs::remove_dir_all(&link).unwrap();
    }
    symlink(&target, &link).unwrap();
    let outside_before = tree(outside.path()).unwrap();

    let error = project.apply(&board_with_element().unwrap()).unwrap_err();

    assert!(
        matches!(&error, ProjectError::WrongKind { path, expected: kind }
            if *path == Path::new(relative) && *kind == expected),
        "{error:?}"
    );
    assert_names_no_absolute_path(&error, directory.path());
    assert_eq!(tree(outside.path()).unwrap(), outside_before);
}

#[rstest]
#[case::elements_directory("abyme/elements")]
#[case::boards_directory("abyme/boards")]
fn refuses_a_symbolic_link_at_a_fixed_path_when_reading(#[case] relative: &str) {
    let (directory, project) = new_project().unwrap();
    let outside = tempfile::tempdir().unwrap();
    write(&outside.path().join("x.md"), element_text("x")).unwrap();
    write(&outside.path().join("x.board.json"), board_text("x")).unwrap();
    symlink(outside.path(), directory.path().join(relative)).unwrap();

    let error = project.snapshot().unwrap_err();

    assert!(
        matches!(&error, ProjectError::WrongKind { path, .. } if *path == Path::new(relative)),
        "{error:?}"
    );
}

#[test]
fn refuses_a_symbolic_link_as_abyme_directory() {
    let directory = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    symlink(outside.path(), directory.path().join("abyme")).unwrap();

    let initialized = Project::init(directory.path()).unwrap_err();
    write(&outside.path().join("abyme.toml"), "format = 1\n").unwrap();
    let discovered = Project::discover(directory.path()).unwrap_err();

    for error in [initialized, discovered] {
        assert!(
            matches!(&error, ProjectError::WrongKind { path, .. } if *path == Path::new("abyme")),
            "{error:?}"
        );
    }
    assert_eq!(
        tree(outside.path()).unwrap(),
        BTreeMap::from([(
            PathBuf::from("abyme.toml"),
            Entry::File(b"format = 1\n".to_vec())
        )])
    );
}

#[test]
fn refuses_a_symbolic_link_as_project_file() {
    let (directory, _) = new_project().unwrap();
    let outside = tempfile::tempdir().unwrap();
    let target = outside.path().join("abyme.toml");
    write(&target, "format = 1\n").unwrap();
    let project_file = directory.path().join("abyme").join("abyme.toml");
    fs::remove_file(&project_file).unwrap();
    symlink(&target, &project_file).unwrap();

    let error = Project::discover(directory.path()).unwrap_err();

    assert!(
        matches!(&error, ProjectError::WrongKind { path, .. }
            if *path == Path::new("abyme/abyme.toml")),
        "{error:?}"
    );
    assert_eq!(fs::read_to_string(&target).unwrap(), "format = 1\n");
}

#[test]
fn ignores_linked_element_and_board_files_and_replaces_the_links_when_writing() {
    let (directory, project) = new_project().unwrap();
    let outside = tempfile::tempdir().unwrap();
    let element_target = outside.path().join("element");
    let board_target = outside.path().join("board");
    write(&element_target, element_text("payment-service")).unwrap();
    write(&board_target, board_text("main")).unwrap();
    let element_link = directory.path().join("abyme/elements/payment-service.md");
    let board_link = directory.path().join("abyme/boards/main.board.json");
    fs::create_dir_all(element_link.parent().unwrap()).unwrap();
    fs::create_dir_all(board_link.parent().unwrap()).unwrap();
    symlink(&element_target, &element_link).unwrap();
    symlink(&board_target, &board_link).unwrap();
    let outside_before = tree(outside.path()).unwrap();

    assert_eq!(project.snapshot().unwrap(), Snapshot::default());

    let created = project.apply(&board_with_element().unwrap()).unwrap();

    let element: ElementId = "payment-service".parse().unwrap();
    let board: BoardId = "main".parse().unwrap();
    assert!(created.contains(&(
        "e".to_owned().try_into().unwrap(),
        Created::Element(element)
    )));
    assert!(created.contains(&("b".to_owned().try_into().unwrap(), Created::Board(board))));
    assert!(fs::symlink_metadata(&element_link).unwrap().is_file());
    assert!(fs::symlink_metadata(&board_link).unwrap().is_file());
    assert_eq!(tree(outside.path()).unwrap(), outside_before);
}

#[test]
fn a_name_shaped_like_a_path_stays_inside_the_project() {
    let parent = tempfile::tempdir().unwrap();
    let root = parent.path().join("project");
    fs::create_dir(&root).unwrap();
    let Init::Created(project) = Project::init(&root).unwrap() else {
        panic!("the temporary directory lies in a project");
    };

    project
        .apply(&operations(
            r#"[{"op": "create_element", "key": "e", "type_ref": "t", "name": "../../outside"}]"#,
        ).unwrap())
        .unwrap();

    let entries = tree(parent.path()).unwrap();
    assert!(entries.contains_key(Path::new("project/abyme/elements/outside.md")));
    assert!(
        entries.keys().all(|path| path.starts_with("project")),
        "{entries:?}"
    );
}

#[test]
fn keeps_whatever_lies_at_the_local_ignore_file() {
    let (directory, project) = new_project().unwrap();
    let outside = tempfile::tempdir().unwrap();
    let target = outside.path().join("target");
    write(&target, "outside").unwrap();
    let ignore_file = directory.path().join(".abyme").join(".gitignore");
    fs::remove_file(&ignore_file).unwrap();
    symlink(&target, &ignore_file).unwrap();

    project.apply(&board_with_element().unwrap()).unwrap();

    assert_eq!(fs::read_link(&ignore_file).unwrap(), target);
    assert_eq!(fs::read_to_string(&target).unwrap(), "outside");
}

#[test]
fn a_rejected_nearest_project_fails_also_below_a_valid_one() {
    let (outer, _) = new_project().unwrap();
    let inner = outer.path().join("inner");
    write(&inner.join("abyme").join("abyme.toml"), "format = 99\n").unwrap();
    let before = tree(outer.path()).unwrap();

    let discovered = Project::discover(&inner).unwrap_err();
    let initialized = Project::init(&inner).unwrap_err();

    for error in [discovered, initialized] {
        assert!(
            matches!(error, ProjectError::InvalidProjectFile { .. }),
            "{error:?}"
        );
    }
    assert_eq!(tree(outer.path()).unwrap(), before);
}

#[test]
fn init_refused_for_a_linked_local_directory_writes_nothing() {
    let directory = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    symlink(outside.path(), directory.path().join(".abyme")).unwrap();

    let error = Project::init(directory.path()).unwrap_err();

    assert!(
        matches!(&error, ProjectError::WrongKind { path, .. } if *path == Path::new(".abyme")),
        "{error:?}"
    );
    assert_eq!(
        tree(directory.path()).unwrap(),
        BTreeMap::from([(
            PathBuf::from(".abyme"),
            Entry::Link(outside.path().to_path_buf())
        )])
    );
    assert!(tree(outside.path()).unwrap().is_empty());
}
