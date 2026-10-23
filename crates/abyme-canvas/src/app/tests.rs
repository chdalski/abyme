//! Tests of the IPC boundary through Tauri's own message handling and the
//! app's real configuration and access list, and of that configuration.

use super::*;
use std::collections::BTreeMap;
use std::fs;

use abyme_core::{LocalKey, MAX_OPERATIONS, Operation, ProjectError, UnknownBoard};
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
        "y": y
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

    assert_eq!(applied, Ok(Value::Null));
    let after = placements(&project);
    let (moved, was) = (&after[&payments], &before[&payments]);
    assert_eq!((moved.x, moved.y), (400, -200));
    assert_eq!((moved.width, moved.height), (was.width, was.height));
    assert_eq!(after[&orders], before[&orders]);
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
            "allow-apply"
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
