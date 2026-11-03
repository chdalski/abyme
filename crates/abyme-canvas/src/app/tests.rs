//! Tests of the IPC boundary through Tauri's own message handling and the
//! app's real configuration and access list, and of that configuration.

#![expect(
    clippy::disallowed_fields,
    reason = "the tests check the fields as the board file stores them"
)]
use super::*;
use std::collections::BTreeMap;
use std::fs;

use abyme_core::{LocalKey, MAX_FILE_BYTES, MAX_OPERATIONS, Operation, ProjectError, UnknownBoard};
use abyme_model::{Element, ElementId, MAX_FRONTMATTER_BYTES, Relations, StatementKind};
use abyme_test_support::{commit_all, git};
use serde_json::{Value, json};
use tauri::ipc::{CallbackFn, InvokeBody};
use tauri::test::{INVOKE_KEY, MockRuntime, get_ipc_response, mock_builder};
use tauri::webview::InvokeRequest;
use tauri::{App, WebviewWindow};

/// The origin the canvas's own page has while it is developed.
const APP_ORIGIN: &str = "http://localhost:5173/";

/// This crate's directory, where its configuration lies.
const CRATE: &str = env!("CARGO_MANIFEST_DIR");

/// Builds the canvas as `main` does, on Tauri's mock runtime, started in
/// `start`, and opens its window as its setup does. The mock runtime never
/// becomes ready, so it does not run the setup itself.
fn canvas(start: &Path) -> (App<MockRuntime>, WebviewWindow<MockRuntime>) {
    let app = configure(mock_builder(), start.to_path_buf())
        .build(tauri::generate_context!())
        .unwrap();
    open_window(&app).unwrap();
    let window = app.get_webview_window("main").unwrap();
    (app, window)
}

/// Sends the command `command` with `body` from a page at `origin`.
fn invoke(
    window: &WebviewWindow<MockRuntime>,
    origin: &str,
    command: &str,
    body: Value,
) -> Result<Value, Value> {
    get_ipc_response(
        window,
        InvokeRequest {
            cmd: command.to_owned(),
            callback: CallbackFn(0),
            error: CallbackFn(1),
            url: origin.parse().unwrap(),
            body: InvokeBody::Json(body),
            headers: tauri::http::HeaderMap::default(),
            invoke_key: INVOKE_KEY.to_owned(),
        },
    )
    .map(|response| response.deserialize::<Value>().unwrap())
}

fn list_boards_from(window: &WebviewWindow<MockRuntime>) -> Result<Value, Value> {
    invoke(window, APP_ORIGIN, "list_boards", json!({}))
}

fn open_board_from(window: &WebviewWindow<MockRuntime>, body: Value) -> Result<Value, Value> {
    invoke(window, APP_ORIGIN, "open_board", body)
}

/// A project with the board `context`, which places the elements
/// `payment-service` and `orders`, whose file is then removed.
fn project_with_a_board(root: &Path) {
    Project::init(root).unwrap();
    let operations = json!([
        {"op": "create_board", "key": "b", "title": "Context"},
        {"op": "create_element", "key": "p", "type_ref": "c4/container", "name": "Payment Service",
         "summary": "Payment processing", "body": "SECRET-BODY handles payments."},
        {"op": "create_element", "key": "o", "type_ref": "c4/container", "name": "Orders"},
        {"op": "place_element", "element": {"new": "p"}, "board": {"new": "b"}},
        {"op": "place_element", "element": {"new": "o"}, "board": {"new": "b"}},
    ]);
    let operations: Vec<Operation> = serde_json::from_value(operations).unwrap();
    Project::discover(root).unwrap().apply(&operations).unwrap();
    fs::remove_file(root.join("abyme/elements/orders.md")).unwrap();
}

fn project_with_boards(root: &Path, titles: &[&str]) {
    Project::init(root).unwrap();
    let operations: Vec<Operation> = titles
        .iter()
        .enumerate()
        .map(|(index, title)| Operation::CreateBoard {
            key: LocalKey::try_from(format!("board-{index}")).unwrap(),
            title: (*title).to_owned(),
            owner: None,
            summary: None,
            zone: None,
        })
        .collect();
    Project::discover(root).unwrap().apply(&operations).unwrap();
}

fn apply_from(window: &WebviewWindow<MockRuntime>, operations: &Value) -> Result<Value, Value> {
    invoke(
        window,
        APP_ORIGIN,
        "apply",
        json!({"operations": operations}),
    )
}

/// A move of `payment-service` on `context` to `x`, `y`.
fn a_move(x: i64, y: i64) -> Value {
    json!({
        "op": "move_element",
        "element": {"existing": "payment-service"},
        "board": {"existing": "context"},
        "x": x,
        "y": y,
        "pinned": true
    })
}

/// Every file under `abyme/`, by its path below the root, with its bytes.
fn project_files(root: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
    fn collect(root: &Path, directory: &Path, files: &mut BTreeMap<PathBuf, Vec<u8>>) {
        for entry in fs::read_dir(directory).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                collect(root, &path, files);
            } else {
                let relative = path.strip_prefix(root).unwrap().to_path_buf();
                files.insert(relative, fs::read(&path).unwrap());
            }
        }
    }
    let mut files = BTreeMap::new();
    collect(root, &root.join("abyme"), &mut files);
    files
}

fn read_json(relative: &str) -> Value {
    serde_json::from_str(&fs::read_to_string(Path::new(CRATE).join(relative)).unwrap()).unwrap()
}

/// The `script-src` directive of a content security policy.
fn script_sources(policy: &str) -> &str {
    policy
        .split(';')
        .map(str::trim)
        .find(|directive| directive.starts_with("script-src "))
        .unwrap()
}

#[test]
fn list_boards_returns_the_titles_of_the_project_s_boards() {
    let root = tempfile::tempdir().unwrap();
    project_with_boards(root.path(), &["Containers", "Context"]);
    let (_app, window) = canvas(root.path());

    assert_eq!(
        list_boards_from(&window),
        Ok(json!([
            {"id": "containers", "title": "Containers"},
            {"id": "context", "title": "Context"},
        ]))
    );
}

#[test]
fn list_boards_returns_an_empty_list_for_a_project_without_boards() {
    let root = tempfile::tempdir().unwrap();
    project_with_boards(root.path(), &[]);
    let (_app, window) = canvas(root.path());

    assert_eq!(list_boards_from(&window), Ok(json!([])));
}

#[test]
fn list_boards_outside_a_project_returns_the_hint_mcp_gives() {
    let root = tempfile::tempdir().unwrap();
    let (_app, window) = canvas(root.path());

    assert_eq!(
        list_boards_from(&window),
        Err(json!(ProjectError::NotFound.to_string()))
    );
}

#[test]
fn list_boards_returns_the_core_s_message_when_the_project_cannot_be_read() {
    let root = tempfile::tempdir().unwrap();
    project_with_boards(root.path(), &["Context"]);
    fs::write(root.path().join("abyme/boards/context.board.json"), "{").unwrap();
    let expected = Project::discover(root.path())
        .unwrap()
        .snapshot()
        .unwrap_err()
        .to_string();
    let (_app, window) = canvas(root.path());

    assert_eq!(list_boards_from(&window), Err(json!(expected)));
}

#[test]
fn open_board_returns_the_board_s_projection_without_bodies() {
    let root = tempfile::tempdir().unwrap();
    project_with_a_board(root.path());
    let (_app, window) = canvas(root.path());

    let view = open_board_from(&window, json!({"board": "context"})).unwrap();

    assert_eq!(view["id"], json!("context"));
    assert_eq!(view["title"], json!("Context"));
    assert_eq!(
        view["placements"]
            .as_array()
            .unwrap()
            .iter()
            .map(|placement| (&placement["element"], &placement["shown"]["kind"]))
            .collect::<Vec<_>>(),
        [
            (&json!("orders"), &json!("missing")),
            (&json!("payment-service"), &json!("element")),
        ]
    );
    assert!(!view.to_string().contains("SECRET-BODY"));
}

#[test]
fn open_board_returns_the_relations_the_board_shows() {
    let root = tempfile::tempdir().unwrap();
    project_with_a_board(root.path());
    let operations: Vec<Operation> = serde_json::from_value(json!([
        {"op": "create_element", "key": "s", "type_ref": "t", "name": "Source"},
        {"op": "place_element", "element": {"new": "s"}, "board": {"existing": "context"}},
        {"op": "create_relation", "from": {"new": "s"}, "kind": "see_also",
         "to": {"existing": "payment-service"}, "label": "<b>label</b>"},
    ]))
    .unwrap();
    Project::discover(root.path())
        .unwrap()
        .apply(&operations)
        .unwrap();
    let (_app, window) = canvas(root.path());

    let view = open_board_from(&window, json!({"board": "context"})).unwrap();

    assert_eq!(
        view["relations"],
        json!([{"from": "source", "kind": "see_also", "to": "payment-service",
                "label": "<b>label</b>"}])
    );
}

#[test]
fn open_board_names_the_boards_above_and_inside_with_only_id_and_title() {
    let root = tempfile::tempdir().unwrap();
    project_with_a_board(root.path());
    let operations: Vec<Operation> = serde_json::from_value(json!([
        {"op": "create_board", "key": "i", "title": "Inside",
         "owner": {"existing": "payment-service"}, "summary": "SECRET-SUMMARY"},
    ]))
    .unwrap();
    Project::discover(root.path())
        .unwrap()
        .apply(&operations)
        .unwrap();
    let (_app, window) = canvas(root.path());

    let above = open_board_from(&window, json!({"board": "context"})).unwrap();
    let inside = open_board_from(&window, json!({"board": "inside"})).unwrap();

    assert_eq!(above["trail"], json!([]));
    let boxes = above["placements"].as_array().unwrap();
    assert_eq!(boxes[0]["inner_boards"], json!([]));
    assert_eq!(
        boxes[1]["inner_boards"],
        json!([{"id": "inside", "title": "Inside"}])
    );
    assert_eq!(
        inside["trail"],
        json!([{"id": "context", "title": "Context"}])
    );
    for view in [above, inside] {
        assert!(!view.to_string().contains("SECRET"), "{view}");
    }
}

