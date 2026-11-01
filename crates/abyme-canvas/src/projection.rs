//! What the UI shows of a board, computed from the core's snapshot by pure
//! functions.
//!
//! The projection names its fields one by one and never carries an
//! element's body: the UI shows a box per placement, and the body is not
//! part of it.

use abyme_core::{ShownRelation, Snapshot, UnknownBoard, relations_on};
use abyme_model::{BoardId, Element, ElementId, Mode, Rect, RelationKind};
use serde::Serialize;

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
    Ok(BoardView {
        id: found.id.clone(),
        title: found.title.clone(),
        mode: found.zone.map(|zone| zone.mode),
        placements: found
            .placements
            .iter()
            .map(|(element, placed)| {
                placement(element, &placed.rect, snapshot.elements.get(element))
            })
            .collect(),
        relations: relations_on(snapshot, found).map(relation).collect(),
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

fn placement(id: &ElementId, rect: &Rect, element: Option<&Element>) -> Placement {
    Placement {
        element: id.clone(),
        x: rect.x,
        y: rect.y,
        width: rect.width.get(),
        height: rect.height.get(),
        shown: element.map_or(Shown::Missing, |element| Shown::Element {
            type_ref: element.type_ref().as_str().to_owned(),
            name: element.name().to_owned(),
            summary: element.summary().map(str::to_owned),
        }),
    }
}
