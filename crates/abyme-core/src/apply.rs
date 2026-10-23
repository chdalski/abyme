//! Turning operations into file writes, without touching any file.
//!
//! [`apply`] is a pure function: it takes the elements and boards as they
//! are and returns the files a request changes, or an error and nothing.
//! The caller writes the files.

use std::collections::btree_map::Entry;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use abyme_model::{Board, BoardId, Element, ElementId, ElementProblem, TypeRef};

use crate::layout::Layout;
use crate::operation::{LocalKey, MAX_COORDINATE, Operation, Ref};
use crate::slug::free_slug;

/// The most operations one request may carry. A thousand covers a C4 model
/// of a large repository in one request and keeps a single call's work and
/// its write lock short.
pub const MAX_OPERATIONS: usize = 1000;

/// The elements and boards as they are before a request.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Snapshot {
    /// Every element, by ID.
    pub elements: BTreeMap<ElementId, Element>,
    /// Every board, by ID.
    pub boards: BTreeMap<BoardId, Board>,
}

/// What a request changes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Applied {
    /// The files to write: new elements first, then each new or changed
    /// board once, so that no board ever points to an element whose file
    /// does not exist yet.
    pub writes: Vec<Write>,
    /// The ID each key of the request received, in the order of creation.
    pub created: Vec<(LocalKey, Created)>,
}

/// One file to write, with its whole content.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Write {
    /// A new element file.
    Element(Element),
    /// A new or changed board file.
    Board(Board),
}

/// What a key of the request created.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Created {
    /// An element with this ID.
    Element(ElementId),
    /// A board with this ID.
    Board(BoardId),
}

/// Why a request was rejected. Nothing of a rejected request is written.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ApplyError {
    /// The request carries more than [`MAX_OPERATIONS`] operations.
    #[error("the request has {count} operations, more than {MAX_OPERATIONS}")]
    TooManyOperations {
        /// The number of operations in the request.
        count: usize,
    },
    /// One operation cannot be applied.
    #[error("operation {position}: {problem}")]
    Operation {
        /// The operation's position in the request, counted from 1.
        position: usize,
        /// What is wrong with it.
        problem: OperationProblem,
    },
}

/// What is wrong with one operation. Keys and IDs here passed their checks,
/// so they are shown as they are; content such as names is never shown.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum OperationProblem {
    /// No element has this ID.
    #[error("no element has the ID {0}")]
    UnknownElement(ElementId),
    /// No board has this ID.
    #[error("no board has the ID {0}")]
    UnknownBoard(BoardId),
    /// No earlier operation of the request created this key.
    #[error("no earlier operation of this request creates the key {0}")]
    UnknownKey(LocalKey),
    /// The request creates this key twice.
    #[error("the key {0} is already used in this request")]
    DuplicateKey(LocalKey),
    /// The key names something of the other kind.
    #[error("the key {key} names {actual}, not {expected}")]
    WrongKind {
        /// The key.
        key: LocalKey,
        /// What the operation expects.
        expected: Kind,
        /// What the key names.
        actual: Kind,
    },
    /// The element already lies on a board.
    #[error("element {0} already has a home on a board")]
    AlreadyHomed(ElementId),
    /// The type, name, or summary breaks a rule of the element file.
    #[error("{0}")]
    InvalidElement(ElementProblem),
    /// No free position on the board fits the coordinate range.
    #[error("board {0} has no free position left")]
    BoardFull(BoardId),
    /// The element has no placement on the board it is to move on.
    #[error("element {element} does not lie on board {board}")]
    NotOnBoard {
        /// The element.
        element: ElementId,
        /// The board.
        board: BoardId,
    },
    /// A move's position lies outside the range a move accepts.
    #[error("the position x {x}, y {y} lies outside -{MAX_COORDINATE} to {MAX_COORDINATE}")]
    OutOfRange {
        /// The left edge asked for.
        x: i32,
        /// The top edge asked for.
        y: i32,
    },
    /// No free ID could be derived; the suffixes make this unreachable.
    #[error("no free ID is left for the name")]
    NoFreeId,
}

/// The two kinds of things a request creates and refers to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// An element.
    Element,
    /// A board.
    Board,
}

