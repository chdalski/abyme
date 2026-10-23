//! The server's helpers, and the tools in an MCP session in this process,
//! with a fake model loader and temporary directories.

use std::error::Error;
use std::fs;
use std::sync::atomic::{AtomicUsize, Ordering};

use abyme_index::{EmbedError, Embedder, ModelSpec, UserCache};
use rmcp::handler::server::common::schema_for_input;
use rmcp::model::CallToolRequestParams;
use rmcp::service::RunningService;
use rmcp::{RoleClient, ServiceExt as _};
use serde_json::{Value, json};
use tempfile::TempDir;

use super::*;
use crate::models::Loader;

type TestResult<T = ()> = Result<T, Box<dyn Error>>;

/// Counts words into a vector, so texts that share words lie close.
struct WordsEmbedder {
    dimension: usize,
}

/// What the fake loader did and is to do.
#[derive(Default)]
struct FakeLoads {
    /// How often it was called.
    calls: AtomicUsize,
    /// How many calls are still to fail.
    failures_left: AtomicUsize,
    /// The error a failing call returns.
    failure: String,
}

/// A project directory, a cache directory, and the fake loader's record.
struct Setup {
    project: TempDir,
    cache: TempDir,
    loads: Arc<FakeLoads>,
}

impl Embedder for WordsEmbedder {
    fn embed(&mut self, texts: &[String]) -> Result<Vec<Vec<f32>>, EmbedError> {
        Ok(texts
            .iter()
            .map(|text| {
                let mut vector = vec![0.0; self.dimension];
                for word in text.split_whitespace() {
                    let hash = word.to_lowercase().bytes().fold(7_usize, |hash, byte| {
                        hash.wrapping_mul(31).wrapping_add(usize::from(byte))
                    });
                    vector[hash % self.dimension] += 1.0;
                }
                vector
            })
            .collect())
    }
}

impl Setup {
    /// A setup whose project directory is an Abyme project or empty, and
    /// whose loader fails the first `failures` calls with `failure`.
    fn new(project: bool, failures: usize, failure: &str) -> TestResult<Self> {
        let setup = Self {
            project: TempDir::new()?,
            cache: TempDir::new()?,
            loads: Arc::new(FakeLoads {
                calls: AtomicUsize::new(0),
                failures_left: AtomicUsize::new(failures),
                failure: failure.to_owned(),
            }),
        };
        if project {
            Project::init(setup.project.path())?;
        }
        Ok(setup)
    }

    fn server(&self) -> TestResult<AbymeServer> {
        let cache = UserCache::from_env(Some(self.cache.path().into()), None).ok_or("no cache")?;
        let models = Models::new(fake_loader(Arc::clone(&self.loads)), Some(cache));
        Ok(AbymeServer::new(
            self.project.path().to_path_buf(),
            Arc::new(models),
        ))
    }

    fn loads(&self) -> usize {
        self.loads.calls.load(Ordering::SeqCst)
    }

    fn cache_is_empty(&self) -> TestResult<bool> {
        Ok(fs::read_dir(self.cache.path())?.next().is_none())
    }
}

fn fake_loader(loads: Arc<FakeLoads>) -> Loader {
    Arc::new(move |spec: &'static ModelSpec, _cache: &UserCache| {
        loads.calls.fetch_add(1, Ordering::SeqCst);
        let failing = loads
            .failures_left
            .try_update(Ordering::SeqCst, Ordering::SeqCst, |left| {
                left.checked_sub(1)
            })
            .is_ok();
        if failing {
            return Err(loads.failure.clone());
        }
        Ok(Box::new(WordsEmbedder {
            dimension: spec.dimension,
        }) as Box<dyn Embedder + Send>)
    })
}

/// Serves `server` in this process and connects a client to it.
async fn connect(server: AbymeServer) -> TestResult<RunningService<RoleClient, ()>> {
    let (client_side, server_side) = tokio::io::duplex(1 << 20);
    tokio::spawn(async move {
        if let Ok(service) = server.serve(tokio::io::split(server_side)).await {
            let _ = service.waiting().await;
        }
    });
    Ok(().serve(tokio::io::split(client_side)).await?)
}

