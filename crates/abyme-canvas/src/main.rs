//! The Abyme canvas: a desktop app that shows the boards of the Abyme
//! project it starts in and keeps them live.
//!
//! The webview reaches the Rust side only through the commands in `app` and
//! listens to one event, `project-changed`, after which it asks again for
//! what it shows. The watcher in `watcher` raises that event when the
//! project's files change; `projection` computes what a board shows, and
//! `pages` keeps the window on the canvas's own pages.

mod app;
#[cfg(test)]
mod bindings;
mod boards;
mod pages;
mod projection;
mod watcher;

use std::env;
use std::process::ExitCode;

fn main() -> ExitCode {
    let start = match env::current_dir() {
        Ok(start) => start,
        Err(error) => {
            eprintln!("abyme-canvas: the current directory cannot be read: {error}");
            return ExitCode::FAILURE;
        }
    };
    match app::configure(tauri::Builder::default(), start).run(tauri::generate_context!()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("abyme-canvas: {error}");
            ExitCode::FAILURE
        }
    }
}
