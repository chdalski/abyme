//! Tests of the watcher: on real directories for what it watches, and on
//! events of its own making for how it turns a burst into one signal.

use super::*;
use std::os::unix::fs::symlink;
use std::sync::atomic::{AtomicBool, Ordering};

use abyme_core::{LocalKey, Operation, Project, Ref};
use notify::event::{AccessKind, CreateKind, RemoveKind};

/// How long a test waits for a signal that must come.
const SIGNAL_DEADLINE: Duration = Duration::from_secs(10);

/// How long a test waits before it concludes that no signal comes: well
/// beyond twice the quiet time that a burst must end with.
const SILENCE: Duration = Duration::from_millis(500);

/// Watches the project in `root` and returns the signals as a channel.
fn signals(root: &Path) -> (Watching, Receiver<()>) {
    let (sender, signals) = mpsc::channel();
    let watching = watch(root, move || {
        let _ = sender.send(());
    })
    .unwrap();
    (watching, signals)
}

fn new_project(root: &Path) -> Project {
    Project::init(root).unwrap();
    Project::discover(root).unwrap()
}

fn create_board(title: &str) -> Operation {
    Operation::CreateBoard {
        key: LocalKey::try_from("board".to_owned()).unwrap(),
        title: title.to_owned(),
    }
}

fn create_element(name: &str) -> Operation {
    Operation::CreateElement {
        key: LocalKey::try_from("element".to_owned()).unwrap(),
        type_ref: "c4/container".to_owned(),
        name: name.to_owned(),
        summary: None,
        body: String::new(),
        anchor: None,
    }
}

fn created() -> Event {
    Event::new(EventKind::Create(CreateKind::Any))
}

fn read() -> Event {
    Event::new(EventKind::Access(AccessKind::Any))
}

/// Runs the loop that turns `events` into signals, with a watcher that
/// watches nothing. Returns the watcher, the signals, and a channel that
/// reports the end of the loop.
fn signal_loop(events: Events) -> (Arc<Mutex<RecommendedWatcher>>, Receiver<()>, Receiver<()>) {
    let watcher =
        RecommendedWatcher::new(|_: notify::Result<Event>| {}, Config::default()).unwrap();
    let watcher = Arc::new(Mutex::new(watcher));
    let weak = Arc::downgrade(&watcher);
    let (signal_sender, signals) = mpsc::channel();
    let (end_sender, ended) = mpsc::channel();
    thread::spawn(move || {
        signal_changes(&events, &weak, &[], &|| {
            let _ = signal_sender.send(());
        });
        let _ = end_sender.send(());
    });
    (watcher, signals, ended)
}

#[test]
fn a_new_board_in_a_fresh_project_raises_a_signal() {
    let root = tempfile::tempdir().unwrap();
    let project = new_project(root.path());
    assert!(!root.path().join("abyme/boards").exists());
    let (_watching, signals) = signals(root.path());

    project.apply(&[create_board("Context")]).unwrap();

    assert_eq!(signals.recv_timeout(SIGNAL_DEADLINE), Ok(()));
}

#[test]
fn a_second_board_raises_a_signal_from_the_boards_directory() {
    let root = tempfile::tempdir().unwrap();
    let project = new_project(root.path());
    project.apply(&[create_board("Context")]).unwrap();
    let (_watching, signals) = signals(root.path());

    project.apply(&[create_board("Containers")]).unwrap();

    assert_eq!(signals.recv_timeout(SIGNAL_DEADLINE), Ok(()));
}

#[test]
fn a_board_after_the_first_in_a_fresh_project_raises_a_signal() {
    let root = tempfile::tempdir().unwrap();
    let project = new_project(root.path());
    let (_watching, signals) = signals(root.path());
    project.apply(&[create_board("Context")]).unwrap();
    assert_eq!(signals.recv_timeout(SIGNAL_DEADLINE), Ok(()));

    project.apply(&[create_board("Containers")]).unwrap();

    assert_eq!(signals.recv_timeout(SIGNAL_DEADLINE), Ok(()));
}

#[test]
fn an_element_after_the_first_in_a_fresh_project_raises_a_signal() {
    let root = tempfile::tempdir().unwrap();
    let project = new_project(root.path());
    let (_watching, signals) = signals(root.path());
    project.apply(&[create_element("Payment Service")]).unwrap();
    assert_eq!(signals.recv_timeout(SIGNAL_DEADLINE), Ok(()));

    project.apply(&[create_element("Orders Database")]).unwrap();

    assert_eq!(signals.recv_timeout(SIGNAL_DEADLINE), Ok(()));
}

