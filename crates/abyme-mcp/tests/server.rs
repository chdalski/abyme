//! The built `abyme-mcp` binary: `init` on the command line, and the MCP
//! server on standard input and output, driven by an MCP client.
//!
//! The server runs without `HOME`, `XDG_CACHE_HOME`, and proxy settings, so
//! no test touches the user's cache or the network unless it sets them; only
//! the ignored tests download their models.

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

/// How long the server may take to exit after its standard input closed.
const EXIT_TIMEOUT: Duration = Duration::from_secs(10);

const BINARY: &str = env!("CARGO_BIN_EXE_abyme-mcp");

/// The line that starts a reply of a read tool in a project outside Git.
const NO_REPOSITORY: &str = "Review flags unavailable: the project lies in no Git repository\n";

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

/// A proxy URL for `listener`, which accepts connections and never answers.
fn proxy_url(listener: &TcpListener) -> io::Result<String> {
    Ok(format!("http://{}", listener.local_addr()?))
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
    assert_eq!(
        names,
        [
            "apply",
            "overview",
            "read_board",
            "read_element",
            "search",
            "status",
            "verify"
        ]
    );

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
        "Review flags unavailable: the project lies in no Git repository\n\
         Board: container-level · Container Level\n\
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
            json!({"operations": [{"op": "create_board", "key": "k", "title": "t", "unexpected": {}}]}),
        ),
        ("apply", json!({"operations": [["create_board", "k", "t"]]})),
        ("apply", json!({"operations": "create_board"})),
        ("read_board", json!({"board": "../x"})),
        ("read_board", json!({"board": "A"})),
        ("read_board", json!({"board": ""})),
        ("read_board", json!({})),
        ("read_element", json!({"element": "../x"})),
        ("read_element", json!({"element": "a/b"})),
        ("read_element", json!({"element": "X"})),
        ("read_element", json!({"element": ""})),
        ("read_element", json!({"element": "a".repeat(65)})),
        ("read_element", json!({"element": 1})),
        ("read_element", json!({"element": "x", "path": "/etc"})),
        ("read_element", json!({})),
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
async fn apply_builds_an_element_s_inner_board_and_refuses_a_board_inside_itself() -> TestResult {
    let directory = project()?;
    let session = start(directory.path()).await?;

    let applied = call(
        &session,
        "apply",
        json!({"operations": [
            {"op": "create_board", "key": "ctx", "title": "Context"},
            {"op": "create_element", "key": "shop", "type_ref": "c4/system", "name": "Shop"},
            {"op": "place_element", "element": {"new": "shop"}, "board": {"new": "ctx"}},
            {"op": "create_board", "key": "inside", "title": "Shop Containers",
             "owner": {"new": "shop"}, "summary": "What the shop runs"},
            {"op": "create_element", "key": "pay", "type_ref": "c4/container", "name": "Payments"},
            {"op": "place_element", "element": {"new": "pay"}, "board": {"new": "inside"}}
        ]}),
    )
    .await??;
    assert_eq!(applied.is_error, Some(false), "{applied:?}");
    let before = files(directory.path())?;
    let rejected = call(
        &session,
        "apply",
        json!({"operations": [
            {"op": "update_board", "board": {"existing": "context"}, "owner": {"existing": "payments"}}
        ]}),
    )
    .await??;

    assert_eq!(
        fs::read_to_string(
            directory
                .path()
                .join("abyme/boards/shop-containers.board.json")
        )?,
        "{\n  \"format\": 1,\n  \"id\": \"shop-containers\",\n  \"title\": \"Shop Containers\",\n  \
         \"owner\": \"shop\",\n  \"summary\": \"What the shop runs\",\n  \"placements\": [\n    \
         {\"element\": \"payments\", \"x\": 0, \"y\": 0, \"width\": 180, \"height\": 90}\n  ]\n}\n"
    );
    assert!(
        !fs::read_to_string(directory.path().join("abyme/boards/context.board.json"))?
            .contains("owner")
    );
    assert_eq!(rejected.is_error, Some(true), "{rejected:?}");
    assert_eq!(
        text(&rejected),
        "operation 1: board context would lie inside itself"
    );
    assert_eq!(files(directory.path())?, before);
    assert_only_protocol_messages(&finish(session).await?);
    Ok(())
}

#[tokio::test]
async fn apply_sets_changes_and_removes_a_board_s_zone_and_refuses_another_one() -> TestResult {
    let directory = project()?;
    let session = start(directory.path()).await?;
    let file = directory.path().join("abyme/boards/ideas.board.json");
    let board_file = |zone: &str| {
        format!(
            "{{\n  \"format\": 1,\n  \"id\": \"ideas\",\n  \"title\": \"Ideas\",\n{zone}  \
             \"placements\": []\n}}\n"
        )
    };
    let mut written = Vec::new();

    for operation in [
        json!({"op": "create_board", "key": "b", "title": "Ideas",
               "zone": {"mode": "free", "statement": "idea"}}),
        json!({"op": "update_board", "board": {"existing": "ideas"},
               "zone": {"mode": "mirrored", "statement": "current"}}),
        json!({"op": "update_board", "board": {"existing": "ideas"}, "zone": null}),
    ] {
        let applied = call(&session, "apply", json!({"operations": [operation]})).await??;
        assert_eq!(applied.is_error, Some(false), "{applied:?}");
        written.push(fs::read_to_string(&file)?);
    }
    let before = files(directory.path())?;
    let refused = call(
        &session,
        "apply",
        json!({"operations": [{"op": "update_board", "board": {"existing": "ideas"},
               "zone": {"mode": {"mirrored": null}, "statement": "historic\n\u{1b}[31m"}}]}),
    )
    .await??;

    assert_eq!(
        written,
        [
            board_file("  \"zone\": {\"mode\": \"free\", \"statement\": \"idea\"},\n"),
            board_file("  \"zone\": {\"mode\": \"mirrored\", \"statement\": \"current\"},\n"),
            board_file(""),
        ]
    );
    assert_eq!(refused.is_error, Some(true), "{refused:?}");
    assert!(
        text(&refused).starts_with("invalid arguments: "),
        "{refused:?}"
    );
    assert!(!text(&refused).chars().any(char::is_control), "{refused:?}");
    assert_eq!(files(directory.path())?, before);
    assert_only_protocol_messages(&finish(session).await?);
    Ok(())
}

#[tokio::test]
async fn the_read_tools_show_each_element_s_kind_from_its_home_board_s_zone() -> TestResult {
    let directory = project()?;
    let session = start(directory.path()).await?;
    let applied = call(
        &session,
        "apply",
        json!({"operations": [
            {"op": "create_board", "key": "ctx", "title": "Context",
             "zone": {"mode": "mirrored", "statement": "current"}},
            {"op": "create_element", "key": "shop", "type_ref": "c4/system", "name": "Shop"},
            {"op": "place_element", "element": {"new": "shop"}, "board": {"new": "ctx"}},
            {"op": "create_board", "key": "z", "title": "Zeta",
             "zone": {"mode": "free", "statement": "idea"}},
            {"op": "create_element", "key": "spark", "type_ref": "note", "name": "Spark"},
            {"op": "place_element", "element": {"new": "spark"}, "board": {"new": "z"}}
        ]}),
    )
    .await??;
    assert_eq!(applied.is_error, Some(false), "{applied:?}");
    // A hand edit places the shop on the idea board too; its home stays the
    // context, the board with the lower ID.
    fs::write(
        directory.path().join("abyme/boards/zeta.board.json"),
        r#"{"format": 1, "id": "zeta", "title": "Zeta",
            "zone": {"mode": "free", "statement": "idea"}, "placements": [
            {"element": "shop", "x": 0, "y": 0, "width": 180, "height": 90},
            {"element": "spark", "x": 0, "y": 100, "width": 180, "height": 90}]}"#,
    )?;
    let unavailable = "Review flags unavailable: the project lies in no Git repository\n";

    let board = call(&session, "read_board", json!({"board": "zeta"})).await??;
    let overview = call(&session, "overview", json!({})).await??;
    let shop = call(&session, "read_element", json!({"element": "shop"})).await??;

    assert_eq!(
        text(&board),
        format!(
            "{unavailable}Board: zeta · Zeta\n\
             Zone: free · idea\n\
             \n\
             shop   c4/system  current  Shop\n\
             spark  note       idea     Spark\n"
        )
    );
    assert_eq!(
        text(&overview),
        format!(
            "{unavailable}context  Context  mirrored · current    1 element\n\
             zeta     Zeta     free · idea           1 element\n"
        )
    );
    assert!(
        text(&shop).contains("\nType: c4/system\nStatement kind: current, from board context\n"),
        "{}",
        text(&shop)
    );
    assert!(
        text(&shop).contains("\nBoards: context, zeta\n"),
        "{}",
        text(&shop)
    );
    assert_only_protocol_messages(&finish(session).await?);
    Ok(())
}

