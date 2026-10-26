//! The MCP tools `apply`, `read_board`, `read_element`, and `search`, on the
//! project that the server's working directory lies in.
//!
//! Arguments come from an AI, so each tool takes them as a raw JSON object
//! and reads them itself: a request that does not fit becomes a tool error
//! with an escaped message, which the AI can act on. The synchronous core
//! runs on a blocking thread, and the project is found again on every call.

use std::path::PathBuf;
use std::sync::Arc;

use abyme_core::{
    Created, Geometry, LocalKey, Operations, Project, element_detail, element_list, outline,
};
use abyme_index::{Index, MAX_QUERY_CHARS, spec};
use abyme_model::{BoardId, ElementId, shown};
use rmcp::handler::server::common::schema_for_type;
use rmcp::handler::server::router::tool::ToolRouter;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{
    CallToolResult, ContentBlock, Implementation, JsonObject, ServerCapabilities, ServerConfig,
};
use rmcp::{ErrorData, ServerHandler, tool, tool_handler, tool_router};
use serde::Deserialize;
use serde::de::DeserializeOwned;
use tokio::runtime::Handle;
use tokio::task::JoinHandle;

use crate::models::Models;

#[cfg(test)]
mod tests;

/// How many characters of the description of rejected arguments a tool
/// error shows.
const SHOWN_LIMIT: usize = 300;

/// The largest reply `read_board`, `read_element`, or `search` returns, in
/// bytes. A larger one is an error rather than cut, so the AI never acts on
/// part of it.
const MAX_REPLY_BYTES: usize = 1024 * 1024;

/// How many elements `search` returns at most.
const SEARCH_RESULTS: usize = 10;

/// What `search` answers when no element matches.
const NO_MATCH: &str = "No element matches the query.";

/// What the server tells the AI about itself when a session starts.
const INSTRUCTIONS: &str = "Abyme keeps project knowledge on boards stored in this repository. \
    Call search with a query to find elements by the words they contain or by what they are \
    about, read_board with a board ID to read the board as an outline, read_element with an \
    element ID to read one element in detail with its relations and description, and apply \
    to create boards and elements, place elements on boards, move them, and create and delete \
    relations between elements.";

/// The arguments of `apply`.
#[derive(Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
struct ApplyArguments {
    /// The operations, applied in order: all of them or none.
    operations: Operations,
}

/// The arguments of `read_board`.
#[derive(Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
struct ReadBoardArguments {
    /// The ID of the board to read.
    board: BoardId,
    /// Whether each line also shows the element's position and size on the
    /// board.
    #[serde(default)]
    coordinates: bool,
}

/// The arguments of `read_element`.
#[derive(Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
struct ReadElementArguments {
    /// The ID of the element to read.
    element: ElementId,
}

/// The arguments of `search`.
#[derive(Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
struct SearchArguments {
    /// What to look for: words the elements contain, or what they are about.
    query: String,
}

/// The MCP server of one project.
#[derive(Clone)]
pub struct AbymeServer {
    /// The directory the server runs in; the project is found from here.
    start: PathBuf,
    /// The embedding model that `search` uses.
    models: Arc<Models>,
    tool_router: ToolRouter<Self>,
}

#[tool_router]
impl AbymeServer {
    /// A server for the project that `start` lies in, whose `search` uses
    /// `models`.
    pub fn new(start: PathBuf, models: Arc<Models>) -> Self {
        Self {
            start,
            models,
            tool_router: Self::tool_router(),
        }
    }

    /// Prepares the embedding model of the project the server starts in,
    /// in the background, so that the first `search` waits less or not at
    /// all. Without a valid project it does nothing; `search` loads the
    /// model once there is one. A failure is one line on standard error,
    /// and `search` tries again.
    pub fn prefetch(&self) -> JoinHandle<()> {
        let (start, models) = (self.start.clone(), Arc::clone(&self.models));
        tokio::task::spawn_blocking(move || {
            let Ok(project) = Project::discover(&start) else {
                return;
            };
            if let Err(message) = models.with(spec(project.embedding_model()), |_, _| Ok(())) {
                eprintln!(
                    "abyme-mcp: the embedding model is not ready, search tries again: {}",
                    shown(&message, SHOWN_LIMIT)
                );
            }
        })
    }

