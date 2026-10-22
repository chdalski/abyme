//! The built `abyme-mcp` binary: `init` on the command line, and the MCP
//! server on standard input and output, driven by an MCP client.

use std::collections::BTreeMap;
use std::error::Error;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use rmcp::model::{CallToolRequestParams, CallToolResult, ErrorCode, JsonObject};
use rmcp::service::RunningService;
use rmcp::{RoleClient, ServiceError, ServiceExt as _};
use serde_json::{Value, json};
use tokio::io::{AsyncBufReadExt as _, AsyncWriteExt as _, BufReader};
use tokio::process::Child;
use tokio::task::JoinHandle;

type TestResult<T = ()> = Result<T, Box<dyn Error>>;

/// How long a call may take before the test gives up.
const TIMEOUT: Duration = Duration::from_secs(30);

const BINARY: &str = env!("CARGO_BIN_EXE_abyme-mcp");

/// A running server with an MCP client connected to it, and every line the
/// server wrote to standard output.
struct Session {
    client: RunningService<RoleClient, ()>,
    child: Child,
    stdout_lines: Arc<Mutex<Vec<String>>>,
    tee: JoinHandle<io::Result<()>>,
}

/// Runs `abyme-mcp` with `arguments` in `directory`.
fn run(directory: &Path, arguments: &[&str]) -> io::Result<Output> {
    Command::new(BINARY)
        .args(arguments)
        .current_dir(directory)
        .stdin(Stdio::null())
        .output()
}

/// A temporary directory made an Abyme project by `abyme-mcp init`.
fn project() -> TestResult<tempfile::TempDir> {
    let directory = tempfile::tempdir()?;
    let output = run(directory.path(), &["init"])?;
    if !output.status.success() {
        return Err(format!("init failed: {output:?}").into());
    }
    Ok(directory)
}

/// Every file below `root` with its content.
fn files(root: &Path) -> io::Result<BTreeMap<PathBuf, Vec<u8>>> {
    let mut found = BTreeMap::new();
    let mut pending = vec![root.to_path_buf()];
    while let Some(directory) = pending.pop() {
        for entry in fs::read_dir(&directory)? {
            let path = entry?.path();
            if path.is_dir() {
                pending.push(path);
            } else {
                found.insert(path.clone(), fs::read(&path)?);
            }
        }
    }
    Ok(found)
}

/// Starts the server in `directory` and connects a client to it. A task
/// passes each line of the server's standard output to the client and
/// records it.
async fn start(directory: &Path) -> TestResult<Session> {
    let mut child = tokio::process::Command::new(BINARY)
        .current_dir(directory)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .kill_on_drop(true)
        .spawn()?;
    let stdin = child.stdin.take().ok_or("no stdin")?;
    let stdout = child.stdout.take().ok_or("no stdout")?;
    let (client_side, mut tee_side) = tokio::io::duplex(1 << 20);
    let stdout_lines = Arc::new(Mutex::new(Vec::new()));
    let recorded = Arc::clone(&stdout_lines);
    let tee = tokio::spawn(async move {
        let mut lines = BufReader::new(stdout).lines();
        while let Some(line) = lines.next_line().await? {
            tee_side.write_all(format!("{line}\n").as_bytes()).await?;
            recorded
                .lock()
                .map_err(|_| io::Error::other("poisoned"))?
                .push(line);
        }
        Ok(())
    });
    let client = tokio::time::timeout(TIMEOUT, ().serve((client_side, stdin))).await??;
    Ok(Session {
        client,
        child,
        stdout_lines,
        tee,
    })
}

/// Closes the session and returns every line the server wrote to standard
/// output.
async fn finish(session: Session) -> TestResult<Vec<String>> {
    let Session {
        client,
        mut child,
        stdout_lines,
        tee,
    } = session;
    tokio::time::timeout(TIMEOUT, client.cancel()).await??;
    let status = tokio::time::timeout(TIMEOUT, child.wait()).await??;
    if !status.success() {
        return Err(format!("the server exited with {status}").into());
    }
    tokio::time::timeout(TIMEOUT, tee).await???;
    let lines = stdout_lines.lock().map_err(|_| "poisoned")?.clone();
    Ok(lines)
}