/// The request's changes as they build up, operation by operation.
struct Draft<'snapshot> {
    snapshot: &'snapshot Snapshot,
    keys: BTreeMap<LocalKey, Created>,
    created: Vec<(LocalKey, Created)>,
    new_elements: Vec<Element>,
    taken_elements: BTreeSet<ElementId>,
    taken_boards: BTreeSet<BoardId>,
    homed: BTreeSet<ElementId>,
    boards: BTreeMap<BoardId, (Board, Layout)>,
}

/// Applies `operations` to `snapshot` and returns the files to write.
///
/// Operations run in order: a key exists from the operation that creates it
/// on, and elements and boards share one set of keys.
///
/// # Errors
///
/// Returns an [`ApplyError`] when the request has too many operations or an
/// operation cannot be applied; then nothing is to be written.
pub fn apply(snapshot: &Snapshot, operations: &[Operation]) -> Result<Applied, ApplyError> {
    if operations.len() > MAX_OPERATIONS {
        return Err(ApplyError::TooManyOperations {
            count: operations.len(),
        });
    }
    let mut draft = Draft::new(snapshot);
    for (index, operation) in operations.iter().enumerate() {
        draft
            .apply(operation)
            .map_err(|problem| ApplyError::Operation {
                position: index + 1,
                problem,
            })?;
    }
    Ok(draft.finish())
}

impl<'snapshot> Draft<'snapshot> {
    fn new(snapshot: &'snapshot Snapshot) -> Self {
        let placed: BTreeSet<ElementId> = snapshot
            .boards
            .values()
            .flat_map(|board| board.placements.keys().cloned())
            .collect();
        // An element placed on a board whose file is missing still owns its
        // ID: a new element must not take over that placement.
        let taken_elements = snapshot
            .elements
            .keys()
            .cloned()
            .chain(placed.iter().cloned())
            .collect();
        Self {
            snapshot,
            keys: BTreeMap::new(),
            created: Vec::new(),
            new_elements: Vec::new(),
            taken_elements,
            taken_boards: snapshot.boards.keys().cloned().collect(),
            homed: placed,
            boards: BTreeMap::new(),
        }
    }

    fn apply(&mut self, operation: &Operation) -> Result<(), OperationProblem> {
        match operation {
            Operation::CreateElement {
                key,
                type_ref,
                name,
                summary,
                body,
            } => self.create_element(key, type_ref, name, summary.as_deref(), body),
            Operation::CreateBoard { key, title } => self.create_board(key, title),
            Operation::PlaceElement { element, board } => self.place_element(element, board),
            Operation::MoveElement {
                element,
                board,
                x,
                y,
            } => self.move_element(element, board, *x, *y),
        }
    }

    fn create_element(
        &mut self,
        key: &LocalKey,
        type_ref: &str,
        name: &str,
        summary: Option<&str>,
        body: &str,
    ) -> Result<(), OperationProblem> {
        self.check_new_key(key)?;
        let type_ref =
            TypeRef::try_from(type_ref.to_owned()).map_err(OperationProblem::InvalidElement)?;
        let id =
            free_slug(name, "element", &self.taken_elements).ok_or(OperationProblem::NoFreeId)?;
        let element = Element::new(
            id.clone(),
            type_ref,
            name.to_owned(),
            summary.map(str::to_owned),
            body.to_owned(),
        )
        .map_err(|error| OperationProblem::InvalidElement(error.problem().clone()))?;
        self.taken_elements.insert(id.clone());
        self.new_elements.push(element);
        self.record(key, Created::Element(id));
        Ok(())
    }

    fn create_board(&mut self, key: &LocalKey, title: &str) -> Result<(), OperationProblem> {
        self.check_new_key(key)?;
        let id = free_slug(title, "board", &self.taken_boards).ok_or(OperationProblem::NoFreeId)?;
        let board = Board {
            id: id.clone(),
            title: title.to_owned(),
            placements: BTreeMap::new(),
        };
        let layout = Layout::of(&board);
        self.taken_boards.insert(id.clone());
        self.boards.insert(id.clone(), (board, layout));
        self.record(key, Created::Board(id));
        Ok(())
    }

