//! What the UI shows of a board, computed from the core's snapshot by pure
//! functions.
//!
//! The projection names its fields one by one and carries no element's body
//! but a note's: the UI shows a box per placement, and only a note shows its
//! whole text.

use std::iter::successors;

use abyme_core::{BoardTree, ShownRelation, Snapshot, UnknownBoard, relations_on};
use abyme_model::{BoardId, Element, ElementId, Mode, RelationKind};
use serde::Serialize;

use crate::boards::BoardEntry;

#[cfg(test)]
mod tests;

/// A board as the UI shows it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct BoardView {
    /// The board's ID.
    pub id: BoardId,
    /// The title shown for the board.
    pub title: String,
    /// The board's mode, if it is a zone. The UI does not offer a drag on a
    /// mirrored board.
    pub mode: Option<Mode>,
    /// One entry per element on the board, in element-ID order.
    pub placements: Vec<Placement>,
    /// One entry per relation the board shows, as `read_board` shows them.
    pub relations: Vec<Relation>,
    /// The boards above this one, top first, as `overview` places them. It
    /// ends where the tree is broken, by a cycle or an owner without a file
    /// or a home.
    pub trail: Vec<BoardEntry>,
}

/// A relation the board shows, which the UI draws as an arrow.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct Relation {
    /// The element it goes out from.
    pub from: ElementId,
    /// Its kind.
    pub kind: RelationKind,
    /// The element it points to, whose file may be missing.
    pub to: ElementId,
    /// Its label, in full, if it has one.
    pub label: Option<String>,
}

/// An element on a board: where its box lies, how large it is, and what it
/// shows.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct Placement {
    /// The element's ID.
    pub element: ElementId,
    /// The left edge, from the board file.
    pub x: i32,
    /// The top edge, from the board file.
    pub y: i32,
    /// The width, from the board file.
    pub width: u32,
    /// The height, from the board file.
    pub height: u32,
    /// What the box shows.
    pub shown: Shown,
    /// The boards the element owns, by ID. More than one only after a merge
    /// or a hand edit.
    pub inner_boards: Vec<BoardEntry>,
    /// Whether the placement is pinned, from the board file.
    pub pinned: bool,
}

/// What the box of a placement shows.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Shown {
    /// The element's type, name, and summary.
    Element {
        /// The semantic type, such as `c4/container`.
        type_ref: String,
        /// The name.
        name: String,
        /// The one-line summary, if the element has one.
        summary: Option<String>,
    },
    /// A sticky note's whole text.
    Note {
        /// The first line of its text.
        name: String,
        /// The rest of its text.
        body: String,
    },
    /// The board places an element whose file is missing.
    Missing,
}

/// The projection of `board` in `snapshot`.
///
/// # Errors
///
/// Returns [`UnknownBoard`] when the snapshot has no such board.
pub fn project(snapshot: &Snapshot, board: &BoardId) -> Result<BoardView, UnknownBoard> {
    let found = snapshot
        .boards
        .get(board)
        .ok_or_else(|| UnknownBoard(board.clone()))?;
    let tree = BoardTree::of(&snapshot.boards);
    Ok(BoardView {
        id: found.id.clone(),
        title: found.title.clone(),
        mode: found.zone.map(|zone| zone.mode),
        placements: found
            .placements
            .iter()
            .map(|(element, placed)| placement(snapshot, &tree, element, placed))
            .collect(),
        relations: relations_on(snapshot, found).map(relation).collect(),
        trail: trail(snapshot, &tree, board),
    })
}

/// The entries of the boards above `board`, top first. A walk along
/// [`BoardTree::above`] cannot loop, since a board inside a cycle has none above.
fn trail(snapshot: &Snapshot, tree: &BoardTree, board: &BoardId) -> Vec<BoardEntry> {
    let has_file = |element: &ElementId| snapshot.elements.contains_key(element);
    let mut entries: Vec<BoardEntry> = successors(tree.above(board, has_file), |upper| {
        tree.above(upper, has_file)
    })
    .filter_map(|upper| entry(snapshot, upper))
    .collect();
    entries.reverse();
    entries
}

/// The ID and the title of `board`, and nothing else of it.
fn entry(snapshot: &Snapshot, board: &BoardId) -> Option<BoardEntry> {
    snapshot.boards.get(board).map(|found| BoardEntry {
        id: found.id.clone(),
        title: found.title.clone(),
    })
}

fn relation(shown: ShownRelation<'_>) -> Relation {
    Relation {
        from: shown.from.clone(),
        kind: shown.kind,
        to: shown.to.clone(),
        label: shown.label.map(str::to_owned),
    }
}

/// What a box shows of `element`.
fn shown(element: &Element) -> Shown {
    if element.is_note() {
        Shown::Note {
            name: element.name().to_owned(),
            body: element.body().to_owned(),
        }
    } else {
        Shown::Element {
            type_ref: element.type_ref().as_str().to_owned(),
            name: element.name().to_owned(),
            summary: element.summary().map(str::to_owned),
        }
    }
}

fn placement(
    snapshot: &Snapshot,
    tree: &BoardTree,
    id: &ElementId,
    placed: &abyme_model::Placement,
) -> Placement {
    let element: Option<&Element> = snapshot.elements.get(id);
    let rect = &placed.rect;
    Placement {
        element: id.clone(),
        x: rect.x,
        y: rect.y,
        width: rect.width.get(),
        height: rect.height.get(),
        shown: element.map_or(Shown::Missing, shown),
        inner_boards: tree
            .owned_by(id)
            .filter_map(|board| entry(snapshot, board))
            .collect(),
        pinned: placed.pinned,
    }
}