#[test]
fn open_board_says_which_placements_a_board_file_pins() {
    let root = tempfile::tempdir().unwrap();
    project_with_a_board(root.path());
    fs::write(
        root.path().join("abyme/boards/context.board.json"),
        r#"{"format": 1, "id": "context", "title": "Context", "placements": [
            {"element": "a", "x": 0, "y": 0, "width": 1, "height": 1, "pinned": true},
            {"element": "b", "x": 0, "y": 0, "width": 1, "height": 1},
            {"element": "c", "x": 0, "y": 0, "width": 1, "height": 1, "pinned": false}
        ]}"#,
    )
    .unwrap();
    let (_app, window) = canvas(root.path());

    let view = open_board_from(&window, json!({"board": "context"})).unwrap();

    assert_eq!(
        view["placements"]
            .as_array()
            .unwrap()
            .iter()
            .map(|placement| &placement["pinned"])
            .collect::<Vec<_>>(),
        [&json!(true), &json!(false), &json!(false)]
    );
}

#[test]
fn a_moved_box_is_pinned_in_the_board_s_next_view() {
    let root = tempfile::tempdir().unwrap();
    project_with_a_board(root.path());
    let (_app, window) = canvas(root.path());

    apply_from(&window, &json!([a_move(400, 200)])).unwrap();

    let view = open_board_from(&window, json!({"board": "context"})).unwrap();
    assert_eq!(
        view["placements"]
            .as_array()
            .unwrap()
            .iter()
            .map(|placement| (&placement["element"], &placement["pinned"]))
            .collect::<Vec<_>>(),
        [
            (&json!("orders"), &json!(false)),
            (&json!("payment-service"), &json!(true)),
        ]
    );
}

#[test]
fn open_board_names_only_the_id_of_an_unknown_board() {
    let root = tempfile::tempdir().unwrap();
    project_with_a_board(root.path());
    let (_app, window) = canvas(root.path());

    assert_eq!(
        open_board_from(&window, json!({"board": "elsewhere"})),
        Err(json!(
            UnknownBoard("elsewhere".parse().unwrap()).to_string()
        ))
    );
}

#[test]
fn open_board_outside_a_project_returns_the_hint_mcp_gives() {
    let root = tempfile::tempdir().unwrap();
    let (_app, window) = canvas(root.path());

    assert_eq!(
        open_board_from(&window, json!({"board": "context"})),
        Err(json!(ProjectError::NotFound.to_string()))
    );
}

#[test]
fn open_board_refuses_an_invalid_id_before_looking_for_a_project() {
    let root = tempfile::tempdir().unwrap();
    let (_app, window) = canvas(root.path());
    let not_found = json!(ProjectError::NotFound.to_string());

    for body in [
        json!({"board": "../x"}),
        json!({"board": "A"}),
        json!({"board": ""}),
        json!({"board": "a".repeat(65)}),
        json!({"board": "con"}),
        json!({"board": 7}),
        json!({}),
    ] {
        let message = open_board_from(&window, body.clone()).expect_err(&body.to_string());
        assert_ne!(message, not_found, "{body}");
        assert!(
            message.to_string().contains("invalid args"),
            "{body}: {message}"
        );
    }
}

fn review_flags_from(window: &WebviewWindow<MockRuntime>) -> Result<Value, Value> {
    invoke(window, APP_ORIGIN, "review_flags", json!({}))
}

/// Writes the element `id` with a stamp, an anchor, and a relation `uses`, as
/// far as they are given.
fn write_element(
    root: &Path,
    id: &str,
    stamp: Option<&str>,
    anchor: Option<&str>,
    uses: Option<&str>,
) {
    let stamp = stamp.map_or(String::new(), |stamp| {
        format!("verified: \"{stamp}\"\nverified_by: ai\n")
    });
    let anchor = anchor.map_or(String::new(), |anchor| format!("anchor: {anchor}\n"));
    let uses = uses.map_or(String::new(), |to| {
        format!("relations:\n  - {{kind: uses, to: {to}}}\n")
    });
    fs::create_dir_all(root.join("abyme/elements")).unwrap();
    fs::write(
        root.join(format!("abyme/elements/{id}.md")),
        format!("---\nid: {id}\ntype: t\nname: N\n{anchor}{stamp}{uses}---\n"),
    )
    .unwrap();
}

/// A project whose board `context` places the element `one`, whose file has
/// the given type, further lines of frontmatter, and body.
fn project_with_one_element(root: &Path, type_ref: &str, frontmatter: &str, body: &str) {
    Project::init(root).unwrap();
    fs::create_dir_all(root.join("abyme/elements")).unwrap();
    fs::write(
        root.join("abyme/elements/one.md"),
        format!("---\nid: one\ntype: {type_ref}\nname: N\n{frontmatter}---\n{body}"),
    )
    .unwrap();
    fs::create_dir_all(root.join("abyme/boards")).unwrap();
    fs::write(
        root.join("abyme/boards/context.board.json"),
        r#"{"format": 1, "id": "context", "title": "Context", "placements": [
            {"element": "one", "x": 0, "y": 0, "width": 180, "height": 90}]}"#,
    )
    .unwrap();
}

/// What `open_board` says the box of `one` shows.
fn shown_of_one(root: &Path) -> Value {
    let (_app, window) = canvas(root);
    let view = open_board_from(&window, json!({"board": "context"})).unwrap();
    view["placements"][0]["shown"].clone()
}

#[test]
fn a_note_shows_its_name_and_body_and_nothing_else_of_its_file() {
    let root = tempfile::tempdir().unwrap();
    let stamp = "a".repeat(40);
    project_with_one_element(
        root.path(),
        "note",
        &format!(
            "summary: SECRET-SUMMARY\nanchor: src/SECRET-ANCHOR\nstatement: idea\n\
             verified: \"{stamp}\"\nverified_by: ai\n"
        ),
        "The text.\n",
    );

    assert_eq!(
        shown_of_one(root.path()),
        json!({"kind": "note", "name": "N", "body": "The text.\n"})
    );
}

#[test]
fn a_note_with_only_a_name_shows_an_empty_body() {
    let root = tempfile::tempdir().unwrap();
    project_with_one_element(root.path(), "note", "", "");

    assert_eq!(
        shown_of_one(root.path()),
        json!({"kind": "note", "name": "N", "body": ""})
    );
}

#[test]
fn a_note_s_body_comes_back_unchanged_with_any_character() {
    let root = tempfile::tempdir().unwrap();
    let body = "</script><b>x</b>\u{0}\u{2028}end\n";
    project_with_one_element(root.path(), "note", "", body);

    assert_eq!(shown_of_one(root.path())["body"], json!(body));
}

#[test]
fn a_type_that_only_resembles_note_shows_no_body() {
    for type_ref in ["Note", "NOTE", "notes", "c4/note", "\"note \""] {
        let root = tempfile::tempdir().unwrap();
        project_with_one_element(root.path(), type_ref, "", "SECRET-BODY\n");

        let shown = shown_of_one(root.path());

        assert_eq!(shown["kind"], json!("element"), "{type_ref}");
        assert!(!shown.to_string().contains("SECRET-BODY"), "{type_ref}");
        assert_eq!(shown.get("body"), None, "{type_ref}");
    }
}

#[test]
fn review_flags_name_each_flagged_element_with_its_highest_level_and_nothing_else() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path();
    git(root, &["init", "-q"]).unwrap();
    Project::init(root).unwrap();
    for file in [
        "src/changed/secret-canary.txt",
        "src/both/b.txt",
        "src/target/t.txt",
    ] {
        let path = root.join(file);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, "a\n").unwrap();
    }
    let first = commit_all(root).unwrap();
    // `hinted` sees the change of `target` since its own stamp; `target` is
    // stamped after it, so it has no flag of its own.
    write_element(root, "hinted", Some(&first), None, Some("target"));
    fs::write(root.join("src/target/t.txt"), "b\n").unwrap();
    let second = commit_all(root).unwrap();
    write_element(root, "target", Some(&second), Some("src/target"), None);
    write_element(root, "changed", Some(&second), Some("src/changed"), None);
    write_element(root, "both", Some(&first), Some("src/both"), Some("target"));
    write_element(root, "calm", None, None, None);
    fs::write(root.join("src/changed/secret-canary.txt"), "b\n").unwrap();
    fs::write(root.join("src/both/b.txt"), "b\n").unwrap();
    let (_app, window) = canvas(root);

    assert_eq!(
        review_flags_from(&window),
        Ok(json!([
            {"element": "both", "priority": "task"},
            {"element": "changed", "priority": "task"},
            {"element": "hinted", "priority": "hint"},
        ]))
    );
}

#[test]
fn review_flags_in_a_project_outside_a_repository_say_so() {
    let root = tempfile::tempdir().unwrap();
    project_with_a_board(root.path());
    let (_app, window) = canvas(root.path());

    assert_eq!(
        review_flags_from(&window),
        Err(json!("the project lies in no Git repository"))
    );
}

#[test]
fn review_flags_outside_a_project_return_the_hint_mcp_gives() {
    let root = tempfile::tempdir().unwrap();
    let (_app, window) = canvas(root.path());

    assert_eq!(
        review_flags_from(&window),
        Err(json!(ProjectError::NotFound.to_string()))
    );
}

#[test]
fn a_remote_page_may_not_ask_for_review_flags() {
    let root = tempfile::tempdir().unwrap();
    let (_app, window) = canvas(root.path());

    let response = invoke(&window, "https://example.com/", "review_flags", json!({}));

    let message = response.expect_err("a remote page asked for review flags");
    assert!(message.to_string().contains("not allowed"), "{message}");
}

#[test]
fn a_remote_page_may_not_open_a_board() {
    let root = tempfile::tempdir().unwrap();
    project_with_a_board(root.path());
    let (_app, window) = canvas(root.path());

    let response = invoke(
        &window,
        "https://example.com/",
        "open_board",
        json!({"board": "context"}),
    );

    let message = response.expect_err("a remote page opened a board");
    assert!(message.to_string().contains("not allowed"), "{message}");
}

