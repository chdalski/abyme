//! The built `abyme-mcp` binary: `init` on the command line, and the MCP
//! server on standard input and output, driven by an MCP client.
//!
//! The server runs without `HOME`, `XDG_CACHE_HOME`, and proxy settings, so
//! no test touches the user's cache or the network unless it sets them; only
//! the ignored test downloads the model.

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
    assert_eq!(names, ["apply", "read_board", "search"]);

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

    for result in [applied, read, searched] {
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
            "read_board" => &["board"],
            "search" => &["query"],
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
        text(&by_word).starts_with("payment-service  "),
        "{by_word:?}"
    );
    assert!(text(&by_meaning).starts_with("mailer  "), "{by_meaning:?}");
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
    assert!(text(&found).starts_with("mailer  "), "{found:?}");
    let models: Vec<String> = fs::read_dir(cache.path().join("abyme/models"))?
        .map(|entry| entry.map(|entry| entry.file_name().to_string_lossy().into_owned()))
        .collect::<Result<_, _>>()?;
    assert_eq!(models, ["paraphrase-multilingual-minilm-l12-v2"]);
    assert_only_protocol_messages(&finish(session).await?);
    Ok(())
}
