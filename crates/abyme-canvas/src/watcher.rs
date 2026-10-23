//! Watches the project's files and signals when they change.
//!
//! The files arrive through clones and pull requests, so the watcher trusts
//! nothing in them: it watches only the fixed directories in [`WATCHED`],
//! each on its own and only while it is a real directory, follows no
//! symbolic link, and never acts on the paths an event names. Reading a file
//! raises no signal, so the reads that answer a signal cannot raise the next
//! one. A burst of changes becomes one signal once the files have been quiet
//! for [`QUIET`].

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError};
use std::sync::{Arc, Mutex, Weak};
use std::thread;
use std::time::{Duration, Instant};

use notify::{Config, Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher as _};

#[cfg(test)]
mod tests;

/// How long the files must stay unchanged before a burst of changes
/// becomes one signal.
pub const QUIET: Duration = Duration::from_millis(100);

/// The directories watched, relative to the project's root. `.abyme/`,
/// which holds the search cache and the lock, is not among them, and
/// `abyme/elements/` and `abyme/boards/` appear only with the first write.
const WATCHED: [&str; 3] = ["abyme", "abyme/elements", "abyme/boards"];

/// The events a watcher delivers.
type Events = Receiver<notify::Result<Event>>;

/// A running watcher; it stops when this is dropped.
pub struct Watching {
    _watcher: Arc<Mutex<RecommendedWatcher>>,
}

/// Starts watching the project whose root is `root` and calls `on_change`
/// on a thread of its own after each burst of changes.
///
/// # Errors
///
/// Returns notify's error when the system's watcher cannot be created.
pub fn watch(root: &Path, on_change: impl Fn() + Send + 'static) -> notify::Result<Watching> {
    let (sender, events) = mpsc::channel();
    let watcher = RecommendedWatcher::new(sender, Config::default().with_follow_symlinks(false))?;
    let watcher = Arc::new(Mutex::new(watcher));
    let directories: Vec<PathBuf> = WATCHED.iter().map(|relative| root.join(relative)).collect();
    watch_directories(&watcher, &directories);
    let weak = Arc::downgrade(&watcher);
    thread::spawn(move || signal_changes(&events, &weak, &directories, &on_change));
    Ok(Watching { _watcher: watcher })
}

/// Turns each burst of changes into one call of `on_change`, after watching
/// the directories that exist by then, so that the reads `on_change` causes
/// see them watched. Ends when the watcher is dropped.
fn signal_changes(
    events: &Events,
    watcher: &Weak<Mutex<RecommendedWatcher>>,
    directories: &[PathBuf],
    on_change: &impl Fn(),
) {
    while let Ok(event) = events.recv() {
        if !raises_signal(&event) {
            continue;
        }
        if !wait_until_quiet(events) {
            return;
        }
        let Some(watcher) = watcher.upgrade() else {
            return;
        };
        watch_directories(&watcher, directories);
        on_change();
    }
}

/// Waits until no change has arrived for [`QUIET`]. Returns false when the
/// watcher was dropped meanwhile.
fn wait_until_quiet(events: &Events) -> bool {
    let mut deadline = Instant::now() + QUIET;
    loop {
        match events.recv_timeout(deadline.saturating_duration_since(Instant::now())) {
            Ok(event) => {
                if raises_signal(&event) {
                    deadline = Instant::now() + QUIET;
                }
            }
            Err(RecvTimeoutError::Timeout) => return true,
            Err(RecvTimeoutError::Disconnected) => return false,
        }
    }
}

/// Whether `event` raises a signal. Every change does, and so does an error,
/// since events may have been lost; reading a file or a directory does not.
const fn raises_signal(event: &notify::Result<Event>) -> bool {
    !matches!(
        event,
        Ok(Event {
            kind: EventKind::Access(_),
            ..
        })
    )
}

/// Watches each of `directories` that is a real directory now, on its own.
/// A directory watched already stays watched; a missing one and a symbolic
/// link are skipped.
fn watch_directories(watcher: &Mutex<RecommendedWatcher>, directories: &[PathBuf]) {
    let Ok(mut watcher) = watcher.lock() else {
        return;
    };
    for directory in directories
        .iter()
        .filter(|directory| is_real_directory(directory))
    {
        // A directory that vanishes after the check stays unwatched until the
        // next burst of changes watches it again.
        let _ = watcher.watch(directory, RecursiveMode::NonRecursive);
    }
}

/// Whether `path` is a directory and not a symbolic link to one.
fn is_real_directory(path: &Path) -> bool {
    fs::symlink_metadata(path).is_ok_and(|metadata| metadata.is_dir())
}