/// Calls a tool with `arguments`, which must be a JSON object.
async fn call(
    session: &Session,
    tool: &'static str,
    arguments: Value,
) -> TestResult<Result<CallToolResult, ServiceError>> {
    let Value::Object(arguments) = arguments else {
        return Err("arguments must be an object".into());
    };
    let request = CallToolRequestParams::new(tool).with_arguments(arguments);
    Ok(tokio::time::timeout(TIMEOUT, session.client.call_tool(request)).await?)
}

/// The text of a tool result.
fn text(result: &CallToolResult) -> String {
    result
        .content
        .iter()
        .filter_map(|content| content.as_text())
        .map(|content| content.text.as_str())
        .collect()
}

/// A request that creates the board `container-level` and places a new
/// element on it.
fn create_board_with_element() -> Value {
    json!({"operations": [
        {"op": "create_board", "key": "b", "title": "Container Level"},
        {"op": "create_element", "key": "pay", "type_ref": "c4/container", "name": "Payment Service",
         "summary": "Payments through Stripe"},
        {"op": "place_element", "element": {"new": "pay"}, "board": {"new": "b"}}
    ]})
}

/// Checks that every line is a JSON-RPC 2.0 message.
fn assert_only_protocol_messages(lines: &[String]) {
    assert_ne!(lines, &[] as &[String]);
    for line in lines {
        let message: Option<Value> = serde_json::from_str(line).ok();
        assert_eq!(
            message.as_ref().and_then(|message| message.get("jsonrpc")),
            Some(&json!("2.0")),
            "not a protocol message: {line}"
        );
    }
}

#[test]
fn init_creates_a_project_and_names_the_one_a_directory_lies_in() -> TestResult {
    let directory = tempfile::tempdir()?;

    let created = run(directory.path(), &["init"])?;

    assert!(created.status.success(), "{created:?}");
    let root = fs::canonicalize(directory.path())?;
    let stdout = String::from_utf8(created.stdout)?;
    assert!(
        stdout.starts_with("Created an Abyme project in "),
        "{stdout}"
    );
    assert!(stdout.contains(&*root.to_string_lossy()), "{stdout}");
    assert_eq!(
        fs::read_to_string(directory.path().join("abyme/abyme.toml"))?,
        "format = 1\n"
    );

    let subdirectory = directory.path().join("src");
    fs::create_dir(&subdirectory)?;
    let before = files(directory.path())?;
    let again = run(&subdirectory, &["init"])?;

    assert!(again.status.success(), "{again:?}");
    let stdout = String::from_utf8(again.stdout)?;
    assert!(stdout.contains("nothing changed"), "{stdout}");
    assert!(stdout.contains(&*root.to_string_lossy()), "{stdout}");
    assert_eq!(files(directory.path())?, before);
    Ok(())
}

#[test]
fn refuses_any_other_command_line_without_writing() -> TestResult {
    for arguments in [&["init", "extra"][..], &["--foo"], &["--help"]] {
        let directory = tempfile::tempdir()?;

        let output = run(directory.path(), arguments)?;

        assert_eq!(output.status.code(), Some(2), "{arguments:?}");
        assert!(output.stdout.is_empty(), "{arguments:?}");
        assert!(String::from_utf8(output.stderr)?.starts_with("usage: abyme-mcp"));
        assert!(
            fs::read_dir(directory.path())?.next().is_none(),
            "{arguments:?}"
        );
    }
    Ok(())
}

