//! The statement kind of each element: its own kind if it holds one, otherwise
//! the kind of its home board's zone.
//!
//! Without an own kind and without a home, or when its home is no zone, an
//! element has none, and no other board that places it and no board above
//! stands in. This is the one place that decides an element's kind.

use std::collections::BTreeMap;

use abyme_model::{BoardId, Element, ElementId, StatementKind};

use crate::apply::Snapshot;
use crate::board_tree::BoardTree;

/// Where an element's statement kind comes from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KindSource<'a> {
    /// The element holds it itself.
    Element,
    /// It is the kind of the zone of this board, the element's home.
    Home(&'a BoardId),
}

/// The statement kind of every element with a file that has one, by element.
#[must_use]
pub fn statement_kinds(snapshot: &Snapshot) -> BTreeMap<ElementId, StatementKind> {
    let tree = BoardTree::of(&snapshot.boards);
    snapshot
        .elements
        .iter()
        .filter_map(|(id, element)| {
            kind_of(snapshot, &tree, element).map(|(kind, _)| (id.clone(), kind))
        })
        .collect()
}

/// The statement kind of `element` and where it comes from, if it has one.
/// `tree` must have been built from `snapshot`'s boards.
pub fn kind_of<'a>(
    snapshot: &Snapshot,
    tree: &'a BoardTree,
    element: &Element,
) -> Option<(StatementKind, KindSource<'a>)> {
    if let Some(kind) = element.statement() {
        return Some((kind, KindSource::Element));
    }
    let home = tree.home(element.id())?;
    let zone = snapshot.boards.get(home)?.zone?;
    Some((zone.statement, KindSource::Home(home)))
}

#[cfg(test)]
mod tests;
