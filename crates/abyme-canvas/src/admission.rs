//! Which operations the canvas may send: a request reaches the project only
//! when every one of its operations is admitted here.
//!
//! The check is pure and runs before any access to the project. It matches
//! every variant of the core's operations and references by name, so a new
//! one breaks the build until it is admitted or refused here.

use std::fmt;

use abyme_core::{MAX_OPERATIONS, Operation, Ref};

#[cfg(test)]
mod tests;

/// Why the canvas refuses a request. It names where and what kind of
/// operation, never anything the operation carries.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refused {
    /// The request carries more than [`MAX_OPERATIONS`] operations.
    TooMany {
        /// The number of operations in the request.
        count: usize,
    },
    /// One operation is of a kind the canvas may not send.
    Operation {
        /// The operation's position in the request, counted from 1.
        position: usize,
        /// The kind of operation, named by this module.
        kind: &'static str,
    },
}

impl fmt::Display for Refused {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooMany { count } => write!(
                formatter,
                "the request has {count} operations, more than {MAX_OPERATIONS}"
            ),
            Self::Operation { position, kind } => write!(
                formatter,
                "operation {position}: the canvas may not send {kind}"
            ),
        }
    }
}

/// Checks that the canvas may send every operation of a request. Admitted
/// today: moving an element that exists on a board that exists.
///
/// Whatever page a debug build loads from the dev server's address can send
/// every operation admitted here, so admitting another one needs a security
/// review of that origin too; one that carries text would let such a page
/// write that text into the repository.
///
/// # Errors
///
/// Returns [`Refused`] for a request with more than [`MAX_OPERATIONS`]
/// operations, and otherwise for its first operation that is not admitted.
pub fn admit(operations: &[Operation]) -> Result<(), Refused> {
    if operations.len() > MAX_OPERATIONS {
        return Err(Refused::TooMany {
            count: operations.len(),
        });
    }
    operations
        .iter()
        .zip(1..)
        .find_map(|(operation, position)| {
            refused_kind(operation).map(|kind| Refused::Operation { position, kind })
        })
        .map_or(Ok(()), Err)
}

/// The kind of `operation`, if the canvas may not send it.
const fn refused_kind(operation: &Operation) -> Option<&'static str> {
    match operation {
        Operation::CreateElement { .. } => Some("create_element"),
        Operation::CreateBoard { .. } => Some("create_board"),
        Operation::PlaceElement { .. } => Some("place_element"),
        Operation::MoveElement { element, board, .. } => match (element, board) {
            (Ref::Existing(_), Ref::Existing(_)) => None,
            (Ref::New(_), Ref::Existing(_) | Ref::New(_)) | (Ref::Existing(_), Ref::New(_)) => {
                Some("move_element with a new element or board")
            }
        },
        Operation::CreateRelation { .. } => Some("create_relation"),
        Operation::DeleteRelation { .. } => Some("delete_relation"),
    }
}