#[tokio::test]
async fn read_board_and_read_element_show_inner_boards_also_from_hand_edited_files() -> TestResult {
    let directory = project()?;
    let session = start(directory.path()).await?;
    let applied = call(
        &session,
        "apply",
        json!({"operations": [
            {"op": "create_board", "key": "ctx", "title": "Context"},
            {"op": "create_element", "key": "shop", "type_ref": "c4/system", "name": "Shop"},
            {"op": "place_element", "element": {"new": "shop"}, "board": {"new": "ctx"}},
            {"op": "create_board", "key": "inside", "title": "Shop Containers",
             "owner": {"new": "shop"}, "summary": "Runs\nInside of: forged \u{202e}x"},
            {"op": "create_element", "key": "pay", "type_ref": "c4/container", "name": "Payments"},
            {"op": "place_element", "element": {"new": "pay"}, "board": {"new": "inside"}}
        ]}),
    )
    .await??;
    assert_eq!(applied.is_error, Some(false), "{applied:?}");
    let boards = directory.path().join("abyme/boards");
    // A hand edit closes a cycle: the context is the inside of payments,
    // which lies inside the shop on the context.
    fs::write(
        boards.join("context.board.json"),
        r#"{"format": 1, "id": "context", "title": "Context", "owner": "payments",
            "placements": [{"element": "shop", "x": 0, "y": 0, "width": 180, "height": 90}]}"#,
    )?;
    fs::write(
        boards.join("orphan.board.json"),
        r#"{"format": 1, "id": "orphan", "title": "Orphan", "owner": "gone", "placements": []}"#,
    )?;
    let unavailable = "Review flags unavailable: the project lies in no Git repository\n";

    let context = call(&session, "read_board", json!({"board": "context"})).await??;
    let inside = call(&session, "read_board", json!({"board": "shop-containers"})).await??;
    let orphan = call(&session, "read_board", json!({"board": "orphan"})).await??;
    let payments = call(&session, "read_element", json!({"element": "payments"})).await??;

    assert_eq!(
        text(&context),
        format!(
            "{unavailable}Board: context · Context\n\
             Inside of: payments · Above: shop-containers ⚠ cycle\n\
             \n\
             shop  c4/system  Shop    ▸ shop-containers\n"
        )
    );
    assert_eq!(
        text(&inside),
        format!(
            "{unavailable}Board: shop-containers · Shop Containers\n\
             Inside of: shop · Above: context ⚠ cycle\n\
             Summary: Runs\\nInside of: forged \\u{{202e}}x\n\
             \n\
             payments  c4/container  Payments    ▸ context\n"
        )
    );
    assert_eq!(
        text(&orphan),
        format!("{unavailable}Board: orphan · Orphan\nInside of: gone ⚠ missing\n\n")
    );
    assert_eq!(payments.is_error, Some(false), "{payments:?}");
    assert!(
        text(&payments).contains("\nBoards: shop-containers\nInner board: context\n"),
        "{}",
        text(&payments)
    );
    assert_only_protocol_messages(&finish(session).await?);
    Ok(())
}

#[tokio::test]
async fn apply_creates_and_deletes_relations_in_the_element_files() -> TestResult {
    let directory = project()?;
    let session = start(directory.path()).await?;
    call(&session, "apply", create_board_with_element()).await??;
    let element_file = directory.path().join("abyme/elements/payment-service.md");

    let created = call(
        &session,
        "apply",
        json!({"operations": [
            {"op": "create_element", "key": "db", "type_ref": "c4/datastore", "name": "Orders DB"},
            {"op": "create_relation", "from": {"existing": "payment-service"}, "kind": "uses",
             "to": {"new": "db"}, "label": "SQL"},
            {"op": "create_relation", "from": {"existing": "payment-service"}, "kind": "depends_on",
             "to": {"new": "db"}}
        ]}),
    )
    .await??;
    assert_eq!(created.is_error, Some(false), "{created:?}");
    assert_eq!(
        fs::read_to_string(&element_file)?,
        "---\n\
         id: \"payment-service\"\n\
         type: \"c4/container\"\n\
         name: \"Payment Service\"\n\
         summary: \"Payments through Stripe\"\n\
         relations:\n  \
         - {kind: \"depends_on\", to: \"orders-db\"}\n  \
         - {kind: \"uses\", to: \"orders-db\", label: \"SQL\"}\n\
         ---\n"
    );

    let deleted = call(
        &session,
        "apply",
        json!({"operations": [
            {"op": "delete_relation", "from": {"existing": "payment-service"}, "kind": "depends_on",
             "to": {"existing": "orders-db"}}
        ]}),
    )
    .await??;
    assert_eq!(deleted.is_error, Some(false), "{deleted:?}");
    let text_after = fs::read_to_string(&element_file)?;
    assert!(!text_after.contains("depends_on"), "{text_after}");
    assert!(text_after.contains("{kind: \"uses\", to: \"orders-db\", label: \"SQL\"}"));

    assert_only_protocol_messages(&finish(session).await?);
    Ok(())
}

#[tokio::test]
async fn read_board_shows_relations_with_their_labels_escaped_onto_one_line() -> TestResult {
    let directory = project()?;
    let session = start(directory.path()).await?;
    call(&session, "apply", create_board_with_element()).await??;
    let applied = call(
        &session,
        "apply",
        json!({"operations": [
            {"op": "create_element", "key": "db", "type_ref": "c4/datastore", "name": "Orders DB"},
            {"op": "place_element", "element": {"new": "db"}, "board": {"existing": "container-level"}},
            {"op": "create_relation", "from": {"existing": "payment-service"}, "kind": "uses",
             "to": {"new": "db"}, "label": "SQL\nBoard: forged \u{202e}x"}
        ]}),
    )
    .await??;
    assert_eq!(applied.is_error, Some(false), "{applied:?}");

    let read = call(&session, "read_board", json!({"board": "container-level"})).await??;

    assert_eq!(read.is_error, Some(false), "{read:?}");
    assert_eq!(
        text(&read),
        "Review flags unavailable: the project lies in no Git repository\n\
         Board: container-level · Container Level\n\
         \n\
         orders-db        c4/datastore  Orders DB\n\
         payment-service  c4/container  Payment Service  Payments through Stripe\n\
         \n\
         payment-service → orders-db  uses  SQL\\nBoard: forged \\u{202e}x\n"
    );

    assert_only_protocol_messages(&finish(session).await?);
    Ok(())
}

