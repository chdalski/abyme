//! The canvas's side of the IPC boundary: its one command, its one event,
//! and the watcher that raises the event.
//!
//! No command takes a path: every command works on the project that the
//! directory the canvas started in lies in, found anew by the core on each
//! call, as the MCP server does.

use std::path::{Path, PathBuf};

use abyme_core::Project;
use tauri::{Builder, Emitter as _, Manager as _, Runtime, State};

use crate::boards::{BoardEntry, board_list};
use crate::watcher;

#[cfg(test)]
mod tests;

/// The event the canvas emits when the project's files change. It carries
/// no payload; the UI asks again for what it shows.
pub const PROJECT_CHANGED: &str = "project-changed";

/// The directory the canvas started in, captured once at launch.
struct Start(PathBuf);

/// Registers the canvas's command on `builder`, and starts watching the
/// project that `start` lies in once the app is set up. Outside a project,
/// nothing is watched.
pub fn configure<R: Runtime>(builder: Builder<R>, start: PathBuf) -> Builder<R> {
    builder
        .manage(Start(start))
        .invoke_handler(tauri::generate_handler![list_boards])
        .setup(|app| {
            let Ok(project) = Project::discover(&app.state::<Start>().0) else {
                return Ok(());
            };
            let handle = app.handle().clone();
            let emit = move || {
                // The event carries no payload, so emitting it cannot fail
                // to serialize; a closed window simply misses it.
                let _ = handle.emit(PROJECT_CHANGED, ());
            };
            match watcher::watch(project.root(), emit) {
                Ok(watching) => {
                    app.manage(watching);
                }
                Err(error) => eprintln!(
                    "abyme-canvas: the project's files cannot be watched, so changes need a restart: {error}"
                ),
            }
            Ok(())
        })
}

/// The boards of the project, in board-ID order, or the core's message
/// when there is no project or it cannot be read.
#[tauri::command]
async fn list_boards(start: State<'_, Start>) -> Result<Vec<BoardEntry>, String> {
    let start = start.0.clone();
    tauri::async_runtime::spawn_blocking(move || boards(&start))
        .await
        .map_err(|_| "the Abyme core failed unexpectedly".to_owned())?
}

/// Reads the boards of the project that `start` lies in.
fn boards(start: &Path) -> Result<Vec<BoardEntry>, String> {
    let project = Project::discover(start).map_err(|error| error.to_string())?;
    let snapshot = project.snapshot().map_err(|error| error.to_string())?;
    Ok(board_list(&snapshot))
}
