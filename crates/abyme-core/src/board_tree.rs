//! The hierarchy of boards: whose inside each board is, where each element
//! has its home, and which board lies above another.
//!
//! [`BoardTree`] is built from the boards alone, so `apply` and the readers
//! answer every question about the hierarchy the same way. Files that a
//! merge or a hand edit wrote can hold an element that owns several boards,
//! an element placed on several boards, and boards that lie inside
//! themselves; the tree keeps all of them, and every walk up the hierarchy
//! stops at a board it has seen before, so no file can make it loop.

use std::collections::{BTreeMap, BTreeSet};

use abyme_model::{Board, BoardId, ElementId};

/// The hierarchy of a set of boards.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct BoardTree {
    /// The boards each element owns, by element.
    owned: BTreeMap<ElementId, BTreeSet<BoardId>>,
    /// The owner of each board that has one.
    owners: BTreeMap<BoardId, ElementId>,
    /// The home of each placed element: of several boards that place it,
    /// the one with the lowest ID.
    homes: BTreeMap<ElementId, BoardId>,
}

impl BoardTree {
    /// The hierarchy of `boards`, each given with its ID.
    pub fn of<'a>(boards: impl IntoIterator<Item = (&'a BoardId, &'a Board)>) -> Self {
        boards
            .into_iter()
            .fold(Self::default(), |mut tree, (id, board)| {
                if let Some(owner) = &board.owner {
                    tree.owned
                        .entry(owner.clone())
                        .or_default()
                        .insert(id.clone());
                    tree.owners.insert(id.clone(), owner.clone());
                }
                for element in board.placements.keys() {
                    tree.homes
                        .entry(element.clone())
                        .and_modify(|home| {
                            if id < home {
                                *home = id.clone();
                            }
                        })
                        .or_insert_with(|| id.clone());
                }
                tree
            })
    }

    /// The boards `element` owns, by ID.
    pub fn owned_by(&self, element: &ElementId) -> impl Iterator<Item = &BoardId> {
        self.owned.get(element).into_iter().flatten()
    }

    /// The element whose inside `board` is, if any.
    pub fn owner(&self, board: &BoardId) -> Option<&ElementId> {
        self.owners.get(board)
    }

    /// The home of `element`, if a board places it.
    pub fn home(&self, element: &ElementId) -> Option<&BoardId> {
        self.homes.get(element)
    }

    /// The board above `board`: the home of its owner, if it has an owner
    /// with a home.
    pub fn parent(&self, board: &BoardId) -> Option<&BoardId> {
        self.owner(board).and_then(|owner| self.home(owner))
    }

    /// Whether `board` is `ancestor` or lies inside it, going up from
    /// `board` parent by parent; a walk that comes back to a board it has
    /// seen ends without reaching `ancestor`.
    pub fn lies_inside(&self, board: &BoardId, ancestor: &BoardId) -> bool {
        let mut seen = BTreeSet::new();
        let mut current = Some(board);
        while let Some(next) = current {
            if next == ancestor {
                return true;
            }
            if !seen.insert(next) {
                return false;
            }
            current = self.parent(next);
        }
        false
    }

    /// Whether `board` lies inside itself: going up from the board above it
    /// comes back to it.
    pub fn lies_inside_itself(&self, board: &BoardId) -> bool {
        self.parent(board)
            .is_some_and(|parent| self.lies_inside(parent, board))
    }
}

#[cfg(test)]
mod tests;