#[test]
fn apply_writes_a_move_into_the_board_file() {
    let root = tempfile::tempdir().unwrap();
    project_with_a_board(root.path());
    let placements = |project: &Project| {
        project.snapshot().unwrap().boards[&"context".parse().unwrap()]
            .placements
            .clone()
    };
    let payments = "payment-service".parse().unwrap();
    let orders = "orders".parse().unwrap();
    let project = Project::discover(root.path()).unwrap();
    let before = placements(&project);
    let (_app, window) = canvas(root.path());

    let applied = apply_from(&window, &json!([a_move(400, -200)]));

    assert_eq!(applied, Ok(json!([])));
    let after = placements(&project);
    let (moved, was) = (&after[&payments].rect, &before[&payments].rect);
    assert_eq!((moved.x, moved.y), (400, -200));
    assert_eq!((moved.width, moved.height), (was.width, was.height));
    assert!(after[&payments].pinned);
    assert_eq!(after[&orders], before[&orders]);
}

#[test]
fn a_request_with_a_layout_writes_nothing() {
    let root = tempfile::tempdir().unwrap();
    project_with_a_board(root.path());
    let before = project_files(root.path());
    let (_app, window) = canvas(root.path());
    let layout = json!({"op": "layout_board", "board": {"existing": "context"}});

    let applied = apply_from(&window, &json!([a_move(400, 200), layout]));

    let error = applied.unwrap_err().to_string();
    assert!(
        error.contains("operation 2: the canvas may not send layout_board"),
        "{error}"
    );
    assert_eq!(project_files(root.path()), before);
}

#[test]
fn a_request_with_a_move_that_does_not_say_pinned_writes_nothing() {
    let root = tempfile::tempdir().unwrap();
    project_with_a_board(root.path());
    let before = project_files(root.path());
    let (_app, window) = canvas(root.path());
    let plain = json!({
        "op": "move_element",
        "element": {"existing": "payment-service"},
        "board": {"existing": "context"},
        "x": 1,
        "y": 2
    });

    let applied = apply_from(&window, &json!([a_move(400, 200), plain]));

    let error = applied.unwrap_err().to_string();
    assert!(
        error.contains("the canvas may not send move_element without pinned"),
        "{error}"
    );
    assert_eq!(project_files(root.path()), before);
}

/// `root` as `project_with_a_board` leaves it, with the board `context` made
/// a zone of `zone`, as the AI or a hand edit does.
fn make_context_a_zone(root: &Path, zone: &str) {
    let file = root.join("abyme/boards/context.board.json");
    let text = fs::read_to_string(&file).unwrap();
    let zoned = text.replace(
        "  \"placements\"",
        &format!("  \"zone\": {zone},\n  \"placements\""),
    );
    fs::write(file, zoned).unwrap();
}

/// A second board, `free-board`, a zone of mode free that places
/// `payment-service` too.
fn add_a_free_board(root: &Path) {
    fs::write(
        root.join("abyme/boards/free-board.board.json"),
        r#"{"format": 1, "id": "free-board", "title": "Free",
            "zone": {"mode": "free", "statement": "idea"}, "placements": [
            {"element": "payment-service", "x": 0, "y": 0, "width": 180, "height": 90}]}"#,
    )
    .unwrap();
}

/// `project_with_a_board` with `payment-service` moved by the canvas, so
/// pinned at 400, 200, in the file the core writes.
fn project_with_a_pin(root: &Path) -> (App<MockRuntime>, WebviewWindow<MockRuntime>) {
    project_with_a_board(root);
    let (app, window) = canvas(root);
    apply_from(&window, &json!([a_move(400, 200)])).unwrap();
    (app, window)
}

fn a_release_of(element: &str, board: &str, x: i64, y: i64) -> Value {
    json!({
        "op": "move_element",
        "element": {"existing": element},
        "board": {"existing": board},
        "x": x,
        "y": y,
        "pinned": false
    })
}

fn a_release(x: i64, y: i64) -> Value {
    a_release_of("payment-service", "context", x, y)
}

fn context_file(root: &Path) -> String {
    fs::read_to_string(root.join("abyme/boards/context.board.json")).unwrap()
}

const RELEASED_ELSEWHERE: &str = "the canvas releases a pin on board";

const MIRRORED: &str = r#"{"mode": "mirrored", "statement": "current"}"#;
const FREE: &str = r#"{"mode": "free", "statement": "idea"}"#;

#[test]
fn a_move_on_a_mirrored_board_is_refused_and_writes_nothing() {
    let root = tempfile::tempdir().unwrap();
    project_with_a_board(root.path());
    make_context_a_zone(root.path(), MIRRORED);
    let before = project_files(root.path());
    let (_app, window) = canvas(root.path());

    let applied = apply_from(&window, &json!([a_move(400, 200)]));

    assert_eq!(
        applied,
        Err(json!(
            "operation 1: board context is mirrored, so the canvas may only create a note there, or move one and keep it pinned"
        ))
    );
    assert_eq!(project_files(root.path()), before);
}

#[test]
fn a_request_that_also_moves_on_a_mirrored_board_writes_nothing() {
    let root = tempfile::tempdir().unwrap();
    project_with_a_board(root.path());
    make_context_a_zone(root.path(), MIRRORED);
    add_a_free_board(root.path());
    let before = project_files(root.path());
    let (_app, window) = canvas(root.path());
    let on_the_free_board = json!({
        "op": "move_element",
        "element": {"existing": "payment-service"},
        "board": {"existing": "free-board"},
        "x": 1,
        "y": 2,
        "pinned": true
    });

    let applied = apply_from(&window, &json!([on_the_free_board, a_move(400, 200)]));

    assert_eq!(
        applied,
        Err(json!(
            "operation 2: board context is mirrored, so the canvas may only create a note there, or move one and keep it pinned"
        ))
    );
    assert_eq!(project_files(root.path()), before);
}

#[test]
fn a_release_at_the_file_s_position_changes_only_the_pin() {
    let root = tempfile::tempdir().unwrap();
    let (_app, window) = project_with_a_pin(root.path());
    let pinned = context_file(root.path());
    assert!(pinned.contains(", \"pinned\": true"), "{pinned}");

    apply_from(&window, &json!([a_release(400, 200)])).unwrap();

    assert_eq!(
        context_file(root.path()),
        pinned.replace(", \"pinned\": true", "")
    );
}

#[test]
fn a_release_of_a_box_without_a_pin_changes_no_file() {
    let root = tempfile::tempdir().unwrap();
    project_with_a_board(root.path());
    let (_app, window) = canvas(root.path());
    let view = open_board_from(&window, json!({"board": "context"})).unwrap();
    let payments = view["placements"]
        .as_array()
        .unwrap()
        .iter()
        .find(|placement| placement["element"] == json!("payment-service"))
        .unwrap();
    let release = a_release(
        payments["x"].as_i64().unwrap(),
        payments["y"].as_i64().unwrap(),
    );
    let before = project_files(root.path());

    apply_from(&window, &json!([release])).unwrap();

    assert_eq!(project_files(root.path()), before);
}

#[test]
fn a_release_anywhere_else_writes_nothing_and_names_position_and_board() {
    for zone in [None, Some(FREE)] {
        let root = tempfile::tempdir().unwrap();
        let (_app, window) = project_with_a_pin(root.path());
        if let Some(zone) = zone {
            make_context_a_zone(root.path(), zone);
        }
        let before = project_files(root.path());

        for (x, y) in [(401, 200), (400, 201), (0, 0)] {
            let error = apply_from(&window, &json!([a_release(x, y)]))
                .unwrap_err()
                .to_string();

            assert!(
                error.contains(&format!(
                    "operation 1: {RELEASED_ELSEWHERE} context only at the position in the board \
                     file"
                )),
                "{error}"
            );
            assert!(!error.contains("payment-service"), "{error}");
            assert_eq!(project_files(root.path()), before);
        }
    }
}

#[test]
fn a_release_of_an_element_the_board_does_not_place_writes_nothing() {
    let root = tempfile::tempdir().unwrap();
    let (_app, window) = project_with_a_pin(root.path());
    let before = project_files(root.path());

    let error = apply_from(
        &window,
        &json!([a_release_of("nowhere", "context", 400, 200)]),
    )
    .unwrap_err()
    .to_string();

    assert!(error.contains(RELEASED_ELSEWHERE), "{error}");
    assert_eq!(project_files(root.path()), before);
}

#[test]
fn a_release_of_an_element_whose_file_is_missing_writes_nothing_like_a_move_of_it() {
    let root = tempfile::tempdir().unwrap();
    project_with_a_board(root.path());
    let (_app, window) = canvas(root.path());
    let view = open_board_from(&window, json!({"board": "context"})).unwrap();
    let orders = view["placements"]
        .as_array()
        .unwrap()
        .iter()
        .find(|placement| placement["element"] == json!("orders"))
        .unwrap();
    let release = a_release_of(
        "orders",
        "context",
        orders["x"].as_i64().unwrap(),
        orders["y"].as_i64().unwrap(),
    );
    let before = project_files(root.path());

    let applied = apply_from(&window, &json!([release]));

    assert!(applied.is_err());
    assert_eq!(project_files(root.path()), before);
}

#[test]
fn a_release_with_the_position_of_another_board_writes_nothing() {
    let root = tempfile::tempdir().unwrap();
    let (_app, window) = project_with_a_pin(root.path());
    add_a_free_board(root.path());
    let before = project_files(root.path());

    let error = apply_from(
        &window,
        &json!([a_release_of("payment-service", "free-board", 400, 200)]),
    )
    .unwrap_err()
    .to_string();

    assert!(
        error.contains(&format!("{RELEASED_ELSEWHERE} free-board")),
        "{error}"
    );
    assert_eq!(project_files(root.path()), before);
}

#[test]
fn a_release_on_a_mirrored_board_is_refused() {
    let root = tempfile::tempdir().unwrap();
    let (_app, window) = project_with_a_pin(root.path());
    make_context_a_zone(root.path(), MIRRORED);
    let before = project_files(root.path());

    let error = apply_from(&window, &json!([a_release(400, 200)]))
        .unwrap_err()
        .to_string();

    assert!(
        error.contains(
            "board context is mirrored, so the canvas may only create a note there, or move one and keep it pinned"
        ),
        "{error}"
    );
    assert_eq!(project_files(root.path()), before);
}