/// Calls a tool with `arguments`, which must be a JSON object.
async fn call(
    client: &RunningService<RoleClient, ()>,
    tool: &'static str,
    arguments: Value,
) -> TestResult<CallToolResult> {
    let Value::Object(arguments) = arguments else {
        return Err("arguments must be an object".into());
    };
    Ok(client
        .call_tool(CallToolRequestParams::new(tool).with_arguments(arguments))
        .await?)
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

/// Creates elements with the given names and summaries.
async fn create(client: &RunningService<RoleClient, ()>, elements: &[(&str, &str)]) -> TestResult {
    let operations: Vec<Value> = elements
        .iter()
        .enumerate()
        .map(|(number, (name, summary))| {
            json!({"op": "create_element", "key": format!("e{number}"), "type_ref": "c4/container",
                   "name": name, "summary": summary})
        })
        .collect();
    let applied = call(client, "apply", json!({"operations": operations})).await?;
    assert_eq!(applied.is_error, Some(false), "{applied:?}");
    Ok(())
}

#[test]
fn rejects_a_reply_over_the_limit_without_showing_it() {
    let error = bounded("the search result", "secret".repeat(MAX_REPLY_BYTES)).unwrap_err();

    assert!(error.starts_with("the search result has "), "{error}");
    assert!(error.contains(&MAX_REPLY_BYTES.to_string()), "{error}");
    assert!(!error.contains("secret"), "{error}");
}

#[test]
fn returns_a_reply_at_the_limit_whole() {
    let text = "x".repeat(MAX_REPLY_BYTES);

    assert_eq!(bounded("the outline of board big", text.clone()), Ok(text));
}

#[test]
fn rejects_an_outline_over_the_limit_instead_of_cutting_it() {
    let error = bounded("the outline of board big", "x".repeat(MAX_REPLY_BYTES + 1)).unwrap_err();

    assert!(error.contains("board big"), "{error}");
    assert!(error.contains(&MAX_REPLY_BYTES.to_string()), "{error}");
}

#[test]
fn advertises_object_schemas_as_mcp_requires() {
    assert!(schema_for_input::<ApplyArguments>().is_ok());
    assert!(schema_for_input::<ReadBoardArguments>().is_ok());
    assert!(schema_for_input::<SearchArguments>().is_ok());
}

#[test]
fn says_so_when_a_request_created_nothing() {
    assert_eq!(report(&[]), "The request created nothing.");
}

#[test]
fn reports_the_id_each_key_received() {
    let created = vec![
        (
            LocalKey::try_from("b".to_owned()).unwrap(),
            Created::Board("main".parse().unwrap()),
        ),
        (
            LocalKey::try_from("pay".to_owned()).unwrap(),
            Created::Element("payment-service".parse().unwrap()),
        ),
    ];

    assert_eq!(
        report(&created),
        "b: board main\npay: element payment-service\n"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn search_ranks_the_matching_element_first_and_returns_at_most_ten() -> TestResult {
    let setup = Setup::new(true, 0, "")?;
    let client = connect(setup.server()?).await?;
    let workers: Vec<(String, &str)> = (0..12)
        .map(|number| (format!("Worker {number}"), "runs queued jobs"))
        .collect();
    let mut elements: Vec<(&str, &str)> = workers
        .iter()
        .map(|(name, summary)| (name.as_str(), *summary))
        .collect();
    elements.push(("Payment Service", "charges cards through Stripe"));
    create(&client, &elements).await?;

    let stripe = call(&client, "search", json!({"query": "Stripe cards"})).await?;
    let jobs = call(&client, "search", json!({"query": "queued jobs"})).await?;

    assert_eq!(stripe.is_error, Some(false), "{stripe:?}");
    assert!(
        text(&stripe).starts_with("payment-service  c4/container  Payment Service  "),
        "{}",
        text(&stripe)
    );
    assert_eq!(jobs.is_error, Some(false), "{jobs:?}");
    let lines: Vec<String> = text(&jobs).lines().map(str::to_owned).collect();
    assert_eq!(lines.len(), SEARCH_RESULTS, "{lines:?}");
    assert!(
        lines.iter().all(|line| line.starts_with("worker-")),
        "{lines:?}"
    );
    assert_eq!(setup.loads(), 1);
    client.cancel().await?;
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn preparing_at_start_and_then_searching_loads_the_model_once() -> TestResult {
    let setup = Setup::new(true, 0, "")?;
    let server = setup.server()?;

    server.prefetch().await?;
    let client = connect(server).await?;
    create(&client, &[("Mailer", "sends newsletters")]).await?;
    let found = call(&client, "search", json!({"query": "newsletters"})).await?;

    assert!(text(&found).starts_with("mailer  "), "{found:?}");
    assert_eq!(setup.loads(), 1);
    client.cancel().await?;
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn in_a_project_without_elements_no_element_matches() -> TestResult {
    let setup = Setup::new(true, 0, "")?;
    let client = connect(setup.server()?).await?;

    let found = call(&client, "search", json!({"query": "anything"})).await?;

    assert_eq!(found.is_error, Some(false), "{found:?}");
    assert_eq!(text(&found), NO_MATCH);
    client.cancel().await?;
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn loads_the_model_at_the_first_search_once_a_project_exists() -> TestResult {
    let setup = Setup::new(false, 0, "")?;
    let server = setup.server()?;

    server.prefetch().await?;
    let client = connect(server).await?;
    let without = call(&client, "search", json!({"query": "anything"})).await?;

    assert_eq!(without.is_error, Some(true), "{without:?}");
    assert!(text(&without).contains("abyme-mcp init"), "{without:?}");
    assert_eq!(setup.loads(), 0);
    assert!(setup.cache_is_empty()?);

    Project::init(setup.project.path())?;
    let with = call(&client, "search", json!({"query": "anything"})).await?;

    assert_eq!(with.is_error, Some(false), "{with:?}");
    assert_eq!(setup.loads(), 1);
    client.cancel().await?;
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn loads_the_model_at_the_first_search_once_the_project_file_is_accepted() -> TestResult {
    let setup = Setup::new(true, 0, "")?;
    let project_file = setup.project.path().join("abyme/abyme.toml");
    fs::write(&project_file, "format = 99\n")?;
    let server = setup.server()?;

    server.prefetch().await?;
    let client = connect(server).await?;
    let rejected = call(&client, "search", json!({"query": "anything"})).await?;

    assert_eq!(rejected.is_error, Some(true), "{rejected:?}");
    assert_eq!(setup.loads(), 0);
    assert!(setup.cache_is_empty()?);

    fs::write(&project_file, "format = 1\n")?;
    let accepted = call(&client, "search", json!({"query": "anything"})).await?;

    assert_eq!(accepted.is_error, Some(false), "{accepted:?}");
    assert_eq!(setup.loads(), 1);
    client.cancel().await?;
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn a_failed_load_is_an_escaped_tool_error_and_the_next_search_loads_again() -> TestResult {
    let failure = format!("download failed \u{1b}[31m{}", "x".repeat(1000));
    let setup = Setup::new(true, 1, &failure)?;
    let client = connect(setup.server()?).await?;

    let failed = call(&client, "search", json!({"query": "anything"})).await?;

    assert_eq!(failed.is_error, Some(true), "{failed:?}");
    let message = text(&failed);
    assert!(message.contains("download failed \\u{1b}[31m"), "{message}");
    assert!(!message.contains('\u{1b}'), "{message}");
    assert!(message.contains('…'), "{message}");
    assert!(message.chars().count() < 400, "{message}");
    assert_eq!(setup.loads(), 1);

    let retried = call(&client, "search", json!({"query": "anything"})).await?;

    assert_eq!(retried.is_error, Some(false), "{retried:?}");
    assert_eq!(setup.loads(), 2);
    client.cancel().await?;
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn answers_queries_it_cannot_search_with_tool_errors_before_loading() -> TestResult {
    let setup = Setup::new(true, 0, "")?;
    let client = connect(setup.server()?).await?;
    let not_searchable = [
        (json!({"query": ""}), "the query is empty"),
        (json!({"query": " \t\n"}), "the query is empty"),
        (
            json!({"query": "x".repeat(MAX_QUERY_CHARS + 1)}),
            "the query has 1001 characters",
        ),
        (json!({}), "invalid arguments: "),
        (json!({"query": "a", "extra": 1}), "invalid arguments: "),
        (json!({"query": 5}), "invalid arguments: "),
    ];

    for (arguments, expected) in not_searchable {
        let result = call(&client, "search", arguments.clone()).await?;

        assert_eq!(result.is_error, Some(true), "{arguments}: {result:?}");
        assert!(text(&result).starts_with(expected), "{result:?}");
        assert!(!text(&result).contains("xxxx"), "{result:?}");
    }
    assert_eq!(setup.loads(), 0);
    assert!(setup.cache_is_empty()?);

    let longest = call(
        &client,
        "search",
        json!({"query": "x".repeat(MAX_QUERY_CHARS)}),
    )
    .await?;

    assert_eq!(longest.is_error, Some(false), "{longest:?}");
    client.cancel().await?;
    Ok(())
}
