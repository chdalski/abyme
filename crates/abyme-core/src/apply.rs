//! Turning operations into file writes, without touching any file.
//!
//! [`apply`] is a pure function: it takes the elements and boards as they
//! are and returns the files a request changes, or an error and nothing.
//! The caller writes the files.

use std::collections::btree_map::Entry;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use abyme_model::{
    Anchor, Board, BoardId, Element, ElementFields, ElementId, ElementProblem, RelationKind,
    Relations, TypeRef,
};

use crate::layout::Layout;
use crate::operation::{LocalKey, MAX_COORDINATE, Operation, Ref};
use crate::slug::free_slug;

/// How many sources of relations a rejected deletion names at most.
pub const MAX_SHOWN_SOURCES: usize = 10;

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
    /// The files to write and remove: new elements first, then changed
    /// elements, then each new or changed board once, then the element files
    /// to remove, so that no board ever points to an element whose file does
    /// not exist yet. Each element is written or removed once.
    pub writes: Vec<Write>,
    /// The ID each key of the request received, in the order of creation.
    pub created: Vec<(LocalKey, Created)>,
}

/// One file to write, with its whole content, or to remove.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Write {
    /// A new or changed element file.
    Element(Element),
    /// A new or changed board file.
    Board(Board),
    /// The file of a deleted element, which exists before the request.
    RemoveElement(ElementId),
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
    /// The type, name, summary, anchor, or a relation breaks a rule of the
    /// element file.
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
    /// The element already has a relation of this kind to this target.
    #[error("element {from} already has the relation {kind} to {to}")]
    RelationExists {
        /// The element the relation goes out from.
        from: ElementId,
        /// The relation's kind.
        kind: RelationKind,
        /// The element the relation points to.
        to: ElementId,
    },
    /// Other elements still relate to the element a deletion names.
    #[error(
        "element {element} is still the target of relations from {sources}; delete those first"
    )]
    StillRelated {
        /// The element to delete.
        element: ElementId,
        /// The sources and kinds of the relations that point to it.
        sources: RelationSources,
    },
    /// An update names no field to change.
    #[error("the update of element {0} names no field to change")]
    NothingToUpdate(ElementId),
    /// The element has no relation of this kind to this target.
    #[error("element {from} has no relation {kind} to {to}")]
    NoSuchRelation {
        /// The element the relation would go out from.
        from: ElementId,
        /// The relation's kind.
        kind: RelationKind,
        /// The element the relation would point to.
        to: ElementId,
    },
}