#[tokio::test]
async fn read_element_shows_an_element_with_its_relations_and_description() -> TestResult {
    let directory = project()?;
    let session = start(directory.path()).await?;
    let applied = call(
        &session,
        "apply",
        json!({"operations": [
            {"op": "create_board", "key": "b", "title": "Container Level"},
            {"op": "create_element", "key": "pay", "type_ref": "c4/container", "name": "Payment Service",
             "summary": "Payments through Stripe", "body": "Handles payments.\nRefunds\u{202e} run later.\n"},
            {"op": "create_element", "key": "db", "type_ref": "c4/datastore", "name": "Orders DB"},
            {"op": "create_element", "key": "web", "type_ref": "c4/container", "name": "Web Shop"},
            {"op": "place_element", "element": {"new": "pay"}, "board": {"new": "b"}},
            {"op": "create_relation", "from": {"new": "pay"}, "kind": "uses", "to": {"new": "db"},
             "label": "SQL"},
            {"op": "create_relation", "from": {"new": "web"}, "kind": "uses", "to": {"new": "pay"},
             "label": "REST"}
        ]}),
    )
    .await??;
    assert_eq!(applied.is_error, Some(false), "{applied:?}");

    let read = call(
        &session,
        "read_element",
        json!({"element": "payment-service"}),
    )
    .await??;

    assert_eq!(read.is_error, Some(false), "{read:?}");
    assert_eq!(
        text(&read),
        "Review flags unavailable: the project lies in no Git repository\n\
         Element: payment-service · Payment Service\n\
         Type: c4/container\n\
         Statement kind: none\n\
         Anchor: none\n\
         Verified: none\n\
         Summary: Payments through Stripe\n\
         Boards: container-level\n\
         Inner board: none\n\
         \n\
         Outgoing relations:\n\
         payment-service → orders-db  uses  SQL\n\
         \n\
         Incoming relations:\n\
         web-shop → payment-service  uses  REST\n\
         \n\
         Description:\n\
         Handles payments.\n\
         Refunds\\u{202e} run later.\n"
    );

    assert_only_protocol_messages(&finish(session).await?);
    Ok(())
}

#[tokio::test]
async fn apply_updates_an_element_and_read_element_shows_it() -> TestResult {
    let directory = project()?;
    let session = start(directory.path()).await?;
    call(&session, "apply", create_board_with_element()).await??;

    let updated = call(
        &session,
        "apply",
        json!({"operations": [
            {"op": "update_element", "element": {"existing": "payment-service"}, "name": "Payments",
             "summary": null, "body": "Handles payments.\n"}
        ]}),
    )
    .await??;
    assert_eq!(updated.is_error, Some(false), "{updated:?}");

    let read = call(
        &session,
        "read_element",
        json!({"element": "payment-service"}),
    )
    .await??;
    assert_eq!(
        text(&read),
        "Review flags unavailable: the project lies in no Git repository\n\
         Element: payment-service · Payments\n\
         Type: c4/container\n\
         Statement kind: none\n\
         Anchor: none\n\
         Verified: none\n\
         Boards: container-level\n\
         Inner board: none\n\
         \n\
         Outgoing relations:\n\
         none\n\
         \n\
         Incoming relations:\n\
         none\n\
         \n\
         Description:\n\
         Handles payments.\n"
    );

    assert_only_protocol_messages(&finish(session).await?);
    Ok(())
}

#[tokio::test]
async fn a_rejected_update_is_a_tool_error_and_changes_no_file() -> TestResult {
    let directory = project()?;
    let session = start(directory.path()).await?;
    call(&session, "apply", create_board_with_element()).await??;
    let before = files(directory.path())?;

    let rejected = call(
        &session,
        "apply",
        json!({"operations": [
            {"op": "update_element", "element": {"existing": "payment-service"}, "name": "Renamed"},
            {"op": "update_element", "element": {"existing": "nowhere"}, "name": "x"}
        ]}),
    )
    .await??;

    assert_eq!(rejected.is_error, Some(true), "{rejected:?}");
    assert_eq!(
        text(&rejected),
        "operation 2: no element has the ID nowhere"
    );
    assert_eq!(files(directory.path())?, before);

    assert_only_protocol_messages(&finish(session).await?);
    Ok(())
}

/// A hand-written element file with an anchor and a stamp by a person.
const STAMPED: &str = "---\nid: stamped\ntype: t\nname: Stamped\nanchor: crates/stamped\n\
    verified: \"0123456789abcdef0123456789abcdef01234567\"\nverified_by: person\n---\n";

/// A project with the hand-written element `stamped`.
fn project_with_a_stamped_element() -> TestResult<tempfile::TempDir> {
    let directory = project()?;
    let elements = directory.path().join("abyme/elements");
    fs::create_dir_all(&elements)?;
    fs::write(elements.join("stamped.md"), STAMPED)?;
    Ok(directory)
}

#[tokio::test]
async fn apply_sets_and_changes_anchors_and_a_changed_anchor_removes_the_stamp() -> TestResult {
    let directory = project_with_a_stamped_element()?;
    let session = start(directory.path()).await?;

    let created = call(
        &session,
        "apply",
        json!({"operations": [
            {"op": "create_element", "key": "d", "type_ref": "doc", "name": "Docs Page",
             "anchor": "docs/a \"b\" #c: d"}
        ]}),
    )
    .await??;
    assert_eq!(
        created.is_error,
        Some(true),
        "a colon is refused: {created:?}"
    );
    let created = call(
        &session,
        "apply",
        json!({"operations": [
            {"op": "create_element", "key": "d", "type_ref": "doc", "name": "Docs Page",
             "anchor": "docs/a \"b\" #c"}
        ]}),
    )
    .await??;
    assert_eq!(created.is_error, Some(false), "{created:?}");
    let read = call(&session, "read_element", json!({"element": "docs-page"})).await??;
    assert!(
        text(&read).starts_with(
            "Review flags unavailable: the project lies in no Git repository\n\
             Element: docs-page · Docs Page\n\
             Type: doc\n\
             Statement kind: none\n\
             Anchor: docs/a \"b\" #c\n\
             Verified: none\n"
        ),
        "{}",
        text(&read)
    );

    let read = call(&session, "read_element", json!({"element": "stamped"})).await??;
    assert!(
        text(&read).contains(
            "\nAnchor: crates/stamped\n\
             Verified: 0123456789abcdef0123456789abcdef01234567 by person\n"
        ),
        "{}",
        text(&read)
    );

    let changed = call(
        &session,
        "apply",
        json!({"operations": [
            {"op": "update_element", "element": {"existing": "stamped"}, "anchor": "crates/other"}
        ]}),
    )
    .await??;
    assert_eq!(changed.is_error, Some(false), "{changed:?}");
    let read = call(&session, "read_element", json!({"element": "stamped"})).await??;
    assert!(
        text(&read).contains("\nAnchor: crates/other\nVerified: none\n"),
        "{}",
        text(&read)
    );
    assert_eq!(
        fs::read_to_string(directory.path().join("abyme/elements/stamped.md"))?,
        "---\n\
         id: \"stamped\"\n\
         type: \"t\"\n\
         name: \"Stamped\"\n\
         anchor: \"crates/other\"\n\
         ---\n"
    );

    assert_only_protocol_messages(&finish(session).await?);
    Ok(())
}