#[tokio::test]
async fn applies_operations_and_reads_the_board_back() -> TestResult {
    let directory = project()?;
    let session = start(directory.path()).await?;

    let tools = tokio::time::timeout(TIMEOUT, session.client.list_all_tools()).await??;
    let mut names: Vec<&str> = tools.iter().map(|tool| tool.name.as_ref()).collect();
    names.sort_unstable();
    assert_eq!(names, ["apply", "read_board"]);

    let applied = call(&session, "apply", create_board_with_element()).await??;
    assert_eq!(applied.is_error, Some(false), "{applied:?}");
    assert_eq!(
        text(&applied),
        "b: board container-level\npay: element payment-service\n"
    );

    let read = call(&session, "read_board", json!({"board": "container-level"})).await??;
    assert_eq!(read.is_error, Some(false), "{read:?}");
    assert_eq!(
        text(&read),
        "Board: container-level · Container Level\n\
         \n\
         payment-service  c4/container  Payment Service  Payments through Stripe\n"
    );
    assert!(
        directory
            .path()
            .join("abyme/elements/payment-service.md")
            .is_file()
    );
    assert!(
        directory
            .path()
            .join("abyme/boards/container-level.board.json")
            .is_file()
    );

    assert_only_protocol_messages(&finish(session).await?);
    Ok(())
}

#[tokio::test]
async fn answers_arguments_that_do_not_fit_with_tool_errors_and_keeps_serving() -> TestResult {
    let directory = project()?;
    let session = start(directory.path()).await?;
    let before = files(directory.path())?;
    let not_fitting = [
        ("apply", json!({})),
        ("apply", json!({"operations": [], "extra": 1})),
        (
            "apply",
            json!({"operations": [{"op": "create_board", "key": "k", "title": "t", "zone": {}}]}),
        ),
        ("apply", json!({"operations": [["create_board", "k", "t"]]})),
        ("apply", json!({"operations": "create_board"})),
        ("read_board", json!({"board": "../x"})),
        ("read_board", json!({"board": "A"})),
        ("read_board", json!({"board": ""})),
        ("read_board", json!({})),
    ];

    for (tool, arguments) in not_fitting {
        let result = call(&session, tool, arguments.clone()).await??;

        assert_eq!(
            result.is_error,
            Some(true),
            "{tool} {arguments}: {result:?}"
        );
        assert!(
            text(&result).starts_with("invalid arguments: "),
            "{result:?}"
        );
    }

    let unknown_board = call(&session, "read_board", json!({"board": "nowhere"})).await??;
    assert_eq!(unknown_board.is_error, Some(true));
    assert_eq!(text(&unknown_board), "no board has the ID nowhere");
    assert_eq!(files(directory.path())?, before);

    let applied = call(&session, "apply", create_board_with_element()).await??;
    assert_eq!(applied.is_error, Some(false), "{applied:?}");

    assert_only_protocol_messages(&finish(session).await?);
    Ok(())
}

#[tokio::test]
async fn escapes_and_shortens_what_it_quotes_from_rejected_arguments() -> TestResult {
    let directory = project()?;
    let session = start(directory.path()).await?;

    let hostile = call(
        &session,
        "apply",
        json!({"operations": [], "\u{1b}[31m\u{202e}x": 1}),
    )
    .await??;
    let long = call(
        &session,
        "apply",
        json!({"operations": [], "x".repeat(10_000): 1}),
    )
    .await??;

    let hostile = text(&hostile);
    assert!(hostile.contains("\\u{1b}[31m\\u{202e}x"), "{hostile}");
    assert!(
        !hostile.contains('\u{1b}') && !hostile.contains('\u{202e}'),
        "{hostile}"
    );
    let long = text(&long);
    assert!(long.contains('…'), "{long}");
    assert!(
        long.chars().count() < 400,
        "{} characters",
        long.chars().count()
    );

    assert_only_protocol_messages(&finish(session).await?);
    Ok(())
}