#[test]
fn a_release_on_an_unknown_board_writes_nothing() {
    let root = tempfile::tempdir().unwrap();
    let (_app, window) = project_with_a_pin(root.path());
    let before = project_files(root.path());

    let applied = apply_from(
        &window,
        &json!([a_release_of("payment-service", "nowhere", 400, 200)]),
    );

    assert!(applied.is_err());
    assert_eq!(project_files(root.path()), before);
}

#[test]
fn a_move_with_a_release_at_its_new_position_writes_nothing() {
    let root = tempfile::tempdir().unwrap();
    let (_app, window) = project_with_a_pin(root.path());
    let before = project_files(root.path());

    let error = apply_from(&window, &json!([a_move(10, 20), a_release(10, 20)]))
        .unwrap_err()
        .to_string();

    assert!(
        error.contains(&format!("operation 2: {RELEASED_ELSEWHERE}")),
        "{error}"
    );
    assert_eq!(project_files(root.path()), before);
}

#[test]
fn a_move_with_a_release_at_the_file_s_position_ends_like_the_release_alone() {
    let root = tempfile::tempdir().unwrap();
    let (_app, window) = project_with_a_pin(root.path());
    let released = context_file(root.path()).replace(", \"pinned\": true", "");

    apply_from(&window, &json!([a_move(10, 20), a_release(400, 200)])).unwrap();

    assert_eq!(context_file(root.path()), released);
}

#[test]
fn a_request_with_one_good_release_and_one_elsewhere_writes_neither() {
    let root = tempfile::tempdir().unwrap();
    let (_app, window) = project_with_a_pin(root.path());
    let before = project_files(root.path());

    let error = apply_from(
        &window,
        &json!([
            a_release(400, 200),
            a_release_of("orders", "context", 999, 999)
        ]),
    )
    .unwrap_err()
    .to_string();

    assert!(
        error.contains(&format!("operation 2: {RELEASED_ELSEWHERE}")),
        "{error}"
    );
    assert_eq!(project_files(root.path()), before);
}

/// A move of `element` on `board` that says `pinned: true`.
fn a_move_of(element: &str, board: &str, x: i64, y: i64) -> Value {
    json!({
        "op": "move_element",
        "element": {"existing": element},
        "board": {"existing": board},
        "x": x,
        "y": y,
        "pinned": true
    })
}

/// `project_with_a_board` with the note `ask` on the board `context` too, which
/// is a mirrored zone. `ask` lies at 0, 0 on it.
fn mirrored_board_with_a_note(root: &Path) {
    project_with_a_board(root);
    let operations = json!([
        {"op": "create_element", "key": "n", "type_ref": "note", "name": "Ask",
         "body": "Why?\n", "statement": "idea"},
        {"op": "place_element", "element": {"new": "n"}, "board": {"existing": "context"}},
        {"op": "move_element", "element": {"new": "n"}, "board": {"existing": "context"},
         "x": 0, "y": 0}
    ]);
    let operations: Vec<Operation> = serde_json::from_value(operations).unwrap();
    Project::discover(root).unwrap().apply(&operations).unwrap();
    make_context_a_zone(root, MIRRORED);
}

/// The lines of `new` that are not in `old`, which has as many lines.
fn new_lines<'a>(old: &str, new: &'a str) -> Vec<&'a str> {
    assert_eq!(old.lines().count(), new.lines().count());
    new.lines().filter(|line| !old.contains(line)).collect()
}

#[test]
fn a_note_on_a_mirrored_board_moves_and_only_its_line_changes() {
    let root = tempfile::tempdir().unwrap();
    mirrored_board_with_a_note(root.path());
    fs::write(
        root.path().join("abyme/boards/free-board.board.json"),
        r#"{"format": 1, "id": "free-board", "title": "Free", "placements": [
            {"element": "ask", "x": 5, "y": 6, "width": 180, "height": 90}]}"#,
    )
    .unwrap();
    let context = PathBuf::from("abyme/boards/context.board.json");
    let mut others = project_files(root.path());
    let before = others.remove(&context).unwrap();
    let (_app, window) = canvas(root.path());

    apply_from(&window, &json!([a_move_of("ask", "context", 400, 200)])).unwrap();

    let mut files = project_files(root.path());
    let after = files.remove(&context).unwrap();
    assert_eq!(files, others);
    let before = String::from_utf8(before).unwrap();
    let after = String::from_utf8(after).unwrap();
    assert_eq!(
        new_lines(&before, &after),
        [
            r#"    {"element": "ask", "x": 400, "y": 200, "width": 180, "height": 90, "pinned": true},"#
        ]
    );
    assert!(after.contains(MIRRORED), "{after}");
}

#[test]
fn on_a_mirrored_board_no_move_but_a_note_s_is_written() {
    let root = tempfile::tempdir().unwrap();
    mirrored_board_with_a_note(root.path());
    let before = project_files(root.path());
    let (_app, window) = canvas(root.path());

    for refused in [
        a_move_of("payment-service", "context", 1, 2),
        a_move_of("orders", "context", 1, 2),
        a_release_of("ask", "context", 0, 0),
        a_release_of("ask", "context", 1, 2),
    ] {
        let error = apply_from(&window, &json!([refused]))
            .unwrap_err()
            .to_string();

        assert!(
            error.contains(
                "operation 1: board context is mirrored, so the canvas may only create a note there, or move one and keep it pinned"
            ),
            "{refused}: {error}"
        );
        for word in ["Ask", "Why", "payment", "orders"] {
            assert!(!error.contains(word), "{refused}: {error}");
        }
        assert_eq!(project_files(root.path()), before, "{refused}");
    }
}

#[test]
fn a_request_with_a_note_and_anything_else_on_a_mirrored_board_writes_nothing() {
    let root = tempfile::tempdir().unwrap();
    mirrored_board_with_a_note(root.path());
    add_a_free_board(root.path());
    let before = project_files(root.path());
    let (_app, window) = canvas(root.path());
    let note = a_move_of("ask", "context", 400, 200);
    let plain_box = a_move_of("payment-service", "context", 1, 2);
    let on_the_free_board = a_move_of("payment-service", "free-board", 1, 2);

    for (request, position) in [
        (json!([note, plain_box]), 2),
        (json!([plain_box, note]), 1),
        (json!([note, on_the_free_board, plain_box]), 3),
    ] {
        let error = apply_from(&window, &request).unwrap_err().to_string();

        assert!(
            error.contains(&format!("operation {position}: board context is mirrored")),
            "{request}: {error}"
        );
        assert_eq!(project_files(root.path()), before, "{request}");
    }
}

#[test]
fn a_move_of_a_note_from_a_stale_view_is_refused_by_what_the_file_says_now() {
    let root = tempfile::tempdir().unwrap();
    mirrored_board_with_a_note(root.path());
    let (_app, window) = canvas(root.path());
    open_board_from(&window, json!({"board": "context"})).unwrap();
    let file = root.path().join("abyme/elements/ask.md");

    fs::write(&file, "---\nid: ask\ntype: t\nname: Ask\n---\n").unwrap();
    let before = project_files(root.path());
    let retyped = apply_from(&window, &json!([a_move_of("ask", "context", 400, 200)]));
    assert_eq!(project_files(root.path()), before);
    assert!(retyped.unwrap_err().to_string().contains("is mirrored"));

    fs::remove_file(&file).unwrap();
    let before = project_files(root.path());
    let gone = apply_from(&window, &json!([a_move_of("ask", "context", 400, 200)]));
    assert_eq!(project_files(root.path()), before);
    assert!(gone.unwrap_err().to_string().contains("is mirrored"));
}

#[test]
fn on_a_mirrored_board_only_the_exact_type_note_moves() {
    for (type_ref, moves) in [
        ("note", true),
        ("Note", false),
        ("notes", false),
        ("c4/note", false),
    ] {
        let root = tempfile::tempdir().unwrap();
        project_with_one_element(root.path(), type_ref, "", "");
        let board = root.path().join("abyme/boards/context.board.json");
        let text = fs::read_to_string(&board).unwrap();
        let zoned = text.replace(
            "\"placements\"",
            &format!("\"zone\": {MIRRORED}, \"placements\""),
        );
        fs::write(&board, zoned).unwrap();
        let (_app, window) = canvas(root.path());

        let moved = apply_from(&window, &json!([a_move_of("one", "context", 1, 2)]));

        assert_eq!(moved.is_ok(), moves, "{type_ref}: {moved:?}");
    }
}

#[test]
fn a_move_of_a_note_the_mirrored_board_does_not_place_writes_nothing() {
    let root = tempfile::tempdir().unwrap();
    mirrored_board_with_a_note(root.path());
    fs::write(
        root.path().join("abyme/elements/elsewhere.md"),
        "---\nid: elsewhere\ntype: note\nname: N\n---\n",
    )
    .unwrap();
    let before = project_files(root.path());
    let (_app, window) = canvas(root.path());

    let applied = apply_from(&window, &json!([a_move_of("elsewhere", "context", 1, 2)]));

    assert!(
        applied
            .unwrap_err()
            .to_string()
            .contains("element elsewhere does not lie on board context")
    );
    assert_eq!(project_files(root.path()), before);
}

/// An update of the text of `element`.
fn an_update_of(element: &str, name: &str, body: &str) -> Value {
    json!({"op": "update_element", "element": {"existing": element}, "name": name, "body": body})
}

/// What the file of `one` holds now.
fn element_one(root: &Path) -> Element {
    let id: ElementId = "one".parse().unwrap();
    Project::discover(root)
        .unwrap()
        .snapshot()
        .unwrap()
        .elements
        .remove(&id)
        .unwrap()
}

const NOT_A_NOTE: &str = "operation 1: the canvas may change and delete only notes";
const MAY_NOT_SEND_AN_UPDATE: &str = "the canvas may not send update_element";

