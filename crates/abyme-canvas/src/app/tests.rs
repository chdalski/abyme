//! Tests of the IPC boundary through Tauri's own message handling and the
//! app's real configuration and access list, and of that configuration.

use super::*;
use std::fs;

use abyme_core::{LocalKey, Operation, ProjectError};
use serde_json::{Value, json};
use tauri::ipc::{CallbackFn, InvokeBody};
use tauri::test::{INVOKE_KEY, MockRuntime, get_ipc_response, mock_builder};
use tauri::webview::InvokeRequest;
use tauri::{App, WebviewWindow, WebviewWindowBuilder};

/// The origin the canvas's own page has while it is developed.
const APP_ORIGIN: &str = "http://localhost:5173/";

/// This crate's directory, where its configuration lies.
const CRATE: &str = env!("CARGO_MANIFEST_DIR");

/// Builds the canvas as `main` does, on Tauri's mock runtime, started in
/// `start`.
fn canvas(start: &Path) -> (App<MockRuntime>, WebviewWindow<MockRuntime>) {
    let app = configure(mock_builder(), start.to_path_buf())
        .build(tauri::generate_context!())
        .unwrap();
    let [window] = app.config().app.windows.as_slice() else {
        panic!("the configuration must declare exactly one window");
    };
    let window = WebviewWindowBuilder::from_config(&app, window)
        .unwrap()
        .build()
        .unwrap();
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
fn the_capability_grants_the_list_of_boards_and_listening_only() {
    let capability = read_json("capabilities/default.json");

    assert_eq!(capability["windows"], json!(["main"]));
    assert_eq!(
        capability["permissions"],
        json!([
            "core:event:allow-listen",
            "core:event:allow-unlisten",
            "allow-list-boards"
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
    assert_eq!(security["freezePrototype"], json!(true));
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
