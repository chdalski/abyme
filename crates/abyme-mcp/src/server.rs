//! The MCP tools `apply`, `read_board`, `read_element`, `search`, `status`,
//! and `verify`, on the project that the server's working directory lies in.
//!
//! Arguments come from an AI, so each tool takes them as a raw JSON object
//! and reads them itself: a request that does not fit becomes a tool error
//! with an escaped message, which the AI can act on. The synchronous core
//! runs on a blocking thread, and the project is found again on every call.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use abyme_core::{
    Created, Geometry, LocalKey, Operations, Project, Review, Snapshot, Unavailable,
    element_detail, element_list, outline, status_report,
};
use abyme_index::{Index, MAX_QUERY_CHARS, spec};
use abyme_model::{BoardId, ElementId, Verifier, shown};
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

/// The largest reply `read_board`, `read_element`, `search`, or `status`
/// returns, in bytes. A larger one is an error rather than cut, so the AI never acts on
/// part of it.
const MAX_REPLY_BYTES: usize = 1024 * 1024;

/// How many elements `search` returns at most.
const SEARCH_RESULTS: usize = 10;

/// How a reply begins when the review flags are unavailable; the reason
/// follows.
const UNAVAILABLE: &str = "Review flags unavailable: ";

/// What `search` answers when no element matches.
const NO_MATCH: &str = "No element matches the query.";

/// What the server tells the AI about itself when a session starts.
const INSTRUCTIONS: &str = "Abyme keeps project knowledge on boards stored in this repository. \
    Call status at the start of a session to see which elements may no longer match the code. \
    Call search with a query to find elements by the words they contain or by what they are \
    about, read_board with a board ID to read the board as an outline, read_element with an \
    element ID to read one element in detail with its relations and description, and apply \
    to create, change, and delete elements, create boards, place elements on boards, move them, \
    and create and delete relations between elements. Elements marked ⚠ task or ⚠ hint may no \
    longer match the code: read them and what changed, update them with apply, and only then call \
    verify with the element's ID to stamp it as checked against the commit HEAD names in the \
    repository its anchor lies in. verify refuses while files under the element's anchor are not \
    committed; ask the user to commit them first.";

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

/// The arguments of `verify`.
#[derive(Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
struct VerifyArguments {
    /// The ID of the element you checked against the code.
    element: ElementId,
}