    #[tool(
        description = "Applies operations to the Abyme project in one transaction: either all of them or none. \
            create_board takes a key and a title; create_element takes a key, type_ref, name, and optionally \
            summary and body; place_element gives an element its home on a board; move_element takes an \
            element, the board it lies on, and a new x and y from -1000000 to 1000000, and keeps its size; \
            create_relation takes from, kind, to, and optionally a label, and adds a relation from one \
            element to another; delete_relation takes from, kind, and to, and removes that relation, also \
            when the target is gone. A kind is one of concerns, depends_on, describes (from a document to \
            what it describes), see_also, and uses; an element has at most one relation of a kind to a \
            target. A reference is \
            {\"new\": \"<key>\"} for something an earlier operation of the same request creates, or \
            {\"existing\": \"<id>\"}. Returns the ID each key received.",
        input_schema = schema_for_type::<ApplyArguments>()
    )]
    async fn apply(
        &self,
        Parameters(arguments): Parameters<JsonObject>,
    ) -> Result<CallToolResult, ErrorData> {
        let arguments: ApplyArguments = match parsed(arguments) {
            Ok(arguments) => arguments,
            Err(result) => return Ok(result),
        };
        let start = self.start.clone();
        let operations = arguments.operations.into_vec();
        on_core(move || {
            let project = Project::discover(&start).map_err(|error| error.to_string())?;
            let created = project
                .apply(&operations)
                .map_err(|error| error.to_string())?;
            Ok(report(&created))
        })
        .await
    }

    #[tool(
        description = "Returns a board of the Abyme project as an outline: a header with the board's ID and \
            title, then one line per element with its ID, type, name, and summary, then, after a blank line, \
            one line per relation between two elements on the board, as source → target, kind, and label. \
            Text from the repository, relation labels included, is escaped onto one line. With coordinates \
            true, each element line also shows the element's position and size right after its ID, as \
            @x,y WIDTHxHEIGHT.",
        input_schema = schema_for_type::<ReadBoardArguments>()
    )]
    async fn read_board(
        &self,
        Parameters(arguments): Parameters<JsonObject>,
    ) -> Result<CallToolResult, ErrorData> {
        let arguments: ReadBoardArguments = match parsed(arguments) {
            Ok(arguments) => arguments,
            Err(result) => return Ok(result),
        };
        let start = self.start.clone();
        on_core(move || {
            let project = Project::discover(&start).map_err(|error| error.to_string())?;
            let snapshot = project.snapshot().map_err(|error| error.to_string())?;
            let geometry = if arguments.coordinates {
                Geometry::Shown
            } else {
                Geometry::Hidden
            };
            let text = outline(&snapshot, &arguments.board, geometry)
                .map_err(|error| error.to_string())?;
            bounded(&format!("the outline of board {}", arguments.board), text)
        })
        .await
    }

    #[tool(
        description = "Returns one element of the Abyme project in detail: its ID and name, type, summary, \
            the boards that place it, its outgoing relations (a target without a file is marked ⚠ missing), \
            the relations of other elements to it, and last its description. The single-line values are \
            escaped onto one line as in read_board. The description keeps its line breaks and tabs; other \
            control and invisible characters in it appear as \\u{...} escapes.",
        input_schema = schema_for_type::<ReadElementArguments>()
    )]
    async fn read_element(
        &self,
        Parameters(arguments): Parameters<JsonObject>,
    ) -> Result<CallToolResult, ErrorData> {
        let arguments: ReadElementArguments = match parsed(arguments) {
            Ok(arguments) => arguments,
            Err(result) => return Ok(result),
        };
        let start = self.start.clone();
        on_core(move || {
            let project = Project::discover(&start).map_err(|error| error.to_string())?;
            let snapshot = project.snapshot().map_err(|error| error.to_string())?;
            let text =
                element_detail(&snapshot, &arguments.element).map_err(|error| error.to_string())?;
            bounded(
                &format!("the detail of element {}", arguments.element),
                text,
            )
        })
        .await
    }

    #[tool(
        description = "Searches the elements of the Abyme project by the words they contain and by what \
            they are about, and returns the 10 that match the query best, best first: one line per element \
            with its ID, type, name, and summary, as in a board outline. Text from the repository is \
            escaped onto one line. The first search after the server starts, and the first after the \
            project file names another embedding model, may wait until that model is downloaded.",
        input_schema = schema_for_type::<SearchArguments>()
    )]
    async fn search(
        &self,
        Parameters(arguments): Parameters<JsonObject>,
    ) -> Result<CallToolResult, ErrorData> {
        let arguments: SearchArguments = match parsed(arguments) {
            Ok(arguments) => arguments,
            Err(result) => return Ok(result),
        };
        if let Err(message) = searchable(&arguments.query) {
            return Ok(CallToolResult::error(vec![ContentBlock::text(message)]));
        }
        let (start, models) = (self.start.clone(), Arc::clone(&self.models));
        let runtime = Handle::current();
        on_core(move || {
            let project = Project::discover(&start).map_err(|error| error.to_string())?;
            let snapshot = project.snapshot().map_err(|error| error.to_string())?;
            let spec = spec(project.embedding_model());
            let found = models
                .with(spec, |cache, embedder| {
                    let index = Index::new(cache.clone(), project.root(), spec);
                    let lock = index.lock().map_err(|error| error.to_string())?;
                    runtime
                        .block_on(async {
                            lock.refresh(&snapshot, embedder).await?;
                            lock.search(&snapshot, &arguments.query, embedder, SEARCH_RESULTS)
                                .await
                        })
                        .map_err(|error| error.to_string())
                })
                .map_err(|message| shown(&message, SHOWN_LIMIT))?;
            if found.is_empty() {
                return Ok(NO_MATCH.to_owned());
            }
            bounded("the search result", element_list(&snapshot, &found))
        })
        .await
    }
}