#[test]
fn placing_the_first_element_of_a_project_on_a_board_raises_a_signal() {
    let root = tempfile::tempdir().unwrap();
    let project = new_project(root.path());
    project.apply(&[create_board("Context")]).unwrap();
    // Git keeps no empty directory, so a clone of a project whose boards
    // place nothing has no `abyme/elements/`.
    fs::remove_dir(root.path().join("abyme/elements")).unwrap();
    let (_watching, signals) = signals(root.path());

    project
        .apply(&[
            create_element("Payment Service"),
            Operation::PlaceElement {
                element: Ref::New(LocalKey::try_from("element".to_owned()).unwrap()),
                board: Ref::Existing("context".parse().unwrap()),
            },
        ])
        .unwrap();

    assert_eq!(signals.recv_timeout(SIGNAL_DEADLINE), Ok(()));
}

#[test]
fn reading_the_project_raises_no_signal() {
    let root = tempfile::tempdir().unwrap();
    let project = new_project(root.path());
    project.apply(&[create_board("Context")]).unwrap();
    let (_watching, signals) = signals(root.path());

    project.snapshot().unwrap();

    assert!(signals.recv_timeout(SILENCE).is_err());
}

#[test]
fn a_write_under_the_cache_directory_raises_no_signal() {
    let root = tempfile::tempdir().unwrap();
    new_project(root.path());
    let (_watching, signals) = signals(root.path());

    fs::write(root.path().join(".abyme/written"), "x").unwrap();

    assert!(signals.recv_timeout(SILENCE).is_err());
}

#[test]
fn a_symbolic_link_in_a_watched_directory_is_not_followed() {
    let root = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    let project = new_project(root.path());
    project.apply(&[create_board("Context")]).unwrap();
    symlink(outside.path(), root.path().join("abyme/boards/outside")).unwrap();
    let (_watching, signals) = signals(root.path());

    fs::write(outside.path().join("written"), "x").unwrap();

    assert!(signals.recv_timeout(SILENCE).is_err());
}

#[test]
fn a_boards_directory_that_is_a_symbolic_link_is_not_watched() {
    let root = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    new_project(root.path());
    symlink(outside.path(), root.path().join("abyme/boards")).unwrap();
    let (_watching, signals) = signals(root.path());

    fs::write(outside.path().join("written"), "x").unwrap();

    assert!(signals.recv_timeout(SILENCE).is_err());
}

#[test]
fn an_error_raises_a_signal_and_a_read_does_not() {
    let removed = Ok(Event::new(EventKind::Remove(RemoveKind::Any)));
    let error = Err(notify::Error::generic("the queue overflowed"));

    assert!(!raises_signal(&Ok(read())));
    assert!(raises_signal(&Ok(created())));
    assert!(raises_signal(&removed));
    assert!(raises_signal(&error));
}

#[test]
fn queued_changes_become_one_signal() {
    let (sender, events) = mpsc::channel();
    for _ in 0..3 {
        sender.send(Ok(created())).unwrap();
    }

    let (_watcher, signals, _ended) = signal_loop(events);

    assert_eq!(signals.recv_timeout(SIGNAL_DEADLINE), Ok(()));
    assert!(signals.recv_timeout(SILENCE).is_err());
}

#[test]
fn reads_alone_raise_no_signal() {
    let (sender, events) = mpsc::channel();
    for _ in 0..3 {
        sender.send(Ok(read())).unwrap();
    }

    let (_watcher, signals, _ended) = signal_loop(events);

    assert!(signals.recv_timeout(SILENCE).is_err());
}

#[test]
fn reads_do_not_hold_back_the_signal_of_a_change() {
    let (sender, events) = mpsc::channel();
    sender.send(Ok(created())).unwrap();
    let (_watcher, signals, _ended) = signal_loop(events);
    let reading = Arc::new(AtomicBool::new(true));
    let still_reading = Arc::clone(&reading);
    thread::spawn(move || {
        while still_reading.load(Ordering::Relaxed) && sender.send(Ok(read())).is_ok() {
            thread::sleep(QUIET / 10);
        }
    });

    let signal = signals.recv_timeout(SIGNAL_DEADLINE);
    reading.store(false, Ordering::Relaxed);

    assert_eq!(signal, Ok(()));
}

#[test]
fn the_loop_ends_when_its_events_end() {
    let (sender, events) = mpsc::channel();
    let (_watcher, _signals, ended) = signal_loop(events);

    drop(sender);

    assert_eq!(ended.recv_timeout(SIGNAL_DEADLINE), Ok(()));
}

#[test]
fn the_loop_ends_without_a_signal_once_the_watcher_is_dropped() {
    let (sender, events) = mpsc::channel();
    let (watcher, signals, ended) = signal_loop(events);

    drop(watcher);
    sender.send(Ok(created())).unwrap();

    assert_eq!(ended.recv_timeout(SIGNAL_DEADLINE), Ok(()));
    assert!(signals.try_recv().is_err());
}
