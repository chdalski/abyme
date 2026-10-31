//! The canvas's side of the IPC boundary: its commands, its one event, its
//! window, and the watcher that raises the event. The one command that
//! writes, `apply`, passes a request on only when `admission` admits it and
//! no move in it lies on a mirrored board.
//!
//! No command takes a path: every command works on the project that the
//! directory the canvas started in lies in, found anew by the core on each
//! call, as the MCP server does.

use std::error::Error;
use std::path::{Path, PathBuf};

use abyme_core::{Operations, Project, Snapshot};
use abyme_model::BoardId;
use tauri::webview::NewWindowResponse;
use tauri::{App, Builder, Emitter as _, Manager as _, Runtime, State, WebviewWindowBuilder};

use crate::admission::{admit, check_modes};
use crate::boards::{BoardEntry, board_list};
use crate::projection::{BoardView, project};
use crate::{pages, watcher};

#[cfg(test)]
mod tests;

/// The event the canvas emits when the project's files change. It carries
/// no payload; the UI asks again for what it shows.
pub const PROJECT_CHANGED: &str = "project-changed";

/// The directory the canvas started in, captured once at launch.
struct Start(PathBuf);

/// Registers the canvas's commands on `builder`. Once the app is set up, it
/// opens the window and starts watching the project that `start` lies in;
/// outside a project, nothing is watched.
pub fn configure<R: Runtime>(builder: Builder<R>, start: PathBuf) -> Builder<R> {
    builder
        .manage(Start(start))
        .invoke_handler(tauri::generate_handler![list_boards, open_board, apply])
        .setup(|app| {
            open_window(app)?;
            watch_project(app);
            Ok(())
        })
}

/// Opens the window that `tauri.conf.json` declares, limited to the
/// canvas's own pages and without windows of its own.
fn open_window<R: Runtime>(app: &App<R>) -> Result<(), Box<dyn Error>> {
    let config = app
        .config()
        .app
        .windows
        .first()
        .ok_or("tauri.conf.json declares no window")?;
    let dev_url = if tauri::is_dev() {
        app.config().build.dev_url.clone()
    } else {
        None
    };
    WebviewWindowBuilder::from_config(app, config)?
        .on_navigation(move |url| pages::is_own_page(url, dev_url.as_ref()))
        .on_new_window(|_, _| NewWindowResponse::Deny)
        .build()?;
    Ok(())
}

/// Starts the watcher of the project the canvas started in, if there is
/// one, and keeps it running as long as the app.
fn watch_project<R: Runtime>(app: &App<R>) {
    let Ok(project) = Project::discover(&app.state::<Start>().0) else {
        return;
    };
    let handle = app.handle().clone();
    let emit = move || {
        // The event carries no payload, so emitting it cannot fail to
        // serialize; a closed window simply misses it.
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
}

/// The boards of the project, in board-ID order, or the core's message
/// when there is no project or it cannot be read.
#[tauri::command]
async fn list_boards(start: State<'_, Start>) -> Result<Vec<BoardEntry>, String> {
    let start = start.0.clone();
    on_core(move || Ok(board_list(&snapshot(&start)?))).await
}

/// The projection of `board`, or the core's message when there is no
/// project, it cannot be read, or it has no such board.
#[tauri::command]
async fn open_board(start: State<'_, Start>, board: BoardId) -> Result<BoardView, String> {
    let start = start.0.clone();
    on_core(move || project(&snapshot(&start)?, &board).map_err(|error| error.to_string())).await
}

/// Applies `operations` to the project, all or none, once the canvas may
/// send every one of them and none moves an element on a mirrored board. The
/// first check comes before any access to the project, the second reads the
/// project the request goes to; a board that turns mirrored between it and
/// the write lets that one move through, which changes only that element's
/// position and pin.
#[tauri::command]
async fn apply(start: State<'_, Start>, operations: Operations) -> Result<(), String> {
    let operations = operations.into_vec();
    admit(&operations).map_err(|refused| refused.to_string())?;
    let start = start.0.clone();
    on_core(move || {
        let project = Project::discover(&start).map_err(|error| error.to_string())?;
        let snapshot = project.snapshot().map_err(|error| error.to_string())?;
        check_modes(&snapshot, &operations).map_err(|refused| refused.to_string())?;
        project
            .apply(&operations)
            .map(|_created| ())
            .map_err(|error| error.to_string())
    })
    .await
}

/// Runs work of the synchronous core on a blocking thread.
async fn on_core<T: Send + 'static>(
    work: impl FnOnce() -> Result<T, String> + Send + 'static,
) -> Result<T, String> {
    tauri::async_runtime::spawn_blocking(work)
        .await
        .map_err(|_| "the Abyme core failed unexpectedly".to_owned())?
}

/// Reads every element and board of the project that `start` lies in.
fn snapshot(start: &Path) -> Result<Snapshot, String> {
    let project = Project::discover(start).map_err(|error| error.to_string())?;
    project.snapshot().map_err(|error| error.to_string())
}
