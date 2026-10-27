//! The Abyme core: operations on elements and boards, and the project on
//! disk that holds them.
//!
//! [`apply`] turns the operations of a request into the files to write,
//! without touching the file system, so every decision can be tested on its
//! own. [`Project`] finds the project a directory lies in, reads its files,
//! writes a request's changes safely, and asks Git what changed, from which
//! [`Review`] holds the review flags. [`outline`] renders a board as the
//! compact text the AI reads, and [`element_detail`] one element in detail.

// A project is used only when it belongs to the user running Abyme, and the
// owner check exists for Unix only.
#[cfg(not(unix))]
compile_error!("abyme-core checks who owns a project, which it can do on Unix only");

mod apply;
mod board_tree;
mod layout;
mod operation;
mod outline;
mod project;
mod review;
mod slug;

pub use apply::{
    Applied, ApplyError, Created, Kind, MAX_OPERATIONS, MAX_SHOWN_SOURCES, OperationProblem,
    RelationSources, Snapshot, Write, apply,
};
pub use operation::{
    LocalKey, LocalKeyError, MAX_COORDINATE, MAX_KEY_LEN, Operation, Operations, Ref,
};
pub use outline::{
    Geometry, UnknownBoard, UnknownElement, element_detail, element_list, outline, status_report,
};
pub use project::{
    EmbeddingModel, EntryKind, Init, MAX_FILE_BYTES, MAX_PROJECT_FILE_BYTES, PROJECT_FORMAT,
    Project, ProjectError,
};
pub use review::{Flag, Priority, Review, Unavailable, VerifyProblem};
