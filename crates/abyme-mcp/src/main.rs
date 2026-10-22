//! The Abyme MCP server.
//!
//! `abyme-mcp init` makes the current directory an Abyme project. Without
//! arguments, `abyme-mcp` serves the project that the current directory
//! lies in to an AI over MCP on standard input and output; standard output
//! then carries nothing but protocol messages, and errors go to standard
//! error.

mod server;

use std::env;
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use abyme_core::{Init, Project};
use abyme_model::shown;
use rmcp::ServiceExt as _;
use rmcp::transport::stdio;

use crate::server::AbymeServer;

const USAGE: &str = "usage: abyme-mcp [init]
  abyme-mcp        serve the Abyme project of the current directory over MCP on standard input and output
  abyme-mcp init   make the current directory an Abyme project";

/// How many characters of a path the output shows.
const SHOWN_PATH_LIMIT: usize = 200;

/// The exit code for a command line this program does not accept.
const USAGE_ERROR: u8 = 2;

fn main() -> ExitCode {
    let arguments: Vec<OsString> = env::args_os().skip(1).collect();
    match arguments.as_slice() {
        [] => serve(),
        [command] if command == "init" => init(),
        _ => {
            eprintln!("{USAGE}");
            ExitCode::from(USAGE_ERROR)
        }
    }
}

/// Makes the current directory an Abyme project, unless it lies in one, and
/// names the project's root.
fn init() -> ExitCode {
    let result = current_dir()
        .and_then(|directory| Project::init(&directory).map_err(|error| error.to_string()));
    match result {
        Ok(Init::Created(project)) => {
            println!("Created an Abyme project in {}", shown_path(project.root()));
            ExitCode::SUCCESS
        }
        Ok(Init::Existing(project)) => {
            println!(
                "This directory lies in the Abyme project in {}; nothing changed",
                shown_path(project.root())
            );
            ExitCode::SUCCESS
        }
        Err(message) => {
            eprintln!("abyme-mcp init: {message}");
            ExitCode::FAILURE
        }
    }
}

/// Serves the project of the current directory until the client closes
/// standard input.
fn serve() -> ExitCode {
    let result = current_dir().and_then(|start| {
        tokio::runtime::Runtime::new()
            .map_err(|error| format!("the async runtime cannot start: {error}"))?
            .block_on(run(start))
    });
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("abyme-mcp: {message}");
            ExitCode::FAILURE
        }
    }
}

async fn run(start: PathBuf) -> Result<(), String> {
    let service = AbymeServer::new(start)
        .serve(stdio())
        .await
        .map_err(|error| format!("the MCP session cannot start: {error}"))?;
    service
        .waiting()
        .await
        .map_err(|error| format!("the MCP session failed: {error}"))?;
    Ok(())
}

fn current_dir() -> Result<PathBuf, String> {
    env::current_dir().map_err(|error| format!("the current directory cannot be read: {error}"))
}

/// Shows a path escaped, so that no character in a directory name can steer
/// the terminal.
fn shown_path(path: &Path) -> String {
    shown(&path.to_string_lossy(), SHOWN_PATH_LIMIT)
}
