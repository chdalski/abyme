//! The MCP tools `apply` and `read_board`, on the project that the server's
//! working directory lies in.
//!
//! Arguments come from an AI, so each tool takes them as a raw JSON object
//! and reads them itself: a request that does not fit becomes a tool error
//! with an escaped message, which the AI can act on. The synchronous core
//! runs on a blocking thread, and the project is found again on every call.

use std::path::PathBuf;

use abyme_core::{Created, LocalKey, Operations, Project, outline};
use abyme_model::{BoardId, shown};
use rmcp::handler::server::common::schema_for_type;
use rmcp::handler::server::router::tool::ToolRouter;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{
    CallToolResult, ContentBlock, Implementation, JsonObject, ServerCapabilities, ServerConfig,
};
use rmcp::{ErrorData, ServerHandler, tool, tool_handler, tool_router};
use serde::Deserialize;
use serde::de::DeserializeOwned;

/// How many characters of the description of rejected arguments a tool
/// error shows.
const SHOWN_LIMIT: usize = 300;

/// The largest outline `read_board` returns, in bytes. A larger one is an
/// error rather than cut, so the AI never acts on part of a board.
const MAX_OUTLINE_BYTES: usize = 1024 * 1024;

/// What the server tells the AI about itself when a session starts.
const INSTRUCTIONS: &str = "Abyme keeps project knowledge on boards stored in this repository. \
    Call read_board with a board ID to read the board as an outline, and apply to create boards \
    and elements and place elements on boards.";

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
}

/// The MCP server of one project.
#[derive(Clone)]
pub struct AbymeServer {
    /// The directory the server runs in; the project is found from here.
    start: PathBuf,
    tool_router: ToolRouter<Self>,
}

#[tool_router]
impl AbymeServer {
    /// A server for the project that `start` lies in.
    pub fn new(start: PathBuf) -> Self {
        Self {
            start,
            tool_router: Self::tool_router(),
        }
    }

    #[tool(
        description = "Applies operations to the Abyme project in one transaction: either all of them or none. \
            create_board takes a key and a title; create_element takes a key, type_ref, name, and optionally \
            summary and body; place_element gives an element its home on a board. A reference is \
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
            title, then one line per element with its ID, type, name, and summary. Text from the repository \
            is escaped onto one line.",
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
            let text = outline(&snapshot, &arguments.board).map_err(|error| error.to_string())?;
            bounded(&arguments.board, text)
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

/// The outline, or an error when it is larger than [`MAX_OUTLINE_BYTES`].
fn bounded(board: &BoardId, text: String) -> Result<String, String> {
    if text.len() > MAX_OUTLINE_BYTES {
        Err(format!(
            "the outline of board {board} has {} bytes, more than the {MAX_OUTLINE_BYTES} a reply may hold",
            text.len()
        ))
    } else {
        Ok(text)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rmcp::handler::server::common::schema_for_input;

    #[test]
    fn rejects_an_outline_over_the_limit_instead_of_cutting_it() {
        let board: BoardId = "big".parse().unwrap();

        let error = bounded(&board, "x".repeat(MAX_OUTLINE_BYTES + 1)).unwrap_err();

        assert!(error.contains("board big"), "{error}");
        assert!(error.contains(&MAX_OUTLINE_BYTES.to_string()), "{error}");
    }

    #[test]
    fn returns_an_outline_at_the_limit_whole() {
        let board: BoardId = "big".parse().unwrap();
        let text = "x".repeat(MAX_OUTLINE_BYTES);

        assert_eq!(bounded(&board, text.clone()), Ok(text));
    }

    #[test]
    fn advertises_object_schemas_as_mcp_requires() {
        assert!(schema_for_input::<ApplyArguments>().is_ok());
        assert!(schema_for_input::<ReadBoardArguments>().is_ok());
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
}
