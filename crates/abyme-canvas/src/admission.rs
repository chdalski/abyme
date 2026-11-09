//! Which operations the canvas may send: a request reaches the project only
//! when every one of its operations is admitted here.
//!
//! [`admit`] is pure and runs before any access to the project. It matches
//! every variant of the core's operations and references by name, so a new
//! one breaks the build until it is admitted or refused here. A request that
//! creates an element is admitted only as a whole, by [`creates_a_note`]: a
//! note, and at most its attachment to an element on the same board.
//! [`check_board_files`] then decides from the project's files, also by name
//! for each variant: a release of a pin keeps the position in the board file,
//! so that a release from a stale view does not move a box back; an update, a
//! deletion, an attachment, and a detachment concern only a note, whose file
//! says what it is; and a deletion only one on which nothing hangs on any
//! board. No rule looks at a board's mode: a person arranges a mirrored board
//! as a free one, while what it shows follows the code, so a mode that
//! restricts people needs a rule of its own here. These are rules of this one
//! client, not a boundary: the files stay open to the AI and to edits by hand,
//! and the position authorizes nothing, since a page that may call `apply` can
//! also read it. A move the canvas sends pins the element, since a person
//! placed it there.

use std::fmt;

use abyme_core::{LocalKey, MAX_OPERATIONS, Operation, Ref, Snapshot};
use abyme_model::{Board, BoardId, Element, ElementId, NOTE_TYPE, StatementKind, is_allowed};

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
    /// One operation changes, deletes, attaches, or detaches an element whose
    /// file does not say it is a note.
    NotANote {
        /// The operation's position in the request, counted from 1.
        position: usize,
    },
    /// One operation deletes a note that the file of a board names as the box
    /// something hangs on, whether that attachment holds or not.
    HungOn {
        /// The operation's position in the request, counted from 1.
        position: usize,
        /// A board on which something hangs on the note.
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
            Self::NotANote { position } => write!(
                formatter,
                "operation {position}: the canvas may change, delete, attach, and detach only \
                 notes"
            ),
            Self::HungOn { position, board } => write!(
                formatter,
                "operation {position}: something on board {board} hangs on the note, so the \
                 canvas does not delete it"
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
/// releasing its pin, which says `pinned: false`, changing the name and body of
/// an element that exists, deleting an element that exists, attaching a note
/// that exists to an element that exists on a board that exists, with
/// `pinned: true`, detaching such a note, and, as a whole request, creating a
/// note, and hanging it, in the same request, on an element placed on that
/// board.
///
/// Whatever page a debug build loads from the dev server's address can send
/// every operation admitted here, so it can create notes on any board, also
/// hung at once on an element placed there, write their text into the
/// repository, where the AI reads it, and delete them; it
/// can hang any note on any element placed on a board, which adds the relation
/// `concerns` to the note and drops its verification stamp, and detach notes;
/// and it can move, pin, and release any element placed on any board,
/// mirrored or not.
/// Admitting another operation needs a security review of that origin too.
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
    if creates_a_note(operations) {
        return Ok(());
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
/// as they stand before the request, on every board alike, whatever its mode:
/// none releases a pin anywhere but at the element's position in the file of
/// the board it names, so no request leaves a box at a new position without a
/// pin. An update, a
/// deletion, an attachment, or a detachment concerns only an element whose file
/// says it is a note, on any board, and a deletion only one that no board names
/// as the box something hangs on, since detaching it would rewrite that line.
/// No admitted operation makes a file name a note, so the files before the
/// request decide. A request that creates a
/// note passes on any board, and it changes nothing that is there. A board
/// `snapshot` does not have passes, and the core rejects it.
///
/// # Errors
///
/// Returns [`Refused`] for the first such operation.
pub fn check_board_files(snapshot: &Snapshot, operations: &[Operation]) -> Result<(), Refused> {
    if creates_a_note(operations) {
        return Ok(());
    }
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
                (*pinned == Some(false) && !lies_at(found, element, *x, *y)).then(|| {
                    Refused::ReleaseElsewhere {
                        position,
                        board: id.clone(),
                    }
                })
            }
            Ref::New(_) => None,
        },
        Operation::UpdateElement { element, .. } => {
            (!is_a_note(snapshot, element)).then_some(Refused::NotANote { position })
        }
        // A deletion names no board: a note goes from every board, mirrored or not,
        // and detaching what hangs on it there would rewrite those lines.
        Operation::DeleteElement { element } => {
            if !is_a_note(snapshot, element) {
                return Some(Refused::NotANote { position });
            }
            snapshot
                .boards
                .values()
                .find(|board| is_named_on(board, element))
                .map(|board| Refused::HungOn {
                    position,
                    board: board.id.clone(),
                })
        }
        Operation::AttachNote { note, .. } | Operation::DetachNote { note, .. } => {
            (!is_a_note(snapshot, note)).then_some(Refused::NotANote { position })
        }
        Operation::CreateElement { .. }
        | Operation::CreateBoard { .. }
        | Operation::UpdateBoard { .. }
        | Operation::PlaceElement { .. }
        | Operation::LayoutBoard { .. }
        | Operation::CreateRelation { .. }
        | Operation::DeleteRelation { .. } => None,
    }
}

