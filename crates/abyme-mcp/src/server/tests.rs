//! The server's helpers, and the tools in an MCP session in this process,
//! with a fake model loader and temporary directories.

use std::error::Error;
use std::fs;
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

use abyme_index::{EmbedError, Embedder, ModelSpec, UserCache};
use abyme_test_support::{commit_all, git};
use rmcp::handler::server::common::schema_for_input;
use rmcp::model::CallToolRequestParams;
use rmcp::service::RunningService;
use rmcp::{RoleClient, ServiceExt as _};
use serde_json::{Value, json};
use tempfile::TempDir;

use super::*;
use crate::models::Loader;

type TestResult<T = ()> = Result<T, Box<dyn Error>>;

/// The name of the multilingual model in a project file.
const MULTILINGUAL: &str = "paraphrase-multilingual-minilm-l12-v2";

/// Counts words into a vector, so texts that share words lie close, and
/// records how many texts it embedded for its model.
struct WordsEmbedder {
    model: &'static str,
    dimension: usize,
    loads: Arc<FakeLoads>,
}

/// What the fake loader did and is to do.
#[derive(Default)]
struct FakeLoads {
    /// The name of the model of each call, in order.
    loaded: Mutex<Vec<&'static str>>,
    /// The model and the number of texts of each embedding.
    embedded: Mutex<Vec<(&'static str, usize)>>,
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
        self.loads
            .embedded
            .lock()
            .unwrap()
            .push((self.model, texts.len()));
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
                loaded: Mutex::new(Vec::new()),
                embedded: Mutex::new(Vec::new()),
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
        self.loaded().len()
    }

    /// The name of the model of each load, in order.
    fn loaded(&self) -> Vec<&'static str> {
        self.loads.loaded.lock().unwrap().clone()
    }

    /// How many texts the model named `model` embedded, in all.
    fn embedded_by(&self, model: &str) -> usize {
        self.loads
            .embedded
            .lock()
            .unwrap()
            .iter()
            .filter(|(embedder, _)| *embedder == model)
            .map(|(_, texts)| texts)
            .sum()
    }

    /// Writes the project file, with `embedding_model` set to `model`.
    fn choose_model(&self, model: &str) -> TestResult {
        fs::write(
            self.project.path().join("abyme/abyme.toml"),
            format!("format = 1\nembedding_model = \"{model}\"\n"),
        )?;
        Ok(())
    }

    /// The directories of the project's index in the cache, one per model.
    fn table_dirs(&self) -> TestResult<Vec<String>> {
        let index = self.cache.path().join("abyme/index");
        let project = fs::read_dir(&index)?
            .next()
            .ok_or("no project in the index")??
            .path();
        let mut names: Vec<String> = fs::read_dir(project)?
            .map(|entry| entry.map(|entry| entry.file_name().to_string_lossy().into_owned()))
            .collect::<Result<_, _>>()?;
        names.retain(|name| name != "lock");
        names.sort();
        Ok(names)
    }

    fn cache_is_empty(&self) -> TestResult<bool> {
        Ok(fs::read_dir(self.cache.path())?.next().is_none())
    }
}