/// The source and kind of each relation that points to an element, by
/// source, then kind. An error shows at most [`MAX_SHOWN_SOURCES`] of them
/// and counts the rest; it never shows a label.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RelationSources(pub Vec<(ElementId, RelationKind)>);

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
    /// The elements as the request has them so far, by ID. An element the
    /// request has not touched is read from the snapshot.
    elements: BTreeMap<ElementId, Element>,
    /// The elements the request has deleted. Their IDs stay taken.
    removed: BTreeSet<ElementId>,
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
            elements: BTreeMap::new(),
            removed: BTreeSet::new(),
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
                anchor,
            } => self.create_element(
                key,
                type_ref,
                name,
                summary.as_deref(),
                body,
                anchor.as_deref(),
            ),
            Operation::CreateBoard { key, title } => self.create_board(key, title),
            Operation::PlaceElement { element, board } => self.place_element(element, board),
            Operation::MoveElement {
                element,
                board,
                x,
                y,
            } => self.move_element(element, board, *x, *y),
            Operation::UpdateElement {
                element,
                name,
                type_ref,
                summary,
                body,
                anchor,
            } => self.update_element(
                element,
                name.as_deref(),
                type_ref.as_deref(),
                summary.as_ref().map(Option::as_deref),
                body.as_deref(),
                anchor.as_ref().map(Option::as_deref),
            ),
            Operation::CreateRelation {
                from,
                kind,
                to,
                label,
            } => self.create_relation(from, *kind, to, label.as_deref()),
            Operation::DeleteRelation { from, kind, to } => self.delete_relation(from, *kind, to),
            Operation::DeleteElement { element } => self.delete_element(element),
        }
    }

    fn create_element(
        &mut self,
        key: &LocalKey,
        type_ref: &str,
        name: &str,
        summary: Option<&str>,
        body: &str,
        anchor: Option<&str>,
    ) -> Result<(), OperationProblem> {
        self.check_new_key(key)?;
        let type_ref =
            TypeRef::try_from(type_ref.to_owned()).map_err(OperationProblem::InvalidElement)?;
        let anchor = anchor
            .map(|anchor| Anchor::try_from(anchor.to_owned()))
            .transpose()
            .map_err(OperationProblem::InvalidElement)?;
        let id =
            free_slug(name, "element", &self.taken_elements).ok_or(OperationProblem::NoFreeId)?;
        let element = Element::new(
            id.clone(),
            ElementFields {
                type_ref,
                name: name.to_owned(),
                summary: summary.map(str::to_owned),
                body: body.to_owned(),
                relations: Relations::new(),
                anchor,
                verified: None,
            },
        )
        .map_err(|error| OperationProblem::InvalidElement(error.problem().clone()))?;
        self.taken_elements.insert(id.clone());
        self.elements.insert(id.clone(), element);
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

    /// Changes the fields an update names and keeps the others, the ID and
    /// the relations included. An update that names no field is rejected.
    #[expect(
        clippy::option_option,
        reason = "a missing summary or anchor keeps it, `Some(None)` removes it, and a text replaces it"
    )]
    fn update_element(
        &mut self,
        element: &Ref<ElementId>,
        name: Option<&str>,
        type_ref: Option<&str>,
        summary: Option<Option<&str>>,
        body: Option<&str>,
        anchor: Option<Option<&str>>,
    ) -> Result<(), OperationProblem> {
        let id = self.resolve_element(element)?;
        if name.is_none()
            && type_ref.is_none()
            && summary.is_none()
            && body.is_none()
            && anchor.is_none()
        {
            return Err(OperationProblem::NothingToUpdate(id));
        }
        let type_ref = type_ref
            .map(|type_ref| TypeRef::try_from(type_ref.to_owned()))
            .transpose()
            .map_err(OperationProblem::InvalidElement)?;
        let anchor = anchor
            .map(|anchor| {
                anchor
                    .map(|anchor| Anchor::try_from(anchor.to_owned()))
                    .transpose()
            })
            .transpose()
            .map_err(OperationProblem::InvalidElement)?;
        self.change_element(&id, |fields| {
            if let Some(name) = name {
                name.clone_into(&mut fields.name);
            }
            if let Some(type_ref) = type_ref {
                fields.type_ref = type_ref;
            }
            if let Some(summary) = summary {
                fields.summary = summary.map(str::to_owned);
            }
            if let Some(body) = body {
                body.clone_into(&mut fields.body);
            }
            if let Some(anchor) = anchor {
                fields.anchor = anchor;
            }
            Ok(())
        })
    }

    fn create_relation(
        &mut self,
        from: &Ref<ElementId>,
        kind: RelationKind,
        to: &Ref<ElementId>,
        label: Option<&str>,
    ) -> Result<(), OperationProblem> {
        let from = self.resolve_element(from)?;
        let to = self.resolve_element(to)?;
        self.change_element(&from, |fields| {
            match fields.relations.entry((kind, to.clone())) {
                Entry::Occupied(_) => Err(OperationProblem::RelationExists {
                    from: from.clone(),
                    kind,
                    to: to.clone(),
                }),
                Entry::Vacant(entry) => {
                    entry.insert(label.map(str::to_owned));
                    Ok(())
                }
            }
        })
    }

    /// Removes a relation. An `existing` target is only looked up among the
    /// source's relations, so a relation whose target's file is gone can
    /// still be removed.
    fn delete_relation(
        &mut self,
        from: &Ref<ElementId>,
        kind: RelationKind,
        to: &Ref<ElementId>,
    ) -> Result<(), OperationProblem> {
        let from = self.resolve_element(from)?;
        let to = match to {
            Ref::Existing(id) => id.clone(),
            Ref::New(_) => self.resolve_element(to)?,
        };
        self.change_element(&from, |fields| {
            if fields.relations.remove(&(kind, to.clone())).is_some() {
                Ok(())
            } else {
                Err(OperationProblem::NoSuchRelation {
                    from: from.clone(),
                    kind,
                    to: to.clone(),
                })
            }
        })
    }

    /// Deletes an element: its file, its placement on every board, and its
    /// own relations. It is rejected while other elements still relate to
    /// it, as the request stands. An ID that a board places although its
    /// file is missing can be deleted too; then only its placements go.
    fn delete_element(&mut self, element: &Ref<ElementId>) -> Result<(), OperationProblem> {
        let id = self.resolve_deletion(element)?;
        let sources = self.incoming_relations(&id);
        if !sources.is_empty() {
            return Err(OperationProblem::StillRelated {
                element: id,
                sources: RelationSources(sources),
            });
        }
        for board in self.boards_placing(&id) {
            let (draft_board, layout) = self.draft_board(&board)?;
            draft_board.placements.remove(&id);
            *layout = Layout::of(draft_board);
        }
        self.homed.remove(&id);
        self.elements.remove(&id);
        self.removed.insert(id);
        Ok(())
    }

    /// The element a deletion names: an element of the request, or an
    /// `existing` ID that a board places although it has no element.
    fn resolve_deletion(&self, reference: &Ref<ElementId>) -> Result<ElementId, OperationProblem> {
        match reference {
            Ref::Existing(id)
                if self.element(id).is_none() && !self.boards_placing(id).is_empty() =>
            {
                Ok(id.clone())
            }
            Ref::Existing(_) | Ref::New(_) => self.resolve_element(reference),
        }
    }

    /// The boards that place `id`, as the request has them: the boards it
    /// has changed, and the snapshot's other boards.
    fn boards_placing(&self, id: &ElementId) -> Vec<BoardId> {
        let changed = self
            .boards
            .iter()
            .filter(|(_, (board, _))| board.placements.contains_key(id))
            .map(|(board_id, _)| board_id.clone());
        let untouched = self
            .snapshot
            .boards
            .iter()
            .filter(|(board_id, board)| {
                !self.boards.contains_key(*board_id) && board.placements.contains_key(id)
            })
            .map(|(board_id, _)| board_id.clone());
        changed.chain(untouched).collect()
    }

    /// The source and kind of every relation that points to `id`, as the
    /// request stands: the working set first, the snapshot for the others,
    /// without deleted elements; by source, then kind.
    fn incoming_relations(&self, id: &ElementId) -> Vec<(ElementId, RelationKind)> {
        let untouched = self.snapshot.elements.values().filter(|element| {
            !self.elements.contains_key(element.id()) && !self.removed.contains(element.id())
        });
        let sources: BTreeSet<(ElementId, RelationKind)> = self
            .elements
            .values()
            .chain(untouched)
            .filter(|source| source.id() != id)
            .flat_map(|source| {
                source
                    .relations()
                    .keys()
                    .filter(|(_, to)| to == id)
                    .map(|(kind, _)| (source.id().clone(), *kind))
            })
            .collect();
        sources.into_iter().collect()
    }

    /// Changes the fields of the element `id`, an element of the request,
    /// and keeps the element rebuilt through [`Element::new`] in the working
    /// set; its ID never changes. Every change removes the element's
    /// verification stamp, also one that leaves every value as it was: a
    /// stamp vouches only for the element as it was verified. An element is
    /// copied from the snapshot only the first time a request changes it.
    /// On an error the element may be gone from the working set; the whole
    /// request fails then anyway.
    fn change_element(
        &mut self,
        id: &ElementId,
        change: impl FnOnce(&mut ElementFields) -> Result<(), OperationProblem>,
    ) -> Result<(), OperationProblem> {
        let current = match self.elements.remove(id) {
            Some(element) => element,
            None => self
                .element(id)
                .cloned()
                .ok_or_else(|| OperationProblem::UnknownElement(id.clone()))?,
        };
        let mut fields = current.into_fields();
        change(&mut fields)?;
        fields.verified = None;
        let element = Element::new(id.clone(), fields)
            .map_err(|error| OperationProblem::InvalidElement(error.problem().clone()))?;
        self.elements.insert(id.clone(), element);
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

    /// The element with this ID as the request has it so far, if any.
    fn element(&self, id: &ElementId) -> Option<&Element> {
        if self.removed.contains(id) {
            return None;
        }
        self.elements
            .get(id)
            .or_else(|| self.snapshot.elements.get(id))
    }

    /// The ID a reference names: an `existing` one only names an element
    /// whose file exists before the request, a `new` one an element an
    /// earlier operation created. Either must still be an element of the
    /// request, which the one lookup in [`Self::element`] decides.
    fn resolve_element(&self, reference: &Ref<ElementId>) -> Result<ElementId, OperationProblem> {
        let id = match reference {
            Ref::Existing(id) if self.snapshot.elements.contains_key(id) => id.clone(),
            Ref::Existing(id) => return Err(OperationProblem::UnknownElement(id.clone())),
            Ref::New(key) => match self.keys.get(key) {
                Some(Created::Element(id)) => id.clone(),
                Some(Created::Board(_)) => {
                    return Err(OperationProblem::WrongKind {
                        key: key.clone(),
                        expected: Kind::Element,
                        actual: Kind::Board,
                    });
                }
                None => return Err(OperationProblem::UnknownKey(key.clone())),
            },
        };
        if self.element(&id).is_some() {
            Ok(id)
        } else {
            Err(OperationProblem::UnknownElement(id))
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

    /// The writes: the created elements in the order of their creation,
    /// unless deleted again, then the existing elements the request changed,
    /// then each changed board, then the files of deleted elements.
    fn finish(mut self) -> Applied {
        let created: Vec<Write> = self
            .created
            .iter()
            .filter_map(|(_, created)| match created {
                Created::Element(id) => self.elements.remove(id),
                Created::Board(_) => None,
            })
            .map(Write::Element)
            .collect();
        // Without the created ones, the working set holds exactly the
        // existing elements an operation changed.
        let changed = self.elements.into_values().map(Write::Element);
        let boards = self
            .boards
            .into_values()
            .map(|(board, _)| Write::Board(board));
        // Only a file that exists before the request is removed: never one
        // of an element without a file, nor one the request created.
        let snapshot = self.snapshot;
        let removals = self
            .removed
            .into_iter()
            .filter(|id| snapshot.elements.contains_key(id))
            .map(Write::RemoveElement);
        Applied {
            writes: created
                .into_iter()
                .chain(changed)
                .chain(boards)
                .chain(removals)
                .collect(),
            created: self.created,
        }
    }
}

impl fmt::Display for RelationSources {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let shown: Vec<String> = self
            .0
            .iter()
            .take(MAX_SHOWN_SOURCES)
            .map(|(source, kind)| format!("{source} ({kind})"))
            .collect();
        formatter.write_str(&shown.join(", "))?;
        let more = self.0.len().saturating_sub(MAX_SHOWN_SOURCES);
        if more > 0 {
            write!(formatter, ", and {more} more")?;
        }
        Ok(())
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