#[tokio::test]
async fn apply_removes_the_stamp_when_the_content_of_an_element_changes() -> TestResult {
    let directory = project_with_a_stamped_element()?;
    let session = start(directory.path()).await?;

    let renamed = call(
        &session,
        "apply",
        json!({"operations": [
            {"op": "update_element", "element": {"existing": "stamped"}, "name": "Renamed"}
        ]}),
    )
    .await??;
    assert_eq!(renamed.is_error, Some(false), "{renamed:?}");

    let read = call(&session, "read_element", json!({"element": "stamped"})).await??;
    assert!(
        text(&read).contains("\nAnchor: crates/stamped\nVerified: none\n"),
        "{}",
        text(&read)
    );
    let file = fs::read_to_string(directory.path().join("abyme/elements/stamped.md"))?;
    assert!(!file.contains("verified"), "{file}");

    assert_only_protocol_messages(&finish(session).await?);
    Ok(())
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

#[tokio::test]
async fn read_element_shows_the_review_flags_git_reports() -> TestResult {
    let directory = repository_with_a_changed_anchor()?;
    let config = tempfile::tempdir()?;
    let session = start_with(
        directory.path(),
        &[("XDG_CONFIG_HOME", config.path().as_os_str())],
    )
    .await?;

    let read = call(&session, "read_element", json!({"element": "x"})).await??;

    assert_eq!(read.is_error, Some(false), "{read:?}");
    assert!(
        text(&read).starts_with("Element: x · X\n"),
        "{}",
        text(&read)
    );
    assert!(
        text(&read).contains(CHANGED_ANCHOR_REVIEW),
        "{}",
        text(&read)
    );
    assert_only_protocol_messages(&finish(session).await?);
    Ok(())
}

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

#[tokio::test]
async fn read_element_shows_each_kind_of_review_flag() -> TestResult {
    let directory = project()?;
    let root = directory.path();
    git(root, &["init", "-q"])?;
    for file in ["crates/c/lib.rs", "crates/u/lib.rs"] {
        fs::create_dir_all(root.join(file).parent().ok_or("no parent")?)?;
        fs::write(root.join(file), "fn a() {}\n")?;
    }
    let stamp = commit_all(root)?;
    let unknown = "0123456789abcdef0123456789abcdef01234567";
    write_element(root, "committed", Some("crates/c"), Some(&stamp))?;
    write_element(root, "untracked", Some("crates/u"), Some(&stamp))?;
    write_element(root, "unknown", Some("crates/c"), Some(unknown))?;
    write_element(root, "missing", Some("crates/gone"), Some(&stamp))?;
    write_element(root, "unstamped", Some("crates/c"), None)?;
    write_element(root, "native", None, None)?;
    fs::write(root.join("crates/c/lib.rs"), "fn a() { b() }\n")?;
    commit_all(root)?;
    fs::write(root.join("crates/u/new.rs"), "fn n() {}\n")?;
    let config = tempfile::tempdir()?;
    let session = start_with(root, &[("XDG_CONFIG_HOME", config.path().as_os_str())]).await?;
    let cases = [
        (
            "committed",
            "Review: task\n  changed since the stamp: crates/c/lib.rs\n",
        ),
        (
            "untracked",
            "Review: task\n  changed since the stamp: crates/u/new.rs\n",
        ),
        (
            "unknown",
            "Review: task\n  the stamp's commit is unknown to the repository or no ancestor of HEAD\n",
        ),
        (
            "missing",
            "Review: task\n  no file that Git knows lies under the anchor\n",
        ),
        ("unstamped", "Review: task\n  never verified\n"),
        ("native", "Review: none\n"),
    ];

    for (id, review) in cases {
        let read = call(&session, "read_element", json!({"element": id})).await??;

        assert_eq!(read.is_error, Some(false), "{read:?}");
        assert!(
            text(&read).contains(&format!("\n{review}Boards: none\n")),
            "{id}: {}",
            text(&read)
        );
    }
    assert_only_protocol_messages(&finish(session).await?);
    Ok(())
}

#[tokio::test]
async fn read_element_shows_flags_passed_on_along_relations() -> TestResult {
    let directory = project()?;
    let root = directory.path();
    git(root, &["init", "-q"])?;
    fs::create_dir_all(root.join("crates/db"))?;
    fs::write(root.join("crates/db/schema.sql"), "create table a;\n")?;
    let stamp = commit_all(root)?;
    write_element(root, "orders-db", Some("crates/db"), Some(&stamp))?;
    for (id, kind) in [("pay", "depends_on"), ("note", "uses")] {
        fs::write(
            root.join(format!("abyme/elements/{id}.md")),
            format!(
                "---\nid: {id}\ntype: t\nname: N\nverified: \"{stamp}\"\nverified_by: ai\n\
                 relations:\n  - {{kind: {kind}, to: orders-db, label: \"secret label\"}}\n---\n"
            ),
        )?;
    }
    fs::write(root.join("crates/db/schema.sql"), "create table b;\n")?;
    let config = tempfile::tempdir()?;
    let session = start_with(root, &[("XDG_CONFIG_HOME", config.path().as_os_str())]).await?;
    let cases = [
        (
            "pay",
            "Review: task\n  depends_on orders-db: what its anchor stands for may have changed since this element's stamp\n",
        ),
        (
            "note",
            "Review: hint\n  uses orders-db: what its anchor stands for may have changed since this element's stamp\n",
        ),
    ];

    for (id, review) in cases {
        let read = call(&session, "read_element", json!({"element": id})).await??;

        let reply = text(&read);
        assert!(
            reply.contains(&format!("\n{review}Boards: none\n")),
            "{id}: {reply}"
        );
        let line = reply
            .lines()
            .find(|line| line.contains("orders-db: what"))
            .ok_or("no passed-on flag")?;
        assert!(!line.contains("schema.sql"), "{line}");
        assert!(!line.contains("secret"), "{line}");
    }
    assert_only_protocol_messages(&finish(session).await?);
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

#[tokio::test]
async fn read_board_marks_the_elements_to_review() -> TestResult {
    let directory = repository_with_marked_elements()?;
    let config = tempfile::tempdir()?;
    let session = start_with(
        directory.path(),
        &[("XDG_CONFIG_HOME", config.path().as_os_str())],
    )
    .await?;

    let read = call(&session, "read_board", json!({"board": "b"})).await??;
    let detail = call(&session, "read_element", json!({"element": "changed"})).await??;

    assert_eq!(
        text(&read),
        "Board: b · B\n\
         \n\
         calm     t  N\n\
         changed  t  N    ⚠ task\n\
         clean    t  N\n\
         ghost    ⚠ missing\n\
         hinted   t  N    ⚠ hint\n\
         \n\
         hinted → changed  uses\n"
    );
    assert!(
        text(&detail).contains("leak-canary.txt"),
        "{}",
        text(&detail)
    );
    assert_only_protocol_messages(&finish(session).await?);
    Ok(())
}

#[tokio::test]
async fn verify_stamps_a_checked_element_and_clears_its_flag() -> TestResult {
    let directory = repository_with_a_changed_anchor()?;
    let head = commit_all(directory.path())?;
    let config = tempfile::tempdir()?;
    let session = start_with(
        directory.path(),
        &[("XDG_CONFIG_HOME", config.path().as_os_str())],
    )
    .await?;

    let verified = call(&session, "verify", json!({"element": "x"})).await??;
    let read = call(&session, "read_element", json!({"element": "x"})).await??;

    assert_eq!(verified.is_error, Some(false), "{verified:?}");
    assert_eq!(text(&verified), format!("Verified x against {head} by ai."));
    assert!(
        text(&read).contains(&format!("\nVerified: {head} by ai\nReview: none\n")),
        "{}",
        text(&read)
    );
    assert_only_protocol_messages(&finish(session).await?);
    Ok(())
}

#[tokio::test]
async fn verify_refuses_and_writes_nothing() -> TestResult {
    let directory = repository_with_a_changed_anchor()?;
    let empty_path = tempfile::tempdir()?;
    let config = tempfile::tempdir()?;
    let abyme = directory.path().join("abyme");
    let before = files(&abyme)?;
    let session = start_with(
        directory.path(),
        &[("XDG_CONFIG_HOME", config.path().as_os_str())],
    )
    .await?;
    let cases = [
        (
            json!({"element": "x"}),
            "element x cannot be verified: files under its anchor differ from HEAD or are untracked; they must be committed first",
        ),
        (
            json!({"element": "nowhere"}),
            "no element has the ID nowhere",
        ),
    ];

    for (arguments, expected) in cases {
        let refused = call(&session, "verify", arguments).await??;

        assert_eq!(refused.is_error, Some(true), "{refused:?}");
        assert_eq!(text(&refused), expected);
    }
    for arguments in [
        json!({"element": "x", "commit": "0123456789abcdef0123456789abcdef01234567", "by": "person"}),
        json!({"element": "../x"}),
    ] {
        let refused = call(&session, "verify", arguments).await??;

        assert_eq!(refused.is_error, Some(true), "{refused:?}");
        assert!(
            text(&refused).starts_with("invalid arguments: "),
            "{refused:?}"
        );
    }
    assert_eq!(files(&abyme)?, before);
    assert_only_protocol_messages(&finish(session).await?);

    let session = start_with(directory.path(), &[("PATH", empty_path.path().as_os_str())]).await?;
    let refused = call(&session, "verify", json!({"element": "x"})).await??;
    assert_eq!(
        text(&refused),
        "element x cannot be verified: git could not be run"
    );
    assert_eq!(files(&abyme)?, before);
    assert_only_protocol_messages(&finish(session).await?);
    Ok(())
}

#[tokio::test]
async fn verify_refuses_in_a_repository_without_a_commit() -> TestResult {
    let directory = project()?;
    git(directory.path(), &["init", "-q"])?;
    write_element(directory.path(), "x", None, None)?;
    let abyme = directory.path().join("abyme");
    let before = files(&abyme)?;
    let config = tempfile::tempdir()?;
    let session = start_with(
        directory.path(),
        &[("XDG_CONFIG_HOME", config.path().as_os_str())],
    )
    .await?;

    let refused = call(&session, "verify", json!({"element": "x"})).await??;

    assert_eq!(refused.is_error, Some(true), "{refused:?}");
    assert_eq!(
        text(&refused),
        "element x cannot be verified: the repository has no commit yet"
    );
    assert_eq!(files(&abyme)?, before);
    assert_only_protocol_messages(&finish(session).await?);
    Ok(())
}

#[tokio::test]
async fn status_lists_the_elements_to_review_without_file_names() -> TestResult {
    let directory = repository_with_marked_elements()?;
    let config = tempfile::tempdir()?;
    let session = start_with(
        directory.path(),
        &[("XDG_CONFIG_HOME", config.path().as_os_str())],
    )
    .await?;

    let status = call(&session, "status", json!({})).await??;

    assert_eq!(status.is_error, Some(false), "{status:?}");
    assert_eq!(
        text(&status),
        "Elements to review: 1 with a task, 1 with only a hint.\n\
         changed  t  N    ⚠ task\n\
         hinted   t  N    ⚠ hint\n"
    );
    assert_only_protocol_messages(&finish(session).await?);
    Ok(())
}

#[tokio::test]
async fn status_names_why_review_flags_are_unavailable_and_takes_no_argument() -> TestResult {
    let directory = project()?;
    let session = start(directory.path()).await?;

    let unavailable = call(&session, "status", json!({})).await??;
    let with_argument = call(&session, "status", json!({"x": 1})).await??;

    assert_eq!(unavailable.is_error, Some(true), "{unavailable:?}");
    assert_eq!(
        text(&unavailable),
        "Review flags unavailable: the project lies in no Git repository"
    );
    assert_eq!(with_argument.is_error, Some(true), "{with_argument:?}");
    assert!(
        text(&with_argument).starts_with("invalid arguments: "),
        "{with_argument:?}"
    );
    assert_only_protocol_messages(&finish(session).await?);
    Ok(())
}

#[tokio::test]
async fn overview_lists_the_boards_as_a_tree_also_from_hand_edited_files() -> TestResult {
    let directory = project()?;
    let root = directory.path();
    let session = start(root).await?;
    let applied = call(
        &session,
        "apply",
        json!({"operations": [
            {"op": "create_board", "key": "ctx", "title": "Context", "summary": "The shop"},
            {"op": "create_element", "key": "shop", "type_ref": "c4/system", "name": "Shop"},
            {"op": "place_element", "element": {"new": "shop"}, "board": {"new": "ctx"}},
            {"op": "create_board", "key": "inside", "title": "Shop Containers",
             "owner": {"new": "shop"}, "summary": "Runs"},
            {"op": "create_element", "key": "pay", "type_ref": "c4/container", "name": "Payments"},
            {"op": "place_element", "element": {"new": "pay"}, "board": {"new": "inside"}},
            {"op": "create_board", "key": "parts", "title": "Payment Parts", "owner": {"new": "pay"}}
        ]}),
    )
    .await??;
    assert_eq!(applied.is_error, Some(false), "{applied:?}");
    // Hand edits make `loop-a` and `loop-b` lie inside each other, with a
    // title that tries to clear the screen and break the line.
    write_element(root, "ia", None, None)?;
    write_element(root, "ib", None, None)?;
    for (id, title, owner, placed) in [
        ("loop-a", "Loop\\u001b[2J\\nA", "ib", "ia"),
        ("loop-b", "Loop B", "ia", "ib"),
    ] {
        fs::write(
            root.join(format!("abyme/boards/{id}.board.json")),
            format!(
                "{{\"format\": 1, \"id\": \"{id}\", \"title\": \"{title}\", \"owner\": \"{owner}\", \
                 \"placements\": [{{\"element\": \"{placed}\", \"x\": 0, \"y\": 0, \"width\": 180, \
                 \"height\": 90}}]}}"
            ),
        )?;
    }

    let overview = call(&session, "overview", json!({})).await??;
    let with_argument = call(&session, "overview", json!({"x": 1})).await??;

    assert_eq!(overview.is_error, Some(false), "{overview:?}");
    assert_eq!(
        text(&overview),
        [
            "Review flags unavailable: the project lies in no Git repository".to_owned(),
            format!(
                "context{}Context{}The shop  1 element",
                " ".repeat(12),
                " ".repeat(11)
            ),
            format!(
                "  shop-containers  Shop Containers{}Runs{}1 element",
                " ".repeat(3),
                " ".repeat(6)
            ),
            format!(
                "    payment-parts  Payment Parts{}0 elements",
                " ".repeat(15)
            ),
            format!(
                "loop-a{}Loop\\u{{1b}}[2J\\nA{}1 element    Inside of: ib · Above: loop-b ⚠ cycle",
                " ".repeat(13),
                " ".repeat(12)
            ),
            format!(
                "loop-b{}Loop B{}1 element    Inside of: ia · Above: loop-a ⚠ cycle",
                " ".repeat(13),
                " ".repeat(22)
            ),
            String::new(),
        ]
        .join("\n")
    );
    assert_eq!(with_argument.is_error, Some(true), "{with_argument:?}");
    assert!(
        text(&with_argument).starts_with("invalid arguments: "),
        "{with_argument:?}"
    );
    assert_only_protocol_messages(&finish(session).await?);
    Ok(())
}

#[tokio::test]
async fn overview_counts_the_flags_of_each_board_s_own_elements() -> TestResult {
    let directory = repository_with_marked_elements()?;
    let config = tempfile::tempdir()?;
    let session = start_with(
        directory.path(),
        &[("XDG_CONFIG_HOME", config.path().as_os_str())],
    )
    .await?;

    let overview = call(&session, "overview", json!({})).await??;

    // `ghost` has no file, so four elements count.
    assert_eq!(text(&overview), "b  B    4 elements  ⚠ 1 task, 1 hint\n");
    assert_only_protocol_messages(&finish(session).await?);
    Ok(())
}

#[tokio::test]
async fn overview_lists_at_most_fifty_boards() -> TestResult {
    let directory = project()?;
    let boards = directory.path().join("abyme/boards");
    fs::create_dir_all(&boards)?;
    for index in 0..53 {
        fs::write(
            boards.join(format!("b{index:02}.board.json")),
            format!(
                "{{\"format\": 1, \"id\": \"b{index:02}\", \"title\": \"T\", \"placements\": []}}"
            ),
        )?;
    }
    let session = start(directory.path()).await?;

    let overview = call(&session, "overview", json!({})).await??;

    let text = text(&overview);
    let lines: Vec<&str> = text.lines().collect();
    assert_eq!(lines.len(), 1 + 50 + 1, "{text}");
    assert_eq!(lines.get(1), Some(&"b00  T    0 elements"));
    assert_eq!(lines.get(50), Some(&"b49  T    0 elements"));
    assert_eq!(lines.last(), Some(&"… and 3 more boards"));
    assert_only_protocol_messages(&finish(session).await?);
    Ok(())
}

#[tokio::test]
async fn overview_answers_whatever_the_size_of_a_title() -> TestResult {
    let directory = project()?;
    let boards = directory.path().join("abyme/boards");
    fs::create_dir_all(&boards)?;
    // Escaped in full, this title would take 2.4 MB, more than a reply may
    // hold.
    fs::write(
        boards.join("big.board.json"),
        format!(
            "{{\"format\": 1, \"id\": \"big\", \"title\": \"{}\", \"placements\": []}}",
            "\u{202e}".repeat(300_000)
        ),
    )?;
    let session = start(directory.path()).await?;

    let overview = call(&session, "overview", json!({})).await??;

    assert_eq!(overview.is_error, Some(false), "{overview:?}");
    assert_eq!(
        text(&overview),
        format!(
            "Review flags unavailable: the project lies in no Git repository\n\
             big  {}…    0 elements\n",
            "\\u{202e}".repeat(80)
        )
    );
    assert_only_protocol_messages(&finish(session).await?);
    Ok(())
}

#[tokio::test]
async fn read_element_asks_the_project_repository_whatever_git_variables_say() -> TestResult {
    let directory = repository_with_a_changed_anchor()?;
    let other = tempfile::tempdir()?;
    git(other.path(), &["init", "-q"])?;
    let config = tempfile::tempdir()?;
    let session = start_with(
        directory.path(),
        &[
            ("XDG_CONFIG_HOME", config.path().as_os_str()),
            ("GIT_DIR", other.path().join(".git").as_os_str()),
            ("GIT_WORK_TREE", other.path().as_os_str()),
            ("GIT_INDEX_FILE", other.path().join("no-index").as_os_str()),
        ],
    )
    .await?;

    let read = call(&session, "read_element", json!({"element": "x"})).await??;

    assert!(
        text(&read).contains(CHANGED_ANCHOR_REVIEW),
        "{}",
        text(&read)
    );
    assert_only_protocol_messages(&finish(session).await?);
    Ok(())
}

#[tokio::test]
async fn read_element_names_why_review_flags_are_unavailable() -> TestResult {
    let without_git = repository_with_a_changed_anchor()?;
    let empty_path = tempfile::tempdir()?;
    let broken = repository_with_a_changed_anchor()?;
    fs::write(broken.path().join(".git/index"), "not an index")?;
    let cases = [
        (
            without_git.path(),
            Some(empty_path.path()),
            "git could not be run",
        ),
        (broken.path(), None, "git failed"),
    ];

    for (directory, path, reason) in cases {
        let variables: Vec<(&str, &OsStr)> = path
            .map(|path| ("PATH", path.as_os_str()))
            .into_iter()
            .collect();
        let session = start_with(directory, &variables).await?;

        let read = call(&session, "read_element", json!({"element": "x"})).await??;

        let reply = text(&read);
        assert!(
            reply.starts_with(&format!(
                "Review flags unavailable: {reason}\nElement: x · X\n"
            )),
            "{reply}"
        );
        assert!(!reply.contains("Review:"), "{reply}");
        assert!(!reply.contains("fatal"), "{reply}");
        assert!(!reply.contains("error:"), "{reply}");
        assert!(!reply.contains(&directory.display().to_string()), "{reply}");
        assert_only_protocol_messages(&finish(session).await?);
    }
    Ok(())
}

#[tokio::test]
async fn apply_refuses_to_set_a_stamp_and_changes_no_file() -> TestResult {
    let directory = project_with_a_stamped_element()?;
    let session = start(directory.path()).await?;
    let before = files(directory.path())?;

    // With a name to change, only the stamp field can make the request fail.
    for (field, value) in [
        ("verified", "0123456789abcdef0123456789abcdef01234567"),
        ("verified_by", "person"),
    ] {
        let mut update =
            json!({"op": "update_element", "element": {"existing": "stamped"}, "name": "X"});
        update[field] = json!(value);
        let refused = call(&session, "apply", json!({"operations": [update]})).await??;

        assert_eq!(refused.is_error, Some(true), "{refused:?}");
        assert!(text(&refused).contains(field), "{}", text(&refused));
        assert_eq!(files(directory.path())?, before);
    }

    assert_only_protocol_messages(&finish(session).await?);
    Ok(())
}

#[tokio::test]
async fn apply_deletes_an_element_and_read_board_no_longer_shows_it() -> TestResult {
    let directory = project()?;
    let session = start(directory.path()).await?;
    call(&session, "apply", create_board_with_element()).await??;

    let deleted = call(
        &session,
        "apply",
        json!({"operations": [{"op": "delete_element", "element": {"existing": "payment-service"}}]}),
    )
    .await??;
    assert_eq!(deleted.is_error, Some(false), "{deleted:?}");

    let read = call(&session, "read_board", json!({"board": "container-level"})).await??;
    assert_eq!(
        text(&read),
        "Review flags unavailable: the project lies in no Git repository\nBoard: container-level · Container Level\n\n"
    );
    assert!(
        !directory
            .path()
            .join("abyme/elements/payment-service.md")
            .exists()
    );

    assert_only_protocol_messages(&finish(session).await?);
    Ok(())
}

#[tokio::test]
async fn apply_refuses_to_delete_an_element_that_others_relate_to() -> TestResult {
    let directory = project()?;
    let session = start(directory.path()).await?;
    call(&session, "apply", create_board_with_element()).await??;
    call(
        &session,
        "apply",
        json!({"operations": [
            {"op": "create_element", "key": "web", "type_ref": "c4/container", "name": "Web Shop"},
            {"op": "create_relation", "from": {"new": "web"}, "kind": "uses",
             "to": {"existing": "payment-service"}, "label": "SECRET-LABEL"}
        ]}),
    )
    .await??;
    let before = files(directory.path())?;

    let refused = call(
        &session,
        "apply",
        json!({"operations": [{"op": "delete_element", "element": {"existing": "payment-service"}}]}),
    )
    .await??;

    assert_eq!(refused.is_error, Some(true), "{refused:?}");
    assert_eq!(
        text(&refused),
        "operation 1: element payment-service is still the target of relations from \
         web-shop (uses); delete those first"
    );
    assert_eq!(files(directory.path())?, before);

    assert_only_protocol_messages(&finish(session).await?);
    Ok(())
}

#[tokio::test]
async fn read_element_names_an_element_without_a_file() -> TestResult {
    let directory = project()?;
    let session = start(directory.path()).await?;
    call(&session, "apply", create_board_with_element()).await??;
    fs::remove_file(directory.path().join("abyme/elements/payment-service.md"))?;

    for id in ["nowhere", "payment-service"] {
        let read = call(&session, "read_element", json!({"element": id})).await??;

        assert_eq!(read.is_error, Some(true), "{read:?}");
        assert_eq!(text(&read), format!("no element has the ID {id}"));
    }

    assert_only_protocol_messages(&finish(session).await?);
    Ok(())
}

#[tokio::test]
async fn read_element_answers_with_an_error_rather_than_a_cut_detail() -> TestResult {
    let directory = project()?;
    let session = start(directory.path()).await?;
    let body = "\u{202e}".repeat(300_000);
    let applied = call(
        &session,
        "apply",
        json!({"operations": [
            {"op": "create_element", "key": "big", "type_ref": "t", "name": "Big", "body": body}
        ]}),
    )
    .await??;
    assert_eq!(applied.is_error, Some(false), "{applied:?}");

    let read = call(&session, "read_element", json!({"element": "big"})).await??;

    assert_eq!(read.is_error, Some(true), "{read:?}");
    assert!(
        text(&read).starts_with("the detail of element big has "),
        "{read:?}"
    );
    assert!(!text(&read).contains('\u{202e}'), "{read:?}");

    assert_only_protocol_messages(&finish(session).await?);
    Ok(())
}

#[tokio::test]
async fn apply_refuses_an_unknown_relation_kind_and_changes_no_file() -> TestResult {
    let directory = project()?;
    let session = start(directory.path()).await?;
    call(&session, "apply", create_board_with_element()).await??;
    let before = files(directory.path())?;

    let rejected = call(
        &session,
        "apply",
        json!({"operations": [
            {"op": "create_relation", "from": {"existing": "payment-service"}, "kind": "contains",
             "to": {"existing": "payment-service"}, "label": "SECRET-LABEL"}
        ]}),
    )
    .await??;

    assert_eq!(rejected.is_error, Some(true), "{rejected:?}");
    assert!(text(&rejected).contains("contains"), "{rejected:?}");
    assert!(!text(&rejected).contains("SECRET"), "{rejected:?}");
    assert_eq!(files(directory.path())?, before);

    assert_only_protocol_messages(&finish(session).await?);
    Ok(())
}

#[tokio::test]
async fn apply_moves_an_element_and_refuses_a_position_out_of_range() -> TestResult {
    let directory = project()?;
    let session = start(directory.path()).await?;
    call(&session, "apply", create_board_with_element()).await??;
    let board_file = directory
        .path()
        .join("abyme/boards/container-level.board.json");
    let move_to = |x: i64, y: i64| {
        json!({"operations": [{"op": "move_element", "element": {"existing": "payment-service"},
                               "board": {"existing": "container-level"}, "x": x, "y": y}]})
    };

    let moved = call(&session, "apply", move_to(400, 200)).await??;

    assert_eq!(moved.is_error, Some(false), "{moved:?}");
    assert_eq!(text(&moved), "The request created nothing.");
    let board = fs::read_to_string(&board_file)?;
    assert!(
        board.contains(
            r#"{"element": "payment-service", "x": 400, "y": 200, "width": 180, "height": 90}"#
        ),
        "{board}"
    );

    let before = fs::read(&board_file)?;
    let refused = call(&session, "apply", move_to(1_000_001, 0)).await??;

    assert_eq!(refused.is_error, Some(true), "{refused:?}");
    assert_eq!(
        text(&refused),
        "operation 1: the position x 1000001, y 0 lies outside -1000000 to 1000000"
    );
    assert_eq!(fs::read(&board_file)?, before);
    assert_only_protocol_messages(&finish(session).await?);
    Ok(())
}

#[tokio::test]
async fn apply_moves_a_pinned_element_only_when_the_request_says_what_becomes_of_the_pin()
-> TestResult {
    let directory = project()?;
    let session = start(directory.path()).await?;
    call(&session, "apply", create_board_with_element()).await??;
    let board_file = directory
        .path()
        .join("abyme/boards/container-level.board.json");
    let move_with = |pin: Value| {
        let mut operation = json!({"op": "move_element", "element": {"existing": "payment-service"},
                                   "board": {"existing": "container-level"}, "x": 400, "y": 200});
        if let (Value::Object(operation), Value::Bool(_)) = (&mut operation, &pin) {
            operation.insert("pinned".to_owned(), pin);
        }
        json!({"operations": [operation]})
    };
    let line = |pin: &str| {
        format!(
            r#"{{"element": "payment-service", "x": 400, "y": 200, "width": 180, "height": 90{pin}}}"#
        )
    };

    let pinned = call(&session, "apply", move_with(json!(true))).await??;

    assert_eq!(pinned.is_error, Some(false), "{pinned:?}");
    let board = fs::read_to_string(&board_file)?;
    assert!(board.contains(&line(r#", "pinned": true"#)), "{board}");

    let before = fs::read(&board_file)?;
    let refused = call(&session, "apply", move_with(Value::Null)).await??;

    assert_eq!(refused.is_error, Some(true), "{refused:?}");
    assert!(
        text(&refused).contains("is pinned on board container-level"),
        "{refused:?}"
    );
    assert_eq!(fs::read(&board_file)?, before);

    let coordinates = call(
        &session,
        "read_board",
        json!({"board": "container-level", "coordinates": true}),
    )
    .await??;
    assert!(
        text(&coordinates).contains("@400,200 180x90 pinned"),
        "{coordinates:?}"
    );

    let released = call(&session, "apply", move_with(json!(false))).await??;

    assert_eq!(released.is_error, Some(false), "{released:?}");
    let board = fs::read_to_string(&board_file)?;
    assert!(board.contains(&line("")), "{board}");
    assert_only_protocol_messages(&finish(session).await?);
    Ok(())
}

#[tokio::test]
async fn read_board_shows_coordinates_only_on_request() -> TestResult {
    let directory = project()?;
    let session = start(directory.path()).await?;
    call(&session, "apply", create_board_with_element()).await??;
    let moved = call(
        &session,
        "apply",
        json!({"operations": [{"op": "move_element", "element": {"existing": "payment-service"},
                               "board": {"existing": "container-level"}, "x": 400, "y": 200}]}),
    )
    .await??;
    assert_eq!(moved.is_error, Some(false), "{moved:?}");

    let with = call(
        &session,
        "read_board",
        json!({"board": "container-level", "coordinates": true}),
    )
    .await??;
    let without = call(&session, "read_board", json!({"board": "container-level"})).await??;

    assert_eq!(with.is_error, Some(false), "{with:?}");
    assert_eq!(
        text(&with),
        "Review flags unavailable: the project lies in no Git repository\n\
         Board: container-level · Container Level\n\
         \n\
         payment-service  @400,200 180x90  c4/container  Payment Service  Payments through Stripe\n"
    );
    assert_eq!(
        text(&without),
        "Review flags unavailable: the project lies in no Git repository\n\
         Board: container-level · Container Level\n\
         \n\
         payment-service  c4/container  Payment Service  Payments through Stripe\n"
    );
    assert_only_protocol_messages(&finish(session).await?);
    Ok(())
}

#[tokio::test]
async fn read_board_takes_coordinates_only_as_an_optional_boolean() -> TestResult {
    let directory = project()?;
    let session = start(directory.path()).await?;

    let tools = tokio::time::timeout(TIMEOUT, session.client.list_all_tools()).await??;
    let read_board = tools
        .iter()
        .find(|tool| tool.name == "read_board")
        .ok_or("no read_board")?;
    let schema = &read_board.input_schema;
    assert_eq!(
        schema
            .get("properties")
            .and_then(|properties| properties.get("coordinates"))
            .and_then(|coordinates| coordinates.get("type")),
        Some(&json!("boolean")),
        "{schema:?}"
    );
    assert_eq!(
        schema.get("required"),
        Some(&json!(["board"])),
        "{schema:?}"
    );
    assert_eq!(
        schema.get("additionalProperties"),
        Some(&json!(false)),
        "{schema:?}"
    );

    for arguments in [
        json!({"board": "main", "coordinates": "yes"}),
        json!({"board": "main", "coordinates": 1}),
        json!({"board": "main", "coordinates": null}),
        json!({"coordinates": true}),
    ] {
        let result = call(&session, "read_board", arguments.clone()).await??;

        assert_eq!(result.is_error, Some(true), "{arguments}: {result:?}");
        assert!(
            text(&result).starts_with("invalid arguments: "),
            "{result:?}"
        );
    }
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

/// The server has a cache and a way out to the network here, so the test
/// shows that without a project it uses neither.
#[tokio::test]
async fn without_a_project_every_tool_asks_for_init_and_writes_nothing() -> TestResult {
    let directory = tempfile::tempdir()?;
    let cache = tempfile::tempdir()?;
    let proxy = TcpListener::bind("127.0.0.1:0")?;
    proxy.set_nonblocking(true)?;
    let proxy_url = proxy_url(&proxy)?;
    let session = start_with(
        directory.path(),
        &[
            ("XDG_CACHE_HOME", cache.path().as_os_str()),
            ("ALL_PROXY", OsStr::new(&proxy_url)),
        ],
    )
    .await?;

    let applied = call(&session, "apply", create_board_with_element()).await??;
    let read = call(&session, "read_board", json!({"board": "main"})).await??;
    let searched = call(&session, "search", json!({"query": "payments"})).await??;
    let status = call(&session, "status", json!({})).await??;

    for result in [applied, read, searched, status] {
        assert_eq!(result.is_error, Some(true), "{result:?}");
        assert!(text(&result).contains("abyme-mcp init"), "{result:?}");
    }
    assert_only_protocol_messages(&finish(session).await?);
    assert!(fs::read_dir(directory.path())?.next().is_none());
    assert!(fs::read_dir(cache.path())?.next().is_none());
    assert_eq!(
        proxy.accept().map_err(|error| error.kind()).err(),
        Some(io::ErrorKind::WouldBlock),
        "the server tried to download"
    );
    Ok(())
}

#[tokio::test]
async fn without_a_cache_search_fails_and_the_other_tools_keep_working() -> TestResult {
    let directory = project()?;
    let session = start(directory.path()).await?;

    let searched = call(&session, "search", json!({"query": "payments"})).await??;
    let applied = call(&session, "apply", create_board_with_element()).await??;
    let read = call(&session, "read_board", json!({"board": "container-level"})).await??;

    assert_eq!(searched.is_error, Some(true), "{searched:?}");
    assert!(
        text(&searched).contains("search needs a cache directory"),
        "{searched:?}"
    );
    assert_eq!(applied.is_error, Some(false), "{applied:?}");
    assert_eq!(read.is_error, Some(false), "{read:?}");
    assert_only_protocol_messages(&finish(session).await?);
    Ok(())
}

/// The download goes through a proxy that accepts the connection and never
/// answers, so it hangs until the server exits.
#[tokio::test]
async fn exits_when_standard_input_closes_while_the_model_download_hangs() -> TestResult {
    let directory = project()?;
    let cache = tempfile::tempdir()?;
    let proxy = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let proxy_url = format!("http://{}", proxy.local_addr()?);
    let session = start_with(
        directory.path(),
        &[
            ("XDG_CACHE_HOME", cache.path().as_os_str()),
            ("ALL_PROXY", OsStr::new(&proxy_url)),
        ],
    )
    .await?;

    let (_connection, _) = tokio::time::timeout(EXIT_TIMEOUT, proxy.accept())
        .await
        .map_err(|_| "the server did not start the download")??;
    let Session {
        client, mut child, ..
    } = session;
    tokio::time::timeout(TIMEOUT, client.cancel()).await??;
    let status = tokio::time::timeout(EXIT_TIMEOUT, child.wait())
        .await
        .map_err(|_| "the server did not exit while the download hung")??;

    assert!(status.success(), "{status}");
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
        let expected: &[&str] = match tool.name.as_ref() {
            "apply" => &["operations"],
            "read_board" => &["board", "coordinates"],
            "read_element" | "verify" => &["element"],
            "search" => &["query", "statement"],
            "overview" | "status" => &[],
            other => return Err(format!("unexpected tool {other}").into()),
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

#[tokio::test]
#[ignore = "downloads the real model from Hugging Face"]
async fn search_finds_elements_by_word_and_by_meaning_with_the_real_model() -> TestResult {
    let directory = project()?;
    let cache = tempfile::tempdir()?;
    let session = start_with(
        directory.path(),
        &[("XDG_CACHE_HOME", cache.path().as_os_str())],
    )
    .await?;
    let applied = call(
        &session,
        "apply",
        json!({"operations": [
            {"op": "create_element", "key": "pay", "type_ref": "c4/container",
             "name": "Payment Service", "summary": "Charges credit cards through Stripe"},
            {"op": "create_element", "key": "mail", "type_ref": "c4/container",
             "name": "Mailer", "summary": "Sends newsletters to customers by email"},
            {"op": "create_element", "key": "stock", "type_ref": "c4/container",
             "name": "Inventory", "summary": "Tracks how many items the warehouse holds"}
        ]}),
    )
    .await??;
    assert_eq!(applied.is_error, Some(false), "{applied:?}");

    let by_word = call_within(
        &session,
        "search",
        json!({"query": "Stripe"}),
        Duration::from_mins(10),
    )
    .await??;
    let by_meaning = call(
        &session,
        "search",
        json!({"query": "electronic messages for subscribers"}),
    )
    .await??;

    eprintln!(
        "by word:\n{}\nby meaning:\n{}",
        text(&by_word),
        text(&by_meaning)
    );
    assert!(
        text(&by_word).starts_with(&format!("{NO_REPOSITORY}payment-service  ")),
        "{by_word:?}"
    );
    assert!(
        text(&by_meaning).starts_with(&format!("{NO_REPOSITORY}mailer  ")),
        "{by_meaning:?}"
    );
    assert_only_protocol_messages(&finish(session).await?);
    Ok(())
}

/// Search marks its lines like a board outline, with the real model.
#[tokio::test]
#[ignore = "downloads the real model from Hugging Face"]
async fn search_marks_the_elements_to_review_with_the_real_model() -> TestResult {
    let directory = repository_with_marked_elements()?;
    let cache = tempfile::tempdir()?;
    let config = tempfile::tempdir()?;
    let session = start_with(
        directory.path(),
        &[
            ("XDG_CACHE_HOME", cache.path().as_os_str()),
            ("XDG_CONFIG_HOME", config.path().as_os_str()),
        ],
    )
    .await?;

    let found = call_within(
        &session,
        "search",
        json!({"query": "N"}),
        Duration::from_secs(600),
    )
    .await??;

    let reply = text(&found);
    eprintln!("{reply}");
    let line = |id: &str| {
        reply
            .lines()
            .find(|line| line.starts_with(&format!("{id} ")))
            .unwrap_or_default()
            .to_owned()
    };
    assert!(line("changed").ends_with("⚠ task"), "{reply}");
    assert!(line("hinted").ends_with("⚠ hint"), "{reply}");
    assert!(!line("calm").contains('⚠'), "{reply}");
    assert_only_protocol_messages(&finish(session).await?);
    Ok(())
}

/// Also embeds an element far longer than the 512 tokens the model reads,
/// which the tokenizer must cut instead of failing the search.
#[tokio::test]
#[ignore = "downloads the real multilingual model from Hugging Face"]
async fn search_finds_an_english_element_through_a_german_query_with_the_multilingual_model()
-> TestResult {
    let directory = project()?;
    fs::write(
        directory.path().join("abyme/abyme.toml"),
        "format = 1\nembedding_model = \"paraphrase-multilingual-minilm-l12-v2\"\n",
    )?;
    let cache = tempfile::tempdir()?;
    let session = start_with(
        directory.path(),
        &[("XDG_CACHE_HOME", cache.path().as_os_str())],
    )
    .await?;
    let long_body = "The archive keeps every report of the year. ".repeat(200);
    let applied = call(
        &session,
        "apply",
        json!({"operations": [
            {"op": "create_element", "key": "pay", "type_ref": "c4/container",
             "name": "Payment Service", "summary": "Charges credit cards through Stripe"},
            {"op": "create_element", "key": "mail", "type_ref": "c4/container",
             "name": "Mailer", "summary": "Sends newsletters to customers by email"},
            {"op": "create_element", "key": "stock", "type_ref": "c4/container",
             "name": "Inventory", "summary": "Tracks how many items the warehouse holds"},
            {"op": "create_element", "key": "archive", "type_ref": "c4/container",
             "name": "Archive", "summary": "Keeps old reports", "body": long_body}
        ]}),
    )
    .await??;
    assert_eq!(applied.is_error, Some(false), "{applied:?}");

    let found = call_within(
        &session,
        "search",
        json!({"query": "Rundbriefe an Kunden verschicken"}),
        Duration::from_mins(15),
    )
    .await??;

    eprintln!("Rundbriefe an Kunden verschicken:\n{}", text(&found));
    assert_eq!(found.is_error, Some(false), "{found:?}");
    assert!(
        text(&found).starts_with(&format!("{NO_REPOSITORY}mailer  ")),
        "{found:?}"
    );
    let models: Vec<String> = fs::read_dir(cache.path().join("abyme/models"))?
        .map(|entry| entry.map(|entry| entry.file_name().to_string_lossy().into_owned()))
        .collect::<Result<_, _>>()?;
    assert_eq!(models, ["paraphrase-multilingual-minilm-l12-v2"]);
    assert_only_protocol_messages(&finish(session).await?);
    Ok(())
}