fn fake_loader(loads: Arc<FakeLoads>) -> Loader {
    Arc::new(move |spec: &'static ModelSpec, _cache: &UserCache| {
        loads.loaded.lock().unwrap().push(spec.name);
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
            model: spec.name,
            dimension: spec.dimension,
            loads: Arc::clone(&loads),
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

/// The line that starts a reply of a read tool in a project outside Git.
const NO_REPOSITORY: &str = "Review flags unavailable: the project lies in no Git repository\n";

/// The search results of a reply in a project outside Git, which starts
/// with the line that names why there are no review flags, and marks no line.
fn results(result: &CallToolResult) -> TestResult<String> {
    let text = text(result);
    let results = text
        .strip_prefix(NO_REPOSITORY)
        .ok_or_else(|| format!("no line for unavailable flags: {text:?}"))?;
    if results.contains('⚠') {
        return Err(format!("a line is marked: {text:?}").into());
    }
    Ok(results.to_owned())
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
        results(&stripe)?.starts_with("payment-service  c4/container  Payment Service  "),
        "{}",
        text(&stripe)
    );
    assert_eq!(jobs.is_error, Some(false), "{jobs:?}");
    let lines: Vec<String> = results(&jobs)?.lines().map(str::to_owned).collect();
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

    assert!(results(&found)?.starts_with("mailer  "), "{found:?}");
    assert_eq!(setup.loads(), 1);
    client.cancel().await?;
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn search_marks_its_lines_and_names_no_file() -> TestResult {
    let setup = Setup::new(true, 0, "")?;
    let root = setup.project.path();
    git(root, &["init", "-q"])?;
    fs::create_dir_all(root.join("crates/c"))?;
    fs::write(root.join("crates/c/leak-canary.txt"), "a\n")?;
    let stamp = commit_all(root)?;
    fs::create_dir_all(root.join("abyme/elements"))?;
    fs::write(
        root.join("abyme/elements/changed.md"),
        format!(
            "---\nid: changed\ntype: t\nname: Billing\nsummary: billing jobs\nanchor: crates/c\n\
             verified: \"{stamp}\"\nverified_by: ai\n---\n"
        ),
    )?;
    fs::write(
        root.join("abyme/elements/calm.md"),
        "---\nid: calm\ntype: t\nname: Calm\nsummary: billing reports\n---\n",
    )?;
    fs::write(
        root.join("abyme/elements/hinted.md"),
        format!(
            "---\nid: hinted\ntype: t\nname: Hinted\nsummary: billing notes\n\
             verified: \"{stamp}\"\nverified_by: ai\nrelations:\n  - {{kind: uses, to: changed}}\n---\n"
        ),
    )?;
    fs::write(root.join("crates/c/leak-canary.txt"), "b\n")?;
    let client = connect(setup.server()?).await?;

    let found = call(&client, "search", json!({"query": "billing"})).await?;

    let reply = text(&found);
    let line = |id: &str| {
        reply
            .lines()
            .find(|line| line.starts_with(&format!("{id} ")))
            .unwrap_or_default()
            .to_owned()
    };
    assert!(line("changed").ends_with("⚠ task"), "{reply}");
    assert!(line("hinted").ends_with("⚠ hint"), "{reply}");
    assert!(line("calm").starts_with("calm "), "{reply}");
    assert!(!line("calm").contains('⚠'), "{reply}");
    assert!(!reply.contains("leak-canary"), "{reply}");
    assert!(!reply.contains("Review flags unavailable"), "{reply}");
    client.cancel().await?;
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn in_a_project_without_elements_no_element_matches() -> TestResult {
    let setup = Setup::new(true, 0, "")?;
    let client = connect(setup.server()?).await?;

    let found = call(&client, "search", json!({"query": "anything"})).await?;

    assert_eq!(found.is_error, Some(false), "{found:?}");
    assert_eq!(results(&found)?, NO_MATCH);
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

#[tokio::test(flavor = "multi_thread")]
async fn prepares_and_searches_with_the_model_the_project_file_names() -> TestResult {
    let setup = Setup::new(true, 0, "")?;
    setup.choose_model(MULTILINGUAL)?;
    let server = setup.server()?;

    server.prefetch().await?;
    let client = connect(server).await?;
    create(&client, &[("Mailer", "sends newsletters")]).await?;
    let found = call(&client, "search", json!({"query": "newsletters"})).await?;

    assert!(results(&found)?.starts_with("mailer  "), "{found:?}");
    assert_eq!(setup.loaded(), [MULTILINGUAL]);
    assert_eq!(setup.embedded_by("bge-small-en-v1.5"), 0);
    client.cancel().await?;
    Ok(())
}

/// Each model keeps its own index: the second model embeds every element
/// itself, and back on the first, only the query is embedded.
#[tokio::test(flavor = "multi_thread")]
async fn a_changed_model_takes_effect_at_the_next_search_with_its_own_index() -> TestResult {
    let setup = Setup::new(true, 0, "")?;
    let client = connect(setup.server()?).await?;
    create(
        &client,
        &[
            ("Mailer", "sends newsletters"),
            ("Payment Service", "charges cards"),
            ("Inventory", "counts stock"),
        ],
    )
    .await?;

    call(&client, "search", json!({"query": "newsletters"})).await?;
    setup.choose_model(MULTILINGUAL)?;
    let changed = call(&client, "search", json!({"query": "newsletters"})).await?;

    assert!(results(&changed)?.starts_with("mailer  "), "{changed:?}");
    assert_eq!(setup.loaded(), ["bge-small-en-v1.5", MULTILINGUAL]);
    assert_eq!(setup.embedded_by(MULTILINGUAL), 3 + 1);
    let tables = setup.table_dirs()?;
    assert_eq!(tables.len(), 2, "{tables:?}");
    assert!(tables[0].starts_with("bge-small-en-v1.5-"), "{tables:?}");
    assert!(
        tables[1].starts_with("paraphrase-multilingual-minilm-l12-v2-"),
        "{tables:?}"
    );

    let embedded_by_bge = setup.embedded_by("bge-small-en-v1.5");
    setup.choose_model("bge-small-en-v1.5")?;
    call(&client, "search", json!({"query": "newsletters"})).await?;

    assert_eq!(
        setup.loaded(),
        ["bge-small-en-v1.5", MULTILINGUAL, "bge-small-en-v1.5"]
    );
    assert_eq!(setup.embedded_by("bge-small-en-v1.5"), embedded_by_bge + 1);
    client.cancel().await?;
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn an_unknown_model_fails_every_call_and_loads_nothing() -> TestResult {
    let setup = Setup::new(true, 0, "")?;
    setup.choose_model("../x")?;
    let server = setup.server()?;

    server.prefetch().await?;
    let client = connect(server).await?;
    let searched = call(&client, "search", json!({"query": "anything"})).await?;
    let read = call(&client, "read_board", json!({"board": "main"})).await?;

    for result in [searched, read] {
        assert_eq!(result.is_error, Some(true), "{result:?}");
        assert!(
            text(&result).starts_with("abyme/abyme.toml: "),
            "{result:?}"
        );
    }
    assert_eq!(setup.loads(), 0);
    assert!(setup.cache_is_empty()?);
    client.cancel().await?;
    Ok(())
}