#[tokio::test]
async fn a_rejected_apply_is_a_tool_error_and_changes_no_file() -> TestResult {
    let directory = project()?;
    let session = start(directory.path()).await?;
    call(&session, "apply", create_board_with_element()).await??;
    let before = files(directory.path())?;

    let rejected = call(
        &session,
        "apply",
        json!({"operations": [
            {"op": "create_element", "key": "e", "type_ref": "t", "name": "New"},
            {"op": "place_element", "element": {"new": "e"}, "board": {"existing": "nope"}}
        ]}),
    )
    .await??;

    assert_eq!(rejected.is_error, Some(true), "{rejected:?}");
    assert_eq!(text(&rejected), "operation 2: no board has the ID nope");
    assert_eq!(files(directory.path())?, before);

    assert_only_protocol_messages(&finish(session).await?);
    Ok(())
}

#[tokio::test]
async fn an_unknown_tool_is_a_protocol_error_and_the_server_keeps_serving() -> TestResult {
    let directory = project()?;
    let session = start(directory.path()).await?;

    let unknown = call(&session, "delete_everything", json!({})).await?;

    match unknown {
        Err(ServiceError::McpError(error)) => assert_eq!(error.code, ErrorCode::INVALID_PARAMS),
        other => panic!("expected a protocol error, got {other:?}"),
    }
    let applied = call(&session, "apply", create_board_with_element()).await??;
    assert_eq!(applied.is_error, Some(false), "{applied:?}");

    assert_only_protocol_messages(&finish(session).await?);
    Ok(())
}

#[tokio::test]
async fn without_a_project_both_tools_ask_for_init_and_write_nothing() -> TestResult {
    let directory = tempfile::tempdir()?;
    let session = start(directory.path()).await?;

    let applied = call(&session, "apply", create_board_with_element()).await??;
    let read = call(&session, "read_board", json!({"board": "main"})).await??;

    for result in [applied, read] {
        assert_eq!(result.is_error, Some(true), "{result:?}");
        assert!(text(&result).contains("abyme-mcp init"), "{result:?}");
    }
    assert_only_protocol_messages(&finish(session).await?);
    assert!(fs::read_dir(directory.path())?.next().is_none());
    Ok(())
}

#[tokio::test]
async fn applies_two_requests_at_once_without_losing_either() -> TestResult {
    let directory = project()?;
    let session = start(directory.path()).await?;
    call(&session, "apply", create_board_with_element()).await??;
    let place = |name: &str| {
        json!({"operations": [
            {"op": "create_element", "key": "e", "type_ref": "t", "name": name},
            {"op": "place_element", "element": {"new": "e"}, "board": {"existing": "container-level"}}
        ]})
    };

    let (left, right) = tokio::join!(
        call(&session, "apply", place("Left")),
        call(&session, "apply", place("Right"))
    );

    for result in [left??, right??] {
        assert_eq!(result.is_error, Some(false), "{result:?}");
    }
    let read = call(&session, "read_board", json!({"board": "container-level"})).await??;
    let outline = text(&read);
    assert!(outline.contains("\nleft  "), "{outline}");
    assert!(outline.contains("\nright  "), "{outline}");
    assert!(outline.contains("\npayment-service  "), "{outline}");

    assert_only_protocol_messages(&finish(session).await?);
    Ok(())
}

/// Not a parameter of any tool: the client cannot steer where the server
/// looks for the project.
#[tokio::test]
async fn no_tool_takes_a_path() -> TestResult {
    let directory = project()?;
    let session = start(directory.path()).await?;

    let tools = tokio::time::timeout(TIMEOUT, session.client.list_all_tools()).await??;

    for tool in &tools {
        let properties: Vec<&String> = tool
            .input_schema
            .get("properties")
            .and_then(Value::as_object)
            .map(JsonObject::keys)
            .into_iter()
            .flatten()
            .collect();
        let expected: &[&str] = if tool.name == "apply" {
            &["operations"]
        } else {
            &["board"]
        };
        assert_eq!(properties, expected, "{}", tool.name);
        assert_eq!(
            tool.input_schema.get("additionalProperties"),
            Some(&json!(false)),
            "{}",
            tool.name
        );
    }
    assert_only_protocol_messages(&finish(session).await?);
    Ok(())
}
