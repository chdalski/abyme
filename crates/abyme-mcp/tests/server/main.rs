//! The built `abyme-mcp` binary: `init` on the command line, and the MCP
//! server on standard input and output, driven by an MCP client.
//!
//! The server runs without `HOME`, `XDG_CACHE_HOME`, and proxy settings, so
//! no test touches the user's cache or the network unless it sets them; only
//! the ignored tests download their models.

mod boards;
mod command_line;
mod elements;
mod environment;
mod notes;
mod protocol;
mod review;
mod search;

use std::collections::BTreeMap;
use std::error::Error;
use std::ffi::OsStr;
use std::fs;
use std::io;
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use abyme_test_support::{commit_all, git};
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

/// The environment variables that would lead the server to the user's cache
/// or through a proxy; ureq reads the proxy variables in both cases.
const REMOVED_VARIABLES: [&str; 10] = [
    "HOME",
    "XDG_CACHE_HOME",
    "ALL_PROXY",
    "all_proxy",
    "HTTPS_PROXY",
    "https_proxy",
    "HTTP_PROXY",
    "http_proxy",
    "NO_PROXY",
    "no_proxy",
];

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
    let mut command = Command::new(BINARY);
    for variable in REMOVED_VARIABLES {
        command.env_remove(variable);
    }
    command
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
    start_with(directory, &[]).await
}

/// Starts the server like [`start`], with the environment variables in
/// `variables` set.
async fn start_with(directory: &Path, variables: &[(&str, &OsStr)]) -> TestResult<Session> {
    let mut command = tokio::process::Command::new(BINARY);
    for variable in REMOVED_VARIABLES {
        command.env_remove(variable);
    }
    command.envs(variables.iter().copied());
    let mut child = command
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
    call_within(session, tool, arguments, TIMEOUT).await
}

/// Calls a tool like [`call`], giving up after `timeout`.
async fn call_within(
    session: &Session,
    tool: &'static str,
    arguments: Value,
    timeout: Duration,
) -> TestResult<Result<CallToolResult, ServiceError>> {
    let Value::Object(arguments) = arguments else {
        return Err("arguments must be an object".into());
    };
    let request = CallToolRequestParams::new(tool).with_arguments(arguments);
    Ok(tokio::time::timeout(timeout, session.client.call_tool(request)).await?)
}

/// A temporary Abyme project with a session started in it.
async fn started_project() -> TestResult<(tempfile::TempDir, Session)> {
    let directory = project()?;
    let session = start(directory.path()).await?;
    Ok((directory, session))
}

/// Closes the session and checks that the server wrote only protocol
/// messages to standard output.
async fn closed(session: Session) -> TestResult {
    assert_only_protocol_messages(&finish(session).await?);
    Ok(())
}

/// Calls a tool that must succeed, and returns the text of its result.
async fn call_ok(session: &Session, tool: &'static str, arguments: Value) -> TestResult<String> {
    let result = call(session, tool, arguments).await??;
    assert_eq!(result.is_error, Some(false), "{result:?}");
    Ok(text(&result))
}

/// Calls a tool that must refuse, and returns the text of its refusal.
async fn call_refused(
    session: &Session,
    tool: &'static str,
    arguments: Value,
) -> TestResult<String> {
    let result = call(session, tool, arguments).await??;
    assert_eq!(result.is_error, Some(true), "{result:?}");
    Ok(text(&result))
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

/// A project at the top of a Git repository whose element `x` is anchored
/// on `crates/x` and stamped with the commit that added `crates/x/lib.rs`,
/// which has changed since.
fn repository_with_a_changed_anchor() -> TestResult<tempfile::TempDir> {
    let directory = project()?;
    git(directory.path(), &["init", "-q"])?;
    fs::create_dir_all(directory.path().join("crates/x"))?;
    fs::write(directory.path().join("crates/x/lib.rs"), "fn a() {}\n")?;
    let stamp = commit_all(directory.path())?;
    fs::create_dir_all(directory.path().join("abyme/elements"))?;
    fs::write(
        directory.path().join("abyme/elements/x.md"),
        format!(
            "---\nid: x\ntype: t\nname: X\nanchor: crates/x\n\
             verified: \"{stamp}\"\nverified_by: person\n---\n"
        ),
    )?;
    fs::write(directory.path().join("crates/x/lib.rs"), "fn a() { b() }\n")?;
    Ok(directory)
}

/// What `read_element` shows of the element `x` between `Verified:` and
/// `Boards:` when `crates/x/lib.rs` changed.
const CHANGED_ANCHOR_REVIEW: &str =
    "\nReview: task\n  changed since the stamp: crates/x/lib.rs\nBoards: none\n";

/// Writes the element `id` with the given anchor and stamp into the
/// project at `directory`.
fn write_element(
    directory: &Path,
    id: &str,
    anchor: Option<&str>,
    stamp: Option<&str>,
) -> TestResult {
    let anchor = anchor.map_or(String::new(), |anchor| format!("anchor: {anchor}\n"));
    let stamp = stamp.map_or(String::new(), |stamp| {
        format!("verified: \"{stamp}\"\nverified_by: person\n")
    });
    fs::create_dir_all(directory.join("abyme/elements"))?;
    fs::write(
        directory.join(format!("abyme/elements/{id}.md")),
        format!("---\nid: {id}\ntype: t\nname: N\n{anchor}{stamp}---\n"),
    )?;
    Ok(())
}

/// A project in a Git repository with the elements `changed`, anchored on
/// `crates/c`, whose file `leak-canary.txt` changed since its stamp,
/// `hinted`, which uses `changed`, `clean`, anchored on `crates/k` and
/// unchanged, and `calm`, without flags; the board `b` places them and the
/// ID `ghost`, which has no file.
fn repository_with_marked_elements() -> TestResult<tempfile::TempDir> {
    let directory = project()?;
    let root = directory.path();
    git(root, &["init", "-q"])?;
    for file in ["crates/c/leak-canary.txt", "crates/k/lib.rs"] {
        fs::create_dir_all(root.join(file).parent().ok_or("no parent")?)?;
        fs::write(root.join(file), "a\n")?;
    }
    let stamp = commit_all(root)?;
    write_element(root, "changed", Some("crates/c"), Some(&stamp))?;
    write_element(root, "clean", Some("crates/k"), Some(&stamp))?;
    write_element(root, "calm", None, None)?;
    fs::write(
        root.join("abyme/elements/hinted.md"),
        format!(
            "---\nid: hinted\ntype: t\nname: N\nverified: \"{stamp}\"\nverified_by: ai\n\
             relations:\n  - {{kind: uses, to: changed}}\n---\n"
        ),
    )?;
    let placements: Vec<String> = ["calm", "changed", "clean", "ghost", "hinted"]
        .iter()
        .enumerate()
        .map(|(row, id)| {
            format!(
                "{{\"element\": \"{id}\", \"x\": 0, \"y\": {}, \"width\": 180, \"height\": 90}}",
                row * 100
            )
        })
        .collect();
    fs::create_dir_all(root.join("abyme/boards"))?;
    fs::write(
        root.join("abyme/boards/b.board.json"),
        format!(
            "{{\"format\": 1, \"id\": \"b\", \"title\": \"B\", \"placements\": [{}]}}\n",
            placements.join(", ")
        ),
    )?;
    fs::write(root.join("crates/c/leak-canary.txt"), "b\n")?;
    Ok(directory)
}