/// Whether `operations` is exactly a request that creates one note: its own
/// statement kind `idea`, placed on a board that exists and pinned where a person
/// put it, and, as a fourth and last operation, at most one attachment that
/// hangs that new note, with its pin confirmed, on an existing element of the
/// same board. Every field is named, so that a new one breaks the build until it
/// is admitted or refused here.
fn creates_a_note(operations: &[Operation]) -> bool {
    match operations {
        [create, place, pin] => created_note(create, place, pin).is_some(),
        [
            create,
            place,
            pin,
            Operation::AttachNote {
                note: Ref::New(attached),
                to: Ref::Existing(_),
                board: Ref::Existing(attached_on),
                pinned: Some(_),
            },
        ] => created_note(create, place, pin)
            .is_some_and(|(key, board)| attached == key && attached_on == board),
        _ => false,
    }
}

/// The key and the board of the note that `create`, `place`, and `pin` create,
/// if they are exactly the three operations that create one note: its own
/// statement kind `idea`, placed on a board that exists and pinned where a
/// person put it. Every field is named, so that a new one breaks the build until
/// it is admitted or refused here.
#[expect(
    clippy::unneeded_field_pattern,
    reason = "naming every field of the move breaks the build when it gains one, until that field is admitted or refused here"
)]
fn created_note<'a>(
    create: &'a Operation,
    place: &'a Operation,
    pin: &'a Operation,
) -> Option<(&'a LocalKey, &'a BoardId)> {
    match (create, place, pin) {
        (
            Operation::CreateElement {
                key,
                type_ref,
                name,
                summary: None,
                body,
                anchor: None,
                statement: Some(StatementKind::Idea),
            },
            Operation::PlaceElement {
                element: Ref::New(placed),
                board: Ref::Existing(placed_on),
            },
            Operation::MoveElement {
                element: Ref::New(moved),
                board: Ref::Existing(moved_on),
                x: _,
                y: _,
                pinned: Some(true),
            },
        ) => (type_ref == NOTE_TYPE
            && placed == key
            && moved == key
            && placed_on == moved_on
            && is_note_name(name)
            && is_note_body(body))
        .then_some((key, placed_on)),
        _ => None,
    }
}

/// Whether the file of `element` in `snapshot` says it is a note. A missing
/// file says nothing of the kind.
fn is_a_note(snapshot: &Snapshot, element: &Ref<ElementId>) -> bool {
    match element {
        Ref::Existing(element) => snapshot.elements.get(element).is_some_and(Element::is_note),
        Ref::New(_) => false,
    }
}

/// Whether `name` can be the name of a note: its first line, so not blank and
/// without a line break.
fn is_note_name(name: &str) -> bool {
    !name.trim().is_empty() && !name.contains(['\n', '\r', '\u{85}', '\u{2028}', '\u{2029}'])
}

/// Whether `body` holds only characters an element's name may hold. A control
/// character would hide the file from `git diff`, the review of what the AI
/// reads.
fn is_note_body(body: &str) -> bool {
    body.chars().all(is_allowed)
}

/// Whether the file of `board` names `element` as the box something hangs on.
fn is_named_on(board: &Board, element: &Ref<ElementId>) -> bool {
    match element {
        Ref::Existing(element) => board.is_named(element),
        Ref::New(_) => false,
    }
}

/// Whether `element` lies at `x` and `y` on `board`.
fn lies_at(board: &Board, element: &Ref<ElementId>, x: i32, y: i32) -> bool {
    match element {
        Ref::Existing(element) => board
            .rect_of(element)
            .is_some_and(|rect| rect.x == x && rect.y == y),
        Ref::New(_) => false,
    }
}

/// The kind of `operation`, if the canvas may not send it.
#[expect(
    clippy::unneeded_field_pattern,
    reason = "naming every field of a move breaks the build when it gains one, until that field is admitted or refused here"
)]
fn refused_kind(operation: &Operation) -> Option<&'static str> {
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
        // The canvas attaches an existing note only right after its own move,
        // which pins it, so the attachment always confirms the pin; a note it
        // creates and attaches at once is admitted by `creates_a_note`.
        Operation::AttachNote {
            note,
            to,
            board,
            pinned,
        } => match (note, to, board, pinned) {
            (Ref::Existing(_), Ref::Existing(_), Ref::Existing(_), Some(_)) => None,
            (Ref::Existing(_), Ref::Existing(_), Ref::Existing(_), None) => {
                Some("attach_note without pinned")
            }
            _ => Some("attach_note with a new element or board"),
        },
        Operation::DetachNote { note, board } => match (note, board) {
            (Ref::Existing(_), Ref::Existing(_)) => None,
            _ => Some("detach_note with a new element or board"),
        },
        // `None` here means a field the request leaves alone; `Some(None)` would
        // remove it.
        Operation::UpdateElement {
            element,
            name,
            type_ref,
            summary,
            body,
            anchor,
            statement,
        } => match (element, name, body, type_ref, summary, anchor, statement) {
            (Ref::Existing(_), Some(name), Some(body), None, None, None, None)
                if is_note_name(name) && is_note_body(body) =>
            {
                None
            }
            _ => Some("update_element"),
        },
        Operation::DeleteElement { element } => match element {
            Ref::Existing(_) => None,
            Ref::New(_) => Some("delete_element"),
        },
        Operation::CreateRelation { .. } => Some("create_relation"),
        Operation::DeleteRelation { .. } => Some("delete_relation"),
    }
}
