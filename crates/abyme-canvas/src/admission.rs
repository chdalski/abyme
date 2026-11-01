//! Which operations the canvas may send: a request reaches the project only
//! when every one of its operations is admitted here.
//!
//! [`admit`] is pure and runs before any access to the project. It matches
//! every variant of the core's operations and references by name, so a new
//! one breaks the build until it is admitted or refused here.
//! [`check_board_files`] then decides from the project's files, also by name
//! for each variant and each mode: a person does not move what lies on a
//! mirrored board, and a release of a pin keeps the position in the board
//! file, so that a release from a stale view does not move a box back. These
//! are rules of this one client, not a boundary: the files stay open to the AI
//! and to edits by hand, and the position authorizes nothing, since a page
//! that may call `apply` can also read it. A move the canvas sends pins the
//! element, since a person placed it there.

use std::fmt;

use abyme_core::{MAX_OPERATIONS, Operation, Ref, Snapshot};
use abyme_model::{Board, BoardId, ElementId, Mode};

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
    /// One operation releases a pin anywhere but at the element's position
    /// in the board file.
    ReleaseElsewhere {
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
            Self::ReleaseElsewhere { position, board } => write!(
                formatter,
                "operation {position}: the canvas releases a pin on board {board} only at the \
                 position in the board file"
            ),
        }
    }
}

/// Checks that the canvas may send every operation of a request. Admitted
/// today: moving and pinning an element that exists on a board that exists,
/// and releasing its pin, which says `pinned: false`.
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

/// Checks every operation of a request against the board files of `snapshot`,
/// as they stand before the request: none moves an element on a mirrored
/// board, and none releases a pin anywhere but at the element's position in
/// the file of the board it names. So no request leaves a box at a new position
/// without a pin. A board `snapshot` does not have passes, and the core rejects
/// it.
///
/// # Errors
///
/// Returns [`Refused`] for the first such operation.
pub fn check_board_files(snapshot: &Snapshot, operations: &[Operation]) -> Result<(), Refused> {
    operations
        .iter()
        .zip(1..)
        .find_map(|(operation, position)| refused_by_board_file(snapshot, operation, position))
        .map_or(Ok(()), Err)
}

/// Why the board files of `snapshot` refuse `operation`, if they do. [`admit`]
/// lets no other operation and no new board through; they pass here.
fn refused_by_board_file(
    snapshot: &Snapshot,
    operation: &Operation,
    position: usize,
) -> Option<Refused> {
    match operation {
        Operation::MoveElement {
            element,
            board,
            x,
            y,
            pinned,
        } => match board {
            Ref::Existing(id) => {
                let found = snapshot.boards.get(id)?;
                match found.zone.map(|zone| zone.mode) {
                    Some(Mode::Mirrored) => Some(Refused::Mirrored {
                        position,
                        board: id.clone(),
                    }),
                    Some(Mode::Free) | None => (*pinned == Some(false)
                        && !lies_at(found, element, *x, *y))
                    .then(|| Refused::ReleaseElsewhere {
                        position,
                        board: id.clone(),
                    }),
                }
            }
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

/// Whether the file of `board` places `element` at `x` and `y`.
fn lies_at(board: &Board, element: &Ref<ElementId>, x: i32, y: i32) -> bool {
    match element {
        Ref::Existing(element) => board
            .placements
            .get(element)
            .is_some_and(|placed| placed.rect.x == x && placed.rect.y == y),
        Ref::New(_) => false,
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
            (Ref::Existing(_), Ref::Existing(_), Some(_)) => None,
            (Ref::Existing(_), Ref::Existing(_), None) => Some("move_element without pinned"),
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