#[tool_handler(router = self.tool_router)]
#[expect(
    clippy::unused_async_trait_impl,
    reason = "the tool_handler macro writes trait methods as async fns that await nothing"
)]
impl ServerHandler for AbymeServer {
    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(Implementation::new(
                env!("CARGO_PKG_NAME"),
                env!("CARGO_PKG_VERSION"),
            ))
            .with_instructions(INSTRUCTIONS)
    }
}

/// Reads a tool's arguments, or returns the tool error that says what is
/// wrong with them, escaped and shortened.
fn parsed<T: DeserializeOwned>(arguments: JsonObject) -> Result<T, CallToolResult> {
    serde_json::from_value(serde_json::Value::Object(arguments)).map_err(|error| {
        CallToolResult::error(vec![ContentBlock::text(format!(
            "invalid arguments: {}",
            shown(&error.to_string(), SHOWN_LIMIT)
        ))])
    })
}

/// Runs work of the synchronous core on a blocking thread. Its error becomes
/// a tool error; a panic becomes a protocol error that shows nothing of it.
async fn on_core(
    work: impl FnOnce() -> Result<String, String> + Send + 'static,
) -> Result<CallToolResult, ErrorData> {
    match tokio::task::spawn_blocking(work).await {
        Ok(Ok(text)) => Ok(CallToolResult::success(vec![ContentBlock::text(text)])),
        Ok(Err(message)) => Ok(CallToolResult::error(vec![ContentBlock::text(message)])),
        Err(_) => Err(ErrorData::internal_error(
            "the Abyme core failed unexpectedly",
            None,
        )),
    }
}

/// The ID each key of a request received, one key per line.
fn report(created: &[(LocalKey, Created)]) -> String {
    if created.is_empty() {
        return "The request created nothing.".to_owned();
    }
    created
        .iter()
        .map(|(key, created)| match created {
            Created::Element(id) => format!("{key}: element {id}\n"),
            Created::Board(id) => format!("{key}: board {id}\n"),
        })
        .collect()
}

/// Whether `query` can be searched, or the tool error that says why not.
/// It is checked before anything is loaded, and the error never quotes it.
fn searchable(query: &str) -> Result<(), String> {
    if query.trim().is_empty() {
        return Err(
            "the query is empty: give words the elements contain or what they are about".to_owned(),
        );
    }
    let length = query.chars().count();
    if length > MAX_QUERY_CHARS {
        return Err(format!(
            "the query has {length} characters, more than the {MAX_QUERY_CHARS} a search takes"
        ));
    }
    Ok(())
}

/// The reply, or an error naming `what` when it is larger than
/// [`MAX_REPLY_BYTES`]; the error shows nothing of the reply.
fn bounded(what: &str, text: String) -> Result<String, String> {
    if text.len() > MAX_REPLY_BYTES {
        Err(format!(
            "{what} has {} bytes, more than the {MAX_REPLY_BYTES} a reply may hold",
            text.len()
        ))
    } else {
        Ok(text)
    }
}