    fn place_element(
        &mut self,
        element: &Ref<ElementId>,
        board: &Ref<BoardId>,
    ) -> Result<(), OperationProblem> {
        let element = self.resolve_element(element)?;
        let board = self.resolve_board(board)?;
        if self.homed.contains(&element) {
            return Err(OperationProblem::AlreadyHomed(element));
        }
        let (draft_board, layout) = self.draft_board(&board)?;
        let rect = layout
            .place()
            .ok_or_else(|| OperationProblem::BoardFull(board.clone()))?;
        draft_board.placements.insert(element.clone(), rect);
        self.homed.insert(element);
        Ok(())
    }

    fn move_element(
        &mut self,
        element: &Ref<ElementId>,
        board: &Ref<BoardId>,
        x: i32,
        y: i32,
    ) -> Result<(), OperationProblem> {
        let element = self.resolve_element(element)?;
        let board = self.resolve_board(board)?;
        let range = -MAX_COORDINATE..=MAX_COORDINATE;
        if !range.contains(&x) || !range.contains(&y) {
            return Err(OperationProblem::OutOfRange { x, y });
        }
        let (draft_board, layout) = self.draft_board(&board)?;
        let placement = draft_board.placements.get_mut(&element).ok_or_else(|| {
            OperationProblem::NotOnBoard {
                element: element.clone(),
                board: board.clone(),
            }
        })?;
        placement.x = x;
        placement.y = y;
        layout.occupy(placement);
        Ok(())
    }

    /// The board as the request has changed it so far, with the layout of
    /// its free space. A board the request has not touched yet is loaded
    /// from the snapshot on first use.
    fn draft_board(&mut self, board: &BoardId) -> Result<&mut (Board, Layout), OperationProblem> {
        match self.boards.entry(board.clone()) {
            Entry::Occupied(entry) => Ok(entry.into_mut()),
            Entry::Vacant(entry) => {
                let existing = self
                    .snapshot
                    .boards
                    .get(board)
                    .ok_or_else(|| OperationProblem::UnknownBoard(board.clone()))?;
                Ok(entry.insert((existing.clone(), Layout::of(existing))))
            }
        }
    }

    fn resolve_element(&self, reference: &Ref<ElementId>) -> Result<ElementId, OperationProblem> {
        match reference {
            Ref::Existing(id) if self.snapshot.elements.contains_key(id) => Ok(id.clone()),
            Ref::Existing(id) => Err(OperationProblem::UnknownElement(id.clone())),
            Ref::New(key) => match self.keys.get(key) {
                Some(Created::Element(id)) => Ok(id.clone()),
                Some(Created::Board(_)) => Err(OperationProblem::WrongKind {
                    key: key.clone(),
                    expected: Kind::Element,
                    actual: Kind::Board,
                }),
                None => Err(OperationProblem::UnknownKey(key.clone())),
            },
        }
    }

    fn resolve_board(&self, reference: &Ref<BoardId>) -> Result<BoardId, OperationProblem> {
        match reference {
            Ref::Existing(id) if self.snapshot.boards.contains_key(id) => Ok(id.clone()),
            Ref::Existing(id) => Err(OperationProblem::UnknownBoard(id.clone())),
            Ref::New(key) => match self.keys.get(key) {
                Some(Created::Board(id)) => Ok(id.clone()),
                Some(Created::Element(_)) => Err(OperationProblem::WrongKind {
                    key: key.clone(),
                    expected: Kind::Board,
                    actual: Kind::Element,
                }),
                None => Err(OperationProblem::UnknownKey(key.clone())),
            },
        }
    }

    fn check_new_key(&self, key: &LocalKey) -> Result<(), OperationProblem> {
        if self.keys.contains_key(key) {
            Err(OperationProblem::DuplicateKey(key.clone()))
        } else {
            Ok(())
        }
    }

    fn record(&mut self, key: &LocalKey, created: Created) {
        self.keys.insert(key.clone(), created.clone());
        self.created.push((key.clone(), created));
    }

    fn finish(self) -> Applied {
        let elements = self.new_elements.into_iter().map(Write::Element);
        let boards = self
            .boards
            .into_values()
            .map(|(board, _)| Write::Board(board));
        Applied {
            writes: elements.chain(boards).collect(),
            created: self.created,
        }
    }
}

impl fmt::Display for Kind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Element => "an element",
            Self::Board => "a board",
        })
    }
}

#[cfg(test)]
mod tests;