#[test]
fn an_update_changes_a_note_s_name_and_body_and_drops_only_its_stamp() {
    let root = tempfile::tempdir().unwrap();
    let stamp = "a".repeat(40);
    project_with_one_element(
        root.path(),
        "note",
        &format!(
            "summary: S\nanchor: src\nstatement: question\nverified: \"{stamp}\"\n\
             verified_by: ai\nrelations:\n  - {{kind: uses, to: other}}\n"
        ),
        "old\n",
    );
    let board = PathBuf::from("abyme/boards/context.board.json");
    let board_before = project_files(root.path()).remove(&board).unwrap();
    let before = element_one(root.path());
    assert!(before.verified().is_some());
    assert_ne!(before.relations(), &Relations::new());
    let (_app, window) = canvas(root.path());

    apply_from(
        &window,
        &json!([an_update_of("one", "New", "Line\nLine\n")]),
    )
    .unwrap();

    let one = element_one(root.path());
    assert_eq!(one.name(), "New");
    assert_eq!(one.body(), "Line\nLine\n");
    assert_eq!(one.summary(), Some("S"));
    assert_eq!(
        one.anchor().map(|anchor| anchor.as_str().to_owned()),
        Some("src".to_owned())
    );
    assert!(one.is_note());
    assert_eq!(one.statement(), Some(StatementKind::Question));
    assert_eq!(one.relations(), before.relations());
    assert_eq!(one.verified(), None);
    assert_eq!(
        project_files(root.path()).remove(&board).unwrap(),
        board_before
    );
}

#[test]
fn an_update_of_a_note_on_a_mirrored_board_writes_only_its_file() {
    let root = tempfile::tempdir().unwrap();
    mirrored_board_with_a_note(root.path());
    let element = PathBuf::from("abyme/elements/ask.md");
    let mut others = project_files(root.path());
    others.remove(&element).unwrap();
    let (_app, window) = canvas(root.path());

    apply_from(&window, &json!([an_update_of("ask", "Asked", "")])).unwrap();

    let mut files = project_files(root.path());
    let written = String::from_utf8(files.remove(&element).unwrap()).unwrap();
    assert!(written.contains("Asked"), "{written}");
    assert_eq!(files, others);
}

#[test]
fn an_update_with_anything_but_a_note_s_name_and_body_writes_nothing() {
    let root = tempfile::tempdir().unwrap();
    project_with_one_element(root.path(), "note", "summary: S\n", "SECRET-OLD\n");
    let before = project_files(root.path());
    let (_app, window) = canvas(root.path());
    let one = json!({"existing": "one"});
    let with = |field: &str, value: Value| {
        json!({"op": "update_element", "element": one, "name": "SECRET-NAME",
               "body": "SECRET-BODY", field: value})
    };

    for refused in [
        with("type_ref", json!("t")),
        with("summary", json!("SECRET-SUMMARY")),
        with("summary", Value::Null),
        with("anchor", json!("src")),
        with("anchor", Value::Null),
        with("statement", json!("idea")),
        with("statement", Value::Null),
        json!({"op": "update_element", "element": one, "name": "SECRET-NAME"}),
        json!({"op": "update_element", "element": one, "body": "SECRET-BODY"}),
        json!({"op": "update_element", "element": {"new": "k"}, "name": "N", "body": ""}),
        an_update_of("one", "", "SECRET-BODY"),
        an_update_of("one", " \t ", "SECRET-BODY"),
        an_update_of("one", "SECRET\nNAME", ""),
        an_update_of("one", "SECRET\rNAME", ""),
        an_update_of("one", "SECRET\u{85}NAME", ""),
        an_update_of("one", "SECRET\u{2028}NAME", ""),
        an_update_of("one", "SECRET\u{2029}NAME", ""),
        an_update_of("one", "N", "SECRET\u{0}BODY"),
        an_update_of("one", "N", "SECRET\u{1b}[31mBODY"),
    ] {
        let error = apply_from(&window, &json!([refused]))
            .unwrap_err()
            .to_string();

        assert!(error.contains(MAY_NOT_SEND_AN_UPDATE), "{refused}: {error}");
        assert!(!error.contains("SECRET"), "{refused}: {error}");
        assert_eq!(project_files(root.path()), before, "{refused}");
    }
}

#[test]
fn an_update_the_format_does_not_read_writes_nothing() {
    let root = tempfile::tempdir().unwrap();
    project_with_one_element(root.path(), "note", "", "");
    let before = project_files(root.path());
    let (_app, window) = canvas(root.path());
    let one = json!({"existing": "one"});

    for refused in [
        json!({"op": "update_element", "element": one, "name": Value::Null, "body": ""}),
        json!({"op": "update_element", "element": one, "name": "N", "body": Value::Null}),
        json!({"op": "update_element", "element": one, "name": "N", "body": "",
               "type_ref": Value::Null}),
        json!({"op": "update_element", "element": one, "name": "N", "body": "",
               "verified": "x"}),
    ] {
        assert!(apply_from(&window, &json!([refused])).is_err(), "{refused}");
        assert_eq!(project_files(root.path()), before, "{refused}");
    }
}

#[test]
fn an_update_of_what_is_no_note_writes_nothing_and_names_only_its_position() {
    for type_ref in ["t", "Note", "notes", "c4/note", "\"note \""] {
        let root = tempfile::tempdir().unwrap();
        project_with_one_element(root.path(), type_ref, "", "");
        let before = project_files(root.path());
        let (_app, window) = canvas(root.path());

        let refused = apply_from(&window, &json!([an_update_of("one", "SECRET", "")]));

        assert_eq!(refused, Err(json!(NOT_A_NOTE)), "{type_ref}");
        assert_eq!(project_files(root.path()), before, "{type_ref}");
    }
}

#[test]
fn an_update_of_a_note_from_a_stale_view_is_refused_by_what_the_file_says_now() {
    let root = tempfile::tempdir().unwrap();
    project_with_one_element(root.path(), "note", "", "");
    let (_app, window) = canvas(root.path());
    open_board_from(&window, json!({"board": "context"})).unwrap();
    let file = root.path().join("abyme/elements/one.md");

    fs::write(&file, "---\nid: one\ntype: t\nname: N\n---\n").unwrap();
    let before = project_files(root.path());
    let retyped = apply_from(&window, &json!([an_update_of("one", "New", "")]));
    assert_eq!(retyped, Err(json!(NOT_A_NOTE)));
    assert_eq!(project_files(root.path()), before);

    fs::remove_file(&file).unwrap();
    let before = project_files(root.path());
    let gone = apply_from(&window, &json!([an_update_of("one", "New", "")]));
    assert_eq!(gone, Err(json!(NOT_A_NOTE)));
    assert_eq!(project_files(root.path()), before);
}

#[test]
fn an_update_keeps_tabs_and_line_breaks_in_a_body() {
    let root = tempfile::tempdir().unwrap();
    project_with_one_element(root.path(), "note", "", "");
    let (_app, window) = canvas(root.path());
    let body = "a\tb\r\nc\u{85}d\n";

    apply_from(&window, &json!([an_update_of("one", "N", body)])).unwrap();

    assert_eq!(element_one(root.path()).body(), body);
}

#[test]
fn text_in_an_update_cannot_change_the_file_s_other_fields() {
    let root = tempfile::tempdir().unwrap();
    project_with_one_element(root.path(), "note", "statement: idea\n", "");
    let (_app, window) = canvas(root.path());
    let name = "a \"b\" \\ c: d # e --- f";
    let body = "---\nid: other\ntype: x\nanchor: src\n---\ntext\n";

    apply_from(&window, &json!([an_update_of("one", name, body)])).unwrap();

    let one = element_one(root.path());
    assert_eq!(one.name(), name);
    assert_eq!(one.body(), body);
    assert_eq!(one.id().as_str(), "one");
    assert!(one.is_note());
    assert_eq!(one.anchor(), None);
    assert_eq!(one.statement(), Some(StatementKind::Idea));
    assert_eq!(one.relations(), &Relations::new());
}

#[test]
fn the_core_refuses_an_update_it_cannot_write_and_shows_none_of_its_text() {
    let root = tempfile::tempdir().unwrap();
    project_with_one_element(root.path(), "note", "", "");
    let before = project_files(root.path());
    let (_app, window) = canvas(root.path());
    let long_body = "SECRET ".repeat(MAX_FILE_BYTES / 7 + 1);
    let long_name = "SECRET ".repeat(MAX_FRONTMATTER_BYTES / 7 + 1);

    for refused in [
        an_update_of("one", "SECRET\u{0}", ""),
        an_update_of("one", "N", &long_body),
        an_update_of("one", &long_name, ""),
    ] {
        let error = apply_from(&window, &json!([refused]))
            .unwrap_err()
            .to_string();

        assert!(
            !error.contains("SECRET"),
            "{}",
            &error[..error.len().min(200)]
        );
        assert_eq!(project_files(root.path()), before);
    }
}

#[test]
fn a_request_with_a_good_update_and_a_refused_one_writes_nothing() {
    let root = tempfile::tempdir().unwrap();
    mirrored_board_with_a_note(root.path());
    let before = project_files(root.path());
    let (_app, window) = canvas(root.path());
    let good = an_update_of("ask", "Asked", "");
    let moved = a_move_of("ask", "context", 400, 200);
    let refused = an_update_of("ask", "", "");
    let of_a_box = an_update_of("payment-service", "SECRET", "");

    for request in [
        json!([good, refused]),
        json!([refused, good]),
        json!([moved, refused]),
        json!([good, of_a_box]),
        json!([of_a_box, good]),
        json!([moved, of_a_box]),
    ] {
        assert!(apply_from(&window, &request).is_err(), "{request}");
        assert_eq!(project_files(root.path()), before, "{request}");
    }
}

/// The three operations that create a note on `board`, each to change in a test.
fn a_new_note(board: &str, name: &str, body: &str) -> Vec<Value> {
    vec![
        json!({"op": "create_element", "key": "note", "type_ref": "note", "name": name,
               "body": body, "statement": "idea"}),
        json!({"op": "place_element", "element": {"new": "note"}, "board": {"existing": board}}),
        json!({"op": "move_element", "element": {"new": "note"}, "board": {"existing": board},
               "x": 400, "y": 200, "pinned": true}),
    ]
}

/// What the file of the element `id` holds now.
fn element_of(root: &Path, id: &str) -> Element {
    let id: ElementId = id.parse().unwrap();
    Project::discover(root)
        .unwrap()
        .snapshot()
        .unwrap()
        .elements
        .remove(&id)
        .unwrap()
}