/// The arguments of `status`: none.
#[derive(Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
struct StatusArguments {}

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
            create_board takes a key, a title, and optionally a summary and an owner, the element whose \
            inside (inner board) the board is; update_board takes board and any of title, summary (null \
            removes it), and owner (null removes it), and changes only the fields it names; an element owns \
            at most one board, and no board may lie inside itself (the board above a board is its owner's \
            home). \
            create_element takes a key, type_ref, name, and optionally \
            summary, body, and anchor; place_element gives an element its home on a board; move_element takes an \
            element, the board it lies on, and a new x and y from -1000000 to 1000000, and keeps its size; \
            update_element takes element and any of name, type_ref, summary (null removes it), body, and \
            anchor (null removes it), and changes only the fields it names; the ID stays. An anchor is the \
            path relative to the project root that the element stands for, covering everything below it, \
            such as crates/payment-service: no leading or trailing /, no . or .. segment, no backslash or \
            colon, and nothing in abyme/, .abyme/, or a .git folder; Abyme does not check that the path \
            exists. Every update_element, create_relation, and delete_relation removes the verification \
            stamp of the element it changes, which no request can set. Text is stored exactly as sent: \
            send only the fields you change, since the \
            escapes read_board and read_element show are not decoded; \
            delete_element takes element and removes its file, its placement on every board, and its own \
            relations, and is refused while other elements still relate to it (delete those relations \
            first) or while it owns a board (give that board another owner or none first); it also clears the placements of an element whose file is missing; \
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
            title, then one line per element with its ID, type, name, and summary, and last ⚠ task or \
            ⚠ hint when the element has review flags (read_element names them), then, after a blank line, \
            one line per relation between two elements on the board, as source → target, kind, and label. \
            Text from the repository, relation labels included, is escaped onto one line. With coordinates \
            true, each element line also shows the element's position and size right after its ID, as \
            @x,y WIDTHxHEIGHT. When Git cannot be asked, the reply starts with a line Review flags \
            unavailable: and the reason, and no line is marked.",
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
            let (project, snapshot) = read_project(&start)?;
            let review = project.review(&snapshot);
            let geometry = if arguments.coordinates {
                Geometry::Shown
            } else {
                Geometry::Hidden
            };
            let text = outline(&snapshot, &arguments.board, geometry, review.as_ref().ok())
                .map_err(|error| error.to_string())?;
            bounded(
                &format!("the outline of board {}", arguments.board),
                format!("{}{text}", unavailable_line(&review)),
            )
        })
        .await
    }

    #[tool(
        description = "Returns one element of the Abyme project in detail: its ID and name, type, anchor \
            (or none), verification stamp as the commit it was last verified against and by ai or person \
            (or none), then Review: with task, hint, or none and one indented line per review flag, such \
            as files changed under the anchor since the stamp (at most 10 named), or a relation of kind \
            depends_on or describes (a task) or uses or concerns (a hint) where what the target's anchor \
            stands for may have changed since the element's stamp, summary, the boards that place it, its outgoing relations (a \
            target without a file is marked ⚠ missing), the relations of other elements to it, and last its description. \
            The single-line values are escaped onto one line as in read_board. The description keeps its \
            line breaks and tabs; other control and invisible characters in it appear as \\u{...} \
            escapes. When Git cannot be asked, the reply starts with a line Review flags unavailable: and \
            the reason, and has no Review: lines.",
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
            let (project, snapshot) = read_project(&start)?;
            let review = project.review(&snapshot);
            let detail = element_detail(&snapshot, &arguments.element, review.as_ref().ok())
                .map_err(|error| error.to_string())?;
            let text = format!("{}{detail}", unavailable_line(&review));
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
            with its ID, type, name, and summary, and the ⚠ task or ⚠ hint marker, as in a board outline. \
            Text from the repository is escaped onto one line. When Git cannot be asked, the reply starts \
            with a line Review flags unavailable: and the reason, and no line is marked. The first search \
            after the server starts, and the first after the project file names another embedding model, \
            may wait until that model is downloaded.",
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
            let (project, snapshot) = read_project(&start)?;
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
            let review = project.review(&snapshot);
            let note = unavailable_line(&review);
            if found.is_empty() {
                return Ok(format!("{note}{NO_MATCH}"));
            }
            bounded(
                "the search result",
                format!(
                    "{note}{}",
                    element_list(&snapshot, &found, review.as_ref().ok())
                ),
            )
        })
        .await
    }

    #[tool(
        description = "Returns the open review flags of the Abyme project: a line counting the elements \
            to review by their highest priority, then one line per element as in search, marked ⚠ task or \
            ⚠ hint, those with a task first, at most 50 followed by how many more there are; or No open \
            review flags. read_element names each element's flags. When Git cannot be asked, it answers \
            with an error that starts with Review flags unavailable: and names the reason.",
        input_schema = schema_for_type::<StatusArguments>()
    )]
    async fn status(
        &self,
        Parameters(arguments): Parameters<JsonObject>,
    ) -> Result<CallToolResult, ErrorData> {
        if let Err(result) = parsed::<StatusArguments>(arguments) {
            return Ok(result);
        }
        let start = self.start.clone();
        on_core(move || {
            let (project, snapshot) = read_project(&start)?;
            let review = project
                .review(&snapshot)
                .map_err(|reason| format!("{UNAVAILABLE}{reason}"))?;
            bounded("the status", status_report(&snapshot, &review))
        })
        .await
    }

    #[tool(
        description = "Stamps one element of the Abyme project as verified by ai against the commit HEAD \
            names in the repository its anchor lies in, a repository nested in the project such as a \
            submodule or the project's own, after you have checked the element against the code and \
            updated it: its review flags for changed files go away until files under its anchor change \
            again, and flags its relations pass on are measured from the new stamp. Only the stamp \
            changes. It refuses an element without a file, a repository without a commit, and an element \
            with files under its anchor that differ from HEAD or are untracked; those must be committed \
            first. It also refuses an element whose anchor lies in a nested repository that Abyme cannot \
            read, and any element when Git cannot be asked. Returns the commit.",
        input_schema = schema_for_type::<VerifyArguments>()
    )]
    async fn verify(
        &self,
        Parameters(arguments): Parameters<JsonObject>,
    ) -> Result<CallToolResult, ErrorData> {
        let arguments: VerifyArguments = match parsed(arguments) {
            Ok(arguments) => arguments,
            Err(result) => return Ok(result),
        };
        let start = self.start.clone();
        on_core(move || {
            let project = Project::discover(&start).map_err(|error| error.to_string())?;
            let commit = project
                .verify(&arguments.element, Verifier::Ai)
                .map_err(|error| error.to_string())?;
            Ok(format!(
                "Verified {} against {commit} by ai.",
                arguments.element
            ))
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

/// The line that starts a reply when the review flags are unavailable, or
/// nothing.
fn unavailable_line(review: &Result<Review, Unavailable>) -> String {
    review
        .as_ref()
        .err()
        .map(|reason| format!("{UNAVAILABLE}{reason}\n"))
        .unwrap_or_default()
}

/// The project the directory `start` lies in, with all of its elements and
/// boards as they are now.
fn read_project(start: &Path) -> Result<(Project, Snapshot), String> {
    let project = Project::discover(start).map_err(|error| error.to_string())?;
    let snapshot = project.snapshot().map_err(|error| error.to_string())?;
    Ok((project, snapshot))
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
