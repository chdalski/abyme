//! The Abyme core: operations on elements and boards.
//!
//! [`apply`] turns the operations of a request into the files to write,
//! without touching the file system, so every decision can be tested on its
//! own.

mod apply;
mod layout;
mod operation;
mod slug;

pub use apply::{
    Applied, ApplyError, Created, Kind, MAX_OPERATIONS, OperationProblem, Snapshot, Write, apply,
};
pub use operation::{LocalKey, LocalKeyError, MAX_KEY_LEN, Operation, Operations, Ref};