#[test]
fn a_new_note_writes_its_file_and_a_pinned_placement_and_returns_its_id() {
    let root = tempfile::tempdir().unwrap();
    project_with_a_board(root.path());
    let (_app, window) = canvas(root.path());

    let created = apply_from(
        &window,
        &json!(a_new_note("context", "Ask why", "It retries.\n")),
    );

    assert_eq!(created, Ok(json!(["ask-why"])));
    let note = element_of(root.path(), "ask-why");
    assert!(note.is_note());
    assert_eq!(note.name(), "Ask why");
    assert_eq!(note.body(), "It retries.\n");
    assert_eq!(note.statement(), Some(StatementKind::Idea));
    assert_eq!(note.summary(), None);
    assert_eq!(note.anchor(), None);
    assert_eq!(note.verified(), None);
    let board = context_file(root.path());
    assert!(
        board.contains(r#"{"element": "ask-why", "x": 400, "y": 200"#),
        "{board}"
    );
    assert!(
        board
            .lines()
            .any(|line| line.contains("ask-why") && line.contains(r#""pinned": true"#)),
        "{board}"
    );
}

#[test]
fn a_move_and_an_update_return_no_ids() {
    let root = tempfile::tempdir().unwrap();
    project_with_a_board(root.path());
    let (_app, window) = canvas(root.path());
    apply_from(&window, &json!(a_new_note("context", "Ask", ""))).unwrap();

    assert_eq!(apply_from(&window, &json!([a_move(1, 2)])), Ok(json!([])));
    assert_eq!(
        apply_from(&window, &json!([an_update_of("ask", "Asked", "")])),
        Ok(json!([]))
    );
}

#[test]
fn a_new_note_on_a_mirrored_board_leaves_its_other_lines_and_zone() {
    let root = tempfile::tempdir().unwrap();
    mirrored_board_with_a_note(root.path());
    let before = context_file(root.path());
    let (_app, window) = canvas(root.path());

    let created = apply_from(&window, &json!(a_new_note("context", "Second", "")));

    assert_eq!(created, Ok(json!(["second"])));
    let after = context_file(root.path());
    assert_eq!(after.lines().count(), before.lines().count() + 1);
    for line in before.lines() {
        // The placement that was the last gains a comma.
        assert!(
            after.contains(line.trim_end_matches(',')),
            "{line}: {after}"
        );
    }
    assert!(after.contains(MIRRORED), "{after}");
}

/// `base` with `field` set to `value`.
fn with_field(base: &Value, field: &str, value: Value) -> Value {
    let mut changed = base.clone();
    changed[field] = value;
    changed
}

/// `base` without `field`.
fn without_field(base: &Value, field: &str) -> Value {
    let mut changed = base.clone();
    changed.as_object_mut().unwrap().remove(field);
    changed
}

/// Sends each of `shapes` to a project with a mirrored board `context` and a
/// free board `free-board`, and checks that it is refused, writes nothing, and
/// shows no text of the request.
fn assert_each_writes_nothing(shapes: &[Vec<Value>]) {
    let root = tempfile::tempdir().unwrap();
    mirrored_board_with_a_note(root.path());
    add_a_free_board(root.path());
    let before = project_files(root.path());
    let (_app, window) = canvas(root.path());

    for shape in shapes {
        let request = json!(shape);
        let error = apply_from(&window, &request).unwrap_err().to_string();

        assert!(!error.contains("SECRET"), "{request}: {error}");
        assert_eq!(project_files(root.path()), before, "{request}");
    }
}

/// The three operations that create a note on `context`, with text a test can
/// look for.
fn a_new_note_in_parts() -> [Value; 3] {
    let good = a_new_note("context", "SECRET-NAME", "SECRET-BODY");
    [good[0].clone(), good[1].clone(), good[2].clone()]
}

#[test]
fn a_new_note_with_anything_beside_it_writes_nothing() {
    let [create, place, pin] = a_new_note_in_parts();
    let triple = [create.clone(), place.clone(), pin.clone()];
    let beside = [
        a_move_of("payment-service", "context", 1, 2),
        an_update_of("payment-service", "SECRET-NAME", "SECRET-BODY"),
        a_release_of("ask", "context", 9, 9),
    ];
    let mut shapes: Vec<Vec<Value>> = vec![
        vec![create.clone()],
        vec![place.clone()],
        vec![pin.clone()],
        vec![create.clone(), place.clone()],
        vec![create.clone(), pin.clone()],
        vec![place.clone(), create.clone(), pin.clone()],
        vec![create, pin, place],
        triple
            .iter()
            .cloned()
            .chain(a_new_note("free-board", "SECRET-NAME", ""))
            .collect(),
    ];
    for other in beside {
        shapes.push(
            std::iter::once(other.clone())
                .chain(triple.iter().cloned())
                .collect(),
        );
        shapes.push(triple.iter().cloned().chain([other]).collect());
    }

    assert_each_writes_nothing(&shapes);
}

#[test]
fn a_new_note_with_another_key_element_or_board_writes_nothing() {
    let [create, place, pin] = a_new_note_in_parts();

    assert_each_writes_nothing(&[
        vec![
            create.clone(),
            with_field(&place, "element", json!({"new": "other"})),
            pin.clone(),
        ],
        vec![
            create.clone(),
            place.clone(),
            with_field(&pin, "element", json!({"new": "other"})),
        ],
        vec![
            create.clone(),
            with_field(&place, "element", json!({"existing": "ask"})),
            pin.clone(),
        ],
        vec![
            create.clone(),
            place.clone(),
            with_field(&pin, "element", json!({"existing": "ask"})),
        ],
        vec![
            create.clone(),
            place.clone(),
            with_field(&pin, "board", json!({"existing": "free-board"})),
        ],
        vec![
            create,
            with_field(&place, "board", json!({"new": "b"})),
            pin,
        ],
    ]);
}

#[test]
fn a_new_note_with_any_other_field_of_its_element_writes_nothing() {
    let [create, place, pin] = a_new_note_in_parts();
    let elements = [
        with_field(&create, "type_ref", json!("t")),
        with_field(&create, "type_ref", json!("Note")),
        with_field(&create, "summary", json!("SECRET-SUMMARY")),
        with_field(&create, "summary", json!("")),
        with_field(&create, "anchor", json!("src")),
        with_field(&create, "anchor", json!("")),
        without_field(&create, "statement"),
        with_field(&create, "statement", Value::Null),
        with_field(&create, "statement", json!("current")),
        with_field(&create, "name", json!("")),
        with_field(&create, "name", json!(" \t ")),
        with_field(&create, "name", json!("SECRET\nNAME")),
        with_field(&create, "name", json!("SECRET\rNAME")),
        with_field(&create, "name", json!("SECRET\u{85}NAME")),
        with_field(&create, "name", json!("SECRET\u{2028}NAME")),
        with_field(&create, "name", json!("SECRET\u{2029}NAME")),
        with_field(&create, "body", json!("SECRET\u{0}BODY")),
        with_field(&create, "body", json!("SECRET\u{1b}BODY")),
    ];
    let shapes: Vec<Vec<Value>> = elements
        .into_iter()
        .map(|element| vec![element, place.clone(), pin.clone()])
        .chain([
            vec![
                create.clone(),
                place.clone(),
                with_field(&pin, "pinned", json!(false)),
            ],
            vec![create, place.clone(), without_field(&pin, "pinned")],
        ])
        .collect();

    assert_each_writes_nothing(&shapes);
}

#[test]
fn a_new_note_with_a_field_the_format_does_not_read_writes_nothing() {
    let root = tempfile::tempdir().unwrap();
    project_with_a_board(root.path());
    let before = project_files(root.path());
    let (_app, window) = canvas(root.path());

    for position in 0..3 {
        let mut shape = a_new_note("context", "N", "");
        shape[position]["verified"] = json!("x");

        assert!(apply_from(&window, &json!(shape)).is_err(), "{position}");
        assert_eq!(project_files(root.path()), before, "{position}");
    }
}

#[test]
fn a_new_note_beyond_the_core_s_bound_or_on_an_unknown_board_writes_nothing() {
    let root = tempfile::tempdir().unwrap();
    project_with_a_board(root.path());
    let before = project_files(root.path());
    let (_app, window) = canvas(root.path());
    let mut far_away = a_new_note("context", "SECRET-NAME", "SECRET-BODY");
    far_away[2]["x"] = json!(1_000_001);

    for shape in [
        far_away,
        a_new_note("nowhere", "SECRET-NAME", "SECRET-BODY"),
    ] {
        let error = apply_from(&window, &json!(shape)).unwrap_err().to_string();

        assert!(!error.contains("SECRET"), "{error}");
        assert_eq!(project_files(root.path()), before);
    }
}

#[test]
fn a_new_note_s_name_cannot_choose_a_place_outside_its_directory() {
    let root = tempfile::tempdir().unwrap();
    project_with_a_board(root.path());
    let outside = root.path().join("x.md");
    let (_app, window) = canvas(root.path());

    let created = apply_from(&window, &json!(a_new_note("context", "../../x", "")));

    assert_eq!(created, Ok(json!(["x"])));
    assert!(root.path().join("abyme/elements/x.md").exists());
    assert!(!outside.exists());
}

#[test]
fn a_new_note_with_the_name_of_an_existing_id_gets_another_one_and_leaves_it() {
    let root = tempfile::tempdir().unwrap();
    project_with_a_board(root.path());
    let existing = PathBuf::from("abyme/elements/payment-service.md");
    let before = project_files(root.path()).remove(&existing).unwrap();
    let (_app, window) = canvas(root.path());

    let same_as_a_file = apply_from(
        &window,
        &json!(a_new_note("context", "Payment Service", "")),
    );
    // `orders` lies on the board, but its file is missing.
    let same_as_a_placement = apply_from(&window, &json!(a_new_note("context", "Orders", "")));

    assert_eq!(same_as_a_file, Ok(json!(["payment-service-2"])));
    assert_eq!(same_as_a_placement, Ok(json!(["orders-2"])));

    // `ghost` only owns a board; it has neither a file nor a placement.
    fs::write(
        root.path().join("abyme/boards/owned.board.json"),
        r#"{"format": 1, "id": "owned", "title": "Owned", "owner": "ghost", "placements": []}"#,
    )
    .unwrap();
    let same_as_an_owner = apply_from(&window, &json!(a_new_note("context", "Ghost", "")));
    assert_eq!(same_as_an_owner, Ok(json!(["ghost-2"])));
    assert_eq!(
        project_files(root.path()).remove(&existing).unwrap(),
        before
    );
}

#[test]
fn text_in_a_new_note_cannot_change_its_other_fields() {
    let root = tempfile::tempdir().unwrap();
    project_with_a_board(root.path());
    let (_app, window) = canvas(root.path());
    let name = "type: board: \"a\" \\ # b";
    let body = "---\nid: other\ntype: x\nanchor: src\n---\n<script>alert(1)</script>\n";

    let created = apply_from(&window, &json!(a_new_note("context", name, body))).unwrap();

    let id = created[0].as_str().unwrap();
    let note = element_of(root.path(), id);
    assert_eq!(note.id().as_str(), id);
    assert_eq!(note.name(), name);
    assert_eq!(note.body(), body);
    assert!(note.is_note());
    assert_eq!(note.statement(), Some(StatementKind::Idea));
    assert_eq!(note.anchor(), None);
    assert_eq!(note.relations(), &Relations::new());
}

fn a_deletion_of(element: &str) -> Value {
    json!({"op": "delete_element", "element": {"existing": element}})
}

/// `mirrored_board_with_a_note`, with a free board `free` that places `ask` and
/// `payment-service` too, both board files as the core writes them.
fn a_note_on_two_boards(root: &Path) {
    mirrored_board_with_a_note(root);
    fs::write(
        root.join("abyme/boards/free.board.json"),
        r#"{"format": 1, "id": "free", "title": "Free", "placements": [
            {"element": "ask", "x": 5, "y": 6, "width": 180, "height": 90},
            {"element": "payment-service", "x": 300, "y": 6, "width": 180, "height": 90}]}"#,
    )
    .unwrap();
    // A move makes the core write the file in its own form.
    let operations: Vec<Operation> =
        serde_json::from_value(json!([a_move_of("payment-service", "free", 300, 6)])).unwrap();
    Project::discover(root).unwrap().apply(&operations).unwrap();
}

/// `text` without its lines that name `element`, as a board file is written
/// without that element.
fn without_lines_of(text: &str, element: &str) -> String {
    let named = format!("\"{element}\"");
    text.lines()
        .filter(|line| !line.contains(&named))
        .flat_map(|line| [line, "\n"])
        .collect()
}

#[test]
fn a_deletion_takes_a_note_from_every_board_and_changes_nothing_else() {
    let root = tempfile::tempdir().unwrap();
    a_note_on_two_boards(root.path());
    let mut before = project_files(root.path());
    let context = PathBuf::from("abyme/boards/context.board.json");
    let free = PathBuf::from("abyme/boards/free.board.json");
    let context_before = String::from_utf8(before.remove(&context).unwrap()).unwrap();
    let free_before = String::from_utf8(before.remove(&free).unwrap()).unwrap();
    before
        .remove(&PathBuf::from("abyme/elements/ask.md"))
        .unwrap();
    let (_app, window) = canvas(root.path());

    let deleted = apply_from(&window, &json!([a_deletion_of("ask")]));

    assert_eq!(deleted, Ok(json!([])));
    let mut after = project_files(root.path());
    let context_after = String::from_utf8(after.remove(&context).unwrap()).unwrap();
    let free_after = String::from_utf8(after.remove(&free).unwrap()).unwrap();
    assert_eq!(after, before);
    assert_eq!(context_after, without_lines_of(&context_before, "ask"));
    assert_eq!(free_after, without_lines_of(&free_before, "ask"));
    assert!(context_after.contains(MIRRORED), "{context_after}");
}

#[test]
fn a_deletion_of_what_is_no_note_writes_nothing_and_names_only_its_position() {
    let root = tempfile::tempdir().unwrap();
    a_note_on_two_boards(root.path());
    let before = project_files(root.path());
    let (_app, window) = canvas(root.path());

    // `payment-service` is a box; `orders` lies on the board, but its file is
    // missing, so its placements stay.
    for element in ["payment-service", "orders"] {
        let refused = apply_from(&window, &json!([a_deletion_of(element)]));

        assert_eq!(refused, Err(json!(NOT_A_NOTE)), "{element}");
        assert_eq!(project_files(root.path()), before, "{element}");
    }
    let new = apply_from(
        &window,
        &json!([{"op": "delete_element", "element": {"new": "k"}}]),
    );
    assert!(
        new.unwrap_err()
            .to_string()
            .contains("the canvas may not send delete_element"),
    );
    assert_eq!(project_files(root.path()), before);
}

#[test]
fn a_deletion_of_a_type_that_only_resembles_note_writes_nothing() {
    for type_ref in ["Note", "notes", "c4/note"] {
        let root = tempfile::tempdir().unwrap();
        project_with_one_element(root.path(), type_ref, "", "");
        let before = project_files(root.path());
        let (_app, window) = canvas(root.path());

        let refused = apply_from(&window, &json!([a_deletion_of("one")]));

        assert_eq!(refused, Err(json!(NOT_A_NOTE)), "{type_ref}");
        assert_eq!(project_files(root.path()), before, "{type_ref}");
    }
}

#[test]
fn a_deletion_from_a_stale_view_is_refused_by_what_the_file_says_now() {
    let root = tempfile::tempdir().unwrap();
    project_with_one_element(root.path(), "note", "", "");
    let (_app, window) = canvas(root.path());
    open_board_from(&window, json!({"board": "context"})).unwrap();
    fs::write(
        root.path().join("abyme/elements/one.md"),
        "---\nid: one\ntype: t\nname: N\n---\n",
    )
    .unwrap();
    let before = project_files(root.path());

    let refused = apply_from(&window, &json!([a_deletion_of("one")]));

    assert_eq!(refused, Err(json!(NOT_A_NOTE)));
    assert_eq!(project_files(root.path()), before);
}

#[test]
fn a_note_that_another_element_relates_to_or_that_owns_a_board_stays() {
    let root = tempfile::tempdir().unwrap();
    mirrored_board_with_a_note(root.path());
    let operations = json!([
        {"op": "create_relation", "from": {"existing": "payment-service"}, "kind": "uses",
         "to": {"existing": "ask"}}
    ]);
    let operations: Vec<Operation> = serde_json::from_value(operations).unwrap();
    Project::discover(root.path())
        .unwrap()
        .apply(&operations)
        .unwrap();
    let before = project_files(root.path());
    let (_app, window) = canvas(root.path());

    let related = apply_from(&window, &json!([a_deletion_of("ask")]))
        .unwrap_err()
        .to_string();

    assert!(related.contains("payment-service"), "{related}");
    assert!(related.contains("uses"), "{related}");
    assert_eq!(project_files(root.path()), before);

    let owner = tempfile::tempdir().unwrap();
    project_with_one_element(owner.path(), "note", "", "");
    fs::write(
        owner.path().join("abyme/boards/inside.board.json"),
        r#"{"format": 1, "id": "inside", "title": "Inside", "owner": "one", "placements": []}"#,
    )
    .unwrap();
    let before = project_files(owner.path());
    let (_app, window) = canvas(owner.path());

    let owning = apply_from(&window, &json!([a_deletion_of("one")]))
        .unwrap_err()
        .to_string();

    assert!(owning.contains("owns board inside"), "{owning}");
    assert_eq!(project_files(owner.path()), before);
}

#[test]
fn a_deletion_goes_with_other_admitted_operations_and_fails_with_a_refused_one() {
    let root = tempfile::tempdir().unwrap();
    project_with_a_board(root.path());
    let (_app, window) = canvas(root.path());
    apply_from(&window, &json!(a_new_note("context", "Ask", ""))).unwrap();
    apply_from(&window, &json!(a_new_note("context", "Other", ""))).unwrap();

    apply_from(&window, &json!([a_move(10, 20), a_deletion_of("ask")])).unwrap();
    assert!(!root.path().join("abyme/elements/ask.md").exists());

    let before = project_files(root.path());
    let refused = apply_from(
        &window,
        &json!([a_deletion_of("other"), a_deletion_of("payment-service")]),
    );
    assert_eq!(
        refused,
        Err(json!(
            "operation 2: the canvas may change and delete only notes"
        ))
    );
    assert_eq!(project_files(root.path()), before);
    assert!(root.path().join("abyme/elements/other.md").exists());
}

#[test]
fn a_move_on_a_free_board_writes_and_keeps_the_zone() {
    let root = tempfile::tempdir().unwrap();
    project_with_a_board(root.path());
    make_context_a_zone(root.path(), FREE);
    let (_app, window) = canvas(root.path());

    let applied = apply_from(&window, &json!([a_move(400, 200)]));

    assert_eq!(applied, Ok(json!([])));
    let written = fs::read_to_string(root.path().join("abyme/boards/context.board.json")).unwrap();
    assert!(
        written.contains(&format!("  \"zone\": {FREE},\n")),
        "{written}"
    );
    assert!(written.contains("\"x\": 400, \"y\": 200"), "{written}");
}

#[test]
fn a_board_file_that_does_not_read_refuses_the_move_and_writes_nothing() {
    for zone in [
        r#"{"mode": "locked", "statement": "current"}"#,
        r#"{"mode": {"free": null}, "statement": "current"}"#,
    ] {
        let root = tempfile::tempdir().unwrap();
        project_with_a_board(root.path());
        make_context_a_zone(root.path(), zone);
        let before = project_files(root.path());
        let (_app, window) = canvas(root.path());

        let applied = apply_from(&window, &json!([a_move(400, 200)]));

        let error = applied.unwrap_err().to_string();
        assert!(error.contains("board context"), "{zone}: {error}");
        assert_eq!(project_files(root.path()), before, "{zone}");
    }
}

#[test]
fn a_move_on_an_unknown_board_still_gets_the_core_s_answer() {
    let root = tempfile::tempdir().unwrap();
    project_with_a_board(root.path());
    let before = project_files(root.path());
    let (_app, window) = canvas(root.path());
    let on_no_board = json!({
        "op": "move_element",
        "element": {"existing": "payment-service"},
        "board": {"existing": "nowhere"},
        "x": 1,
        "y": 2,
        "pinned": true
    });

    let applied = apply_from(&window, &json!([on_no_board]));

    let error = applied.unwrap_err().to_string();
    assert!(
        error.contains("nowhere") && !error.contains("mirrored"),
        "{error}"
    );
    assert_eq!(project_files(root.path()), before);
}

#[test]
fn an_operation_the_canvas_may_not_send_gets_that_answer_on_a_mirrored_board_too() {
    let root = tempfile::tempdir().unwrap();
    project_with_a_board(root.path());
    make_context_a_zone(root.path(), MIRRORED);
    let (_app, window) = canvas(root.path());

    let applied = apply_from(
        &window,
        &json!([a_move(400, 200), {"op": "create_board", "key": "b", "title": "T"}]),
    );

    assert_eq!(
        applied,
        Err(json!("operation 2: the canvas may not send create_board"))
    );
}

#[test]
fn open_board_says_the_board_s_mode_and_not_its_statement_kind() {
    for (zone, expected) in [
        (Some(MIRRORED), json!("mirrored")),
        (Some(FREE), json!("free")),
        (None, Value::Null),
    ] {
        let root = tempfile::tempdir().unwrap();
        project_with_a_board(root.path());
        if let Some(zone) = zone {
            make_context_a_zone(root.path(), zone);
        }
        let (_app, window) = canvas(root.path());

        let view = open_board_from(&window, json!({"board": "context"})).unwrap();

        assert_eq!(view["mode"], expected, "{zone:?}");
        assert!(!view.to_string().contains("statement"), "{view}");
    }
}

#[test]
fn a_request_with_a_refused_operation_writes_nothing() {
    let root = tempfile::tempdir().unwrap();
    project_with_a_board(root.path());
    let before = project_files(root.path());
    let (_app, window) = canvas(root.path());

    let applied = apply_from(
        &window,
        &json!([a_move(400, 200), {"op": "create_board", "key": "b", "title": "SECRET-TITLE"}]),
    );

    assert_eq!(
        applied,
        Err(json!("operation 2: the canvas may not send create_board"))
    );
    assert_eq!(project_files(root.path()), before);
}

#[test]
fn every_kind_the_canvas_may_not_send_is_refused_through_ipc() {
    let root = tempfile::tempdir().unwrap();
    project_with_a_board(root.path());
    let before = project_files(root.path());
    let (_app, window) = canvas(root.path());

    for (operation, kind) in [
        (
            json!({"op": "create_element", "key": "e", "type_ref": "c4/container", "name": "SECRET-NAME"}),
            "create_element",
        ),
        (
            json!({"op": "create_board", "key": "b", "title": "SECRET-TITLE"}),
            "create_board",
        ),
        (
            json!({"op": "place_element", "element": {"existing": "payment-service"}, "board": {"existing": "context"}}),
            "place_element",
        ),
        (
            json!({"op": "move_element", "element": {"new": "e"}, "board": {"existing": "context"}, "x": 1, "y": 2}),
            "move_element with a new element or board",
        ),
        (
            json!({"op": "move_element", "element": {"existing": "payment-service"}, "board": {"new": "b"}, "x": 1, "y": 2}),
            "move_element with a new element or board",
        ),
        (
            json!({"op": "move_element", "element": {"new": "e"}, "board": {"new": "b"}, "x": 1, "y": 2}),
            "move_element with a new element or board",
        ),
    ] {
        assert_eq!(
            apply_from(&window, &json!([operation])),
            Err(json!(format!(
                "operation 1: the canvas may not send {kind}"
            ))),
            "{operation}"
        );
    }
    assert_eq!(project_files(root.path()), before);
}

#[test]
fn apply_refuses_before_looking_for_a_project() {
    let root = tempfile::tempdir().unwrap();
    let (_app, window) = canvas(root.path());
    let too_many: Vec<Value> = (0..=MAX_OPERATIONS).map(|_| a_move(1, 2)).collect();

    let refused_kind = apply_from(
        &window,
        &json!([{"op": "create_board", "key": "b", "title": "x"}]),
    );
    let refused_count = apply_from(&window, &json!(too_many));
    let admitted_count = apply_from(&window, &json!([a_move(1, 2)]));

    assert_eq!(
        refused_kind,
        Err(json!("operation 1: the canvas may not send create_board"))
    );
    assert_eq!(
        refused_count,
        Err(json!(format!(
            "the request has {} operations, more than {MAX_OPERATIONS}",
            MAX_OPERATIONS + 1
        )))
    );
    assert_eq!(
        admitted_count,
        Err(json!(ProjectError::NotFound.to_string()))
    );
}

#[test]
fn apply_rejects_what_the_core_s_reader_does_not_take() {
    let root = tempfile::tempdir().unwrap();
    project_with_a_board(root.path());
    let before = project_files(root.path());
    let (_app, window) = canvas(root.path());
    let with = |field: &str, value: Value| {
        let mut operation = a_move(1, 2);
        operation[field] = value;
        operation
    };

    for operations in [
        json!([["move_element", {"existing": "payment-service"}, {"existing": "context"}, 1, 2]]),
        json!([with("width", json!(500))]),
        json!([with("x", json!(1.5))]),
        json!([with("x", Value::Null)]),
        json!({"op": "move_element"}),
    ] {
        let message = apply_from(&window, &operations).expect_err(&operations.to_string());
        assert!(
            message.to_string().contains("invalid args"),
            "{operations}: {message}"
        );
    }
    assert_eq!(project_files(root.path()), before);
}

#[test]
fn a_move_beyond_the_core_s_bound_writes_nothing() {
    let root = tempfile::tempdir().unwrap();
    project_with_a_board(root.path());
    let before = project_files(root.path());
    let (_app, window) = canvas(root.path());

    let applied = apply_from(&window, &json!([a_move(1_000_001, 0)]));

    let message = applied.expect_err("a move beyond the bound was applied");
    assert!(
        message.to_string().starts_with("\"operation 1:"),
        "{message}"
    );
    assert_eq!(project_files(root.path()), before);
}

#[test]
fn a_remote_page_may_not_apply() {
    let root = tempfile::tempdir().unwrap();
    project_with_a_board(root.path());
    let before = project_files(root.path());
    let (_app, window) = canvas(root.path());

    let response = invoke(
        &window,
        "https://example.com/",
        "apply",
        json!({"operations": [a_move(400, 200)]}),
    );

    let message = response.expect_err("a remote page applied a move");
    assert!(message.to_string().contains("not allowed"), "{message}");
    assert_eq!(project_files(root.path()), before);
}

#[test]
fn the_page_may_listen_to_events() {
    let root = tempfile::tempdir().unwrap();
    let (_app, window) = canvas(root.path());

    let listened = invoke(
        &window,
        APP_ORIGIN,
        "plugin:event|listen",
        json!({"event": PROJECT_CHANGED, "target": {"kind": "Any"}, "handler": 7}),
    );

    assert!(listened.is_ok(), "{listened:?}");
}

#[test]
fn the_page_may_call_nothing_else() {
    let root = tempfile::tempdir().unwrap();
    let (_app, window) = canvas(root.path());

    let refused = [
        ("plugin:event|emit", json!({"event": PROJECT_CHANGED})),
        ("plugin:path|resolve_directory", json!({"directory": 1})),
        (
            "plugin:webview|create_webview_window",
            json!({"options": {"label": "other"}}),
        ),
        ("plugin:window|set_title", json!({"title": "x"})),
        ("delete_board", json!({})),
    ];

    for (command, body) in refused {
        let response = invoke(&window, APP_ORIGIN, command, body);
        let message = response.expect_err(command);
        assert!(
            message.to_string().contains("not allowed"),
            "{command}: {message}"
        );
    }
}

#[test]
fn a_remote_page_may_not_list_boards() {
    let root = tempfile::tempdir().unwrap();
    project_with_boards(root.path(), &["Context"]);
    let (_app, window) = canvas(root.path());

    let response = invoke(&window, "https://example.com/", "list_boards", json!({}));

    let message = response.expect_err("a remote page listed the boards");
    assert!(message.to_string().contains("not allowed"), "{message}");
}

#[test]
fn the_app_includes_no_tauri_plugin() {
    let manifest = fs::read_to_string(Path::new(CRATE).join("Cargo.toml")).unwrap();
    let package = fs::read_to_string(Path::new(CRATE).join("ui/package.json")).unwrap();

    assert!(!manifest.contains("tauri-plugin"));
    assert!(!package.contains("@tauri-apps/plugin-"));
}

#[test]
fn the_capability_grants_the_canvas_s_commands_and_listening_only() {
    let capability = read_json("capabilities/default.json");

    assert_eq!(capability["windows"], json!(["main"]));
    assert_eq!(
        capability["permissions"],
        json!([
            "core:event:allow-listen",
            "core:event:allow-unlisten",
            "allow-list-boards",
            "allow-open-board",
            "allow-apply",
            "allow-review-flags"
        ])
    );
    assert_eq!(capability.get("remote"), None);
}

#[test]
fn the_configuration_keeps_the_webview_locked_down() {
    let config = read_json("tauri.conf.json");
    let security = &config["app"]["security"];
    let policy = security["csp"].as_str().unwrap();

    assert_eq!(config["app"]["withGlobalTauri"], json!(false));
    assert_eq!(
        security["freezePrototype"],
        json!(false),
        "ui/src/main.tsx freezes Object.prototype once the page's modules have loaded; \
         Tauri's flag freezes it earlier, which breaks d3-color, which React Flow loads"
    );
    assert_eq!(security["capabilities"], json!(["default"]));
    assert!(!script_sources(policy).contains("'unsafe-inline'"));
    assert!(!script_sources(policy).contains("'unsafe-eval'"));
}

#[test]
fn the_page_carries_its_own_content_security_policy() {
    let page = fs::read_to_string(Path::new(CRATE).join("ui/index.html")).unwrap();
    let (_, meta) = page
        .split_once(r#"http-equiv="Content-Security-Policy""#)
        .unwrap();
    let (_, content) = meta.split_once(r#"content=""#).unwrap();
    let (policy, _) = content.split_once('"').unwrap();

    assert!(script_sources(policy).starts_with("script-src 'self'"));
    assert!(!script_sources(policy).contains("'unsafe-inline'"));
    assert!(!script_sources(policy).contains("'unsafe-eval'"));
}
