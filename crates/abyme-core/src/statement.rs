//! The statement kind of each element: the kind of its home board's zone.
//!
//! An element takes its kind from its home alone. Without a home, or when
//! its home is no zone, it has none, and no other board that places it and
//! no board above stands in.

use std::collections::BTreeMap;

use abyme_model::{BoardId, ElementId, StatementKind};

use crate::apply::Snapshot;
use crate::board_tree::BoardTree;

/// The statement kind of every element with a file whose home is a zone, by
/// element.
#[must_use]
pub fn statement_kinds(snapshot: &Snapshot) -> BTreeMap<ElementId, StatementKind> {
    let tree = BoardTree::of(&snapshot.boards);
    snapshot
        .elements
        .keys()
        .filter_map(|id| kind_of(snapshot, &tree, id).map(|(kind, _)| (id.clone(), kind)))
        .collect()
}

/// The statement kind of `element` and the home board it comes from, if the
/// home is a zone. `tree` must have been built from `snapshot`'s boards.
pub fn kind_of<'a>(
    snapshot: &Snapshot,
    tree: &'a BoardTree,
    element: &ElementId,
) -> Option<(StatementKind, &'a BoardId)> {
    let home = tree.home(element)?;
    let zone = snapshot.boards.get(home)?.zone?;
    Some((zone.statement, home))
}

#[cfg(test)]
mod tests;
