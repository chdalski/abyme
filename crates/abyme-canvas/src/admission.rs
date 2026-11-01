//! Which operations the canvas may send: a request reaches the project only
//! when every one of its operations is admitted here.
//!
//! [`admit`] is pure and runs before any access to the project. It matches
//! every variant of the core's operations and references by name, so a new
//! one breaks the build until it is admitted or refused here.
//! [`check_modes`] then decides from the project's files, also by name for
//! each variant and each mode: a person does not move what lies on a mirrored
//! board. That is a rule of this one client, not a boundary: the files stay
//! open to the AI and to edits by hand. A move the canvas sends pins the
//! element, since a person placed it there.

use std::fmt;

use abyme_core::{MAX_OPERATIONS, Operation, Ref, Snapshot};
use abyme_model::{BoardId, Mode};

#[cfg(test)]
mod tests;

/// Why the canvas refuses a request. It names where and what kind of
/// operation, and a board by its validated ID, never any other text the
/// operation or the project holds.
#[derive(Debug, Clone, PartialEq, Eq)]
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
    /// One operation moves an element on a mirrored board.
    Mirrored {
        /// The operation's position in the request, counted from 1.
        position: usize,
        /// The board.
        board: BoardId,
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
            Self::Mirrored { position, board } => write!(
                formatter,
                "operation {position}: board {board} is mirrored, so the canvas may not move its \
                 elements"
            ),
        }
    }
}

/// Checks that the canvas may send every operation of a request. Admitted
/// today: moving and pinning an element that exists on a board that exists.
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

/// Checks that no operation of a request moves an element on a mirrored
/// board of `snapshot`. A board `snapshot` does not have passes, and the core
/// rejects it.
///
/// # Errors
///
/// Returns [`Refused::Mirrored`] for the first such operation.
pub fn check_modes(snapshot: &Snapshot, operations: &[Operation]) -> Result<(), Refused> {
    operations
        .iter()
        .zip(1..)
        .find_map(|(operation, position)| {
            mirrored_board(snapshot, operation).map(|board| Refused::Mirrored {
                position,
                board: board.clone(),
            })
        })
        .map_or(Ok(()), Err)
}

/// The board `operation` moves an element on, if it is mirrored. [`admit`]
/// lets no other operation and no new board through; they pass here.
fn mirrored_board<'a>(snapshot: &Snapshot, operation: &'a Operation) -> Option<&'a BoardId> {
    match operation {
        Operation::MoveElement { board, .. } => match board {
            Ref::Existing(board) => match snapshot.boards.get(board)?.zone?.mode {
                Mode::Mirrored => Some(board),
                Mode::Free => None,
            },
            Ref::New(_) => None,
        },
        Operation::CreateElement { .. }
        | Operation::CreateBoard { .. }
        | Operation::UpdateBoard { .. }
        | Operation::PlaceElement { .. }
        | Operation::LayoutBoard { .. }
        | Operation::UpdateElement { .. }
        | Operation::DeleteElement { .. }
        | Operation::CreateRelation { .. }
        | Operation::DeleteRelation { .. } => None,
    }
}

/// The kind of `operation`, if the canvas may not send it.
#[expect(
    clippy::unneeded_field_pattern,
    reason = "naming every field of a move breaks the build when it gains one, until that field is admitted or refused here"
)]
const fn refused_kind(operation: &Operation) -> Option<&'static str> {
    match operation {
        Operation::CreateElement { .. } => Some("create_element"),
        Operation::CreateBoard { .. } => Some("create_board"),
        Operation::UpdateBoard { .. } => Some("update_board"),
        Operation::PlaceElement { .. } => Some("place_element"),
        Operation::MoveElement {
            element,
            board,
            x: _,
            y: _,
            pinned,
        } => match (element, board, pinned) {
            (Ref::Existing(_), Ref::Existing(_), Some(true)) => None,
            (Ref::Existing(_), Ref::Existing(_), None | Some(false)) => {
                Some("move_element without pinned: true")
            }
            (Ref::New(_), ..) | (Ref::Existing(_), Ref::New(_), _) => {
                Some("move_element with a new element or board")
            }
        },
        Operation::LayoutBoard { .. } => Some("layout_board"),
        Operation::UpdateElement { .. } => Some("update_element"),
        Operation::DeleteElement { .. } => Some("delete_element"),
        Operation::CreateRelation { .. } => Some("create_relation"),
        Operation::DeleteRelation { .. } => Some("delete_relation"),
    }
}
