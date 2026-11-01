//! The hierarchy of boards: whose inside each board is, where each element
//! has its home, and which board lies above another.
//!
//! [`BoardTree`] is built from the boards alone, so `apply` and the readers
//! answer every question about the hierarchy the same way. Files that a
//! merge or a hand edit wrote can hold an element that owns several boards,
//! an element placed on several boards, and boards that lie inside
//! themselves; the tree keeps all of them, and no file can make a walk up
//! the hierarchy loop: a walk along [`BoardTree::above`] ends at a board inside
//! a cycle, which has none above it, and a walk along the owners' homes inside
//! this crate stops at a board it has seen before. The
//! boards that lie inside themselves are found once, when the tree is
//! built, in a time that grows with the number of boards alone.

use std::collections::{BTreeMap, BTreeSet};

use abyme_model::{Board, BoardId, ElementId};

/// The hierarchy of a set of boards.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct BoardTree {
    /// Every board, by ID.
    boards: BTreeSet<BoardId>,
    /// The boards each element owns, by element.
    owned: BTreeMap<ElementId, BTreeSet<BoardId>>,
    /// The owner of each board that has one.
    owners: BTreeMap<BoardId, ElementId>,
    /// The home of each placed element: of several boards that place it,
    /// the one with the lowest ID.
    homes: BTreeMap<ElementId, BoardId>,
    /// The boards that lie inside themselves.
    cycles: BTreeSet<BoardId>,
}

impl BoardTree {
    /// The hierarchy of `boards`, each given with its ID.
    pub fn of<'a>(boards: impl IntoIterator<Item = (&'a BoardId, &'a Board)>) -> Self {
        let tree = boards
            .into_iter()
            .fold(Self::default(), |mut tree, (id, board)| {
                tree.boards.insert(id.clone());
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
            });
        let cycles = tree.boards_inside_themselves();
        Self { cycles, ..tree }
    }

    /// The boards that lie inside themselves, found in one pass: each walk
    /// up from a board not yet reached stops at a board an earlier walk
    /// reached, or at one this walk reached before, which closes a cycle of
    /// the boards from there on. Every board is walked over once.
    fn boards_inside_themselves(&self) -> BTreeSet<BoardId> {
        let mut reached: BTreeSet<&BoardId> = BTreeSet::new();
        let mut cycles = BTreeSet::new();
        for start in &self.boards {
            let mut path: Vec<&BoardId> = Vec::new();
            let mut positions: BTreeMap<&BoardId, usize> = BTreeMap::new();
            let mut current = Some(start);
            while let Some(board) = current {
                if reached.contains(board) {
                    break;
                }
                if let Some(&position) = positions.get(board) {
                    cycles.extend(path.iter().skip(position).map(|&walked| walked.clone()));
                    break;
                }
                positions.insert(board, path.len());
                path.push(board);
                current = self.parent(board);
            }
            reached.extend(path);
        }
        cycles
    }

    /// The boards `element` owns, by ID.
    pub fn owned_by(&self, element: &ElementId) -> impl Iterator<Item = &BoardId> {
        self.owned.get(element).into_iter().flatten()
    }

    /// The element whose inside `board` is, if any.
    #[must_use]
    pub fn owner(&self, board: &BoardId) -> Option<&ElementId> {
        self.owners.get(board)
    }

    /// The home of `element`, if a board places it.
    #[must_use]
    pub fn home(&self, element: &ElementId) -> Option<&BoardId> {
        self.homes.get(element)
    }

    /// The board above `board`: the home of its owner, if it has an owner
    /// with a home.
    pub(crate) fn parent(&self, board: &BoardId) -> Option<&BoardId> {
        self.owner(board).and_then(|owner| self.home(owner))
    }

    /// Whether `board` is `ancestor` or lies inside it, going up from
    /// `board` parent by parent; a walk that comes back to a board it has
    /// seen ends without reaching `ancestor`.
    #[must_use]
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
    #[must_use]
    pub fn lies_inside_itself(&self, board: &BoardId) -> bool {
        self.cycles.contains(board)
    }

    /// The board above `board` in the tree readers show: the home of its owner.
    /// `None` for a board without an owner, one that lies inside itself, one
    /// whose owner has no element file, as `has_file` tells, and one whose owner
    /// has no home. Unlike [`Self::parent`], it never follows a cycle.
    pub fn above(
        &self,
        board: &BoardId,
        has_file: impl Fn(&ElementId) -> bool,
    ) -> Option<&BoardId> {
        self.owner(board)
            .filter(|owner| has_file(owner) && !self.lies_inside_itself(board))
            .and_then(|owner| self.home(owner))
    }

    /// Every board once, depth first, with its depth: the top-level boards
    /// by ID, each followed by the boards inside it, by ID, and theirs. A
    /// board is top-level when no board lies above it, when it lies inside
    /// itself, or when its owner has no element file, as `has_file` tells;
    /// every other board lies inside the board above it. Cutting those links
    /// leaves no cycle, so every board is reached from a top-level board.
    pub fn depth_first(&self, has_file: impl Fn(&ElementId) -> bool) -> Vec<(usize, &BoardId)> {
        let mut top_level: Vec<&BoardId> = Vec::new();
        let mut inside: BTreeMap<&BoardId, Vec<&BoardId>> = BTreeMap::new();
        for board in &self.boards {
            match self.above(board, &has_file) {
                Some(above) => inside.entry(above).or_default().push(board),
                None => top_level.push(board),
            }
        }
        let mut stack: Vec<(usize, &BoardId)> = top_level
            .into_iter()
            .rev()
            .map(|board| (0, board))
            .collect();
        let mut seen: BTreeSet<&BoardId> = BTreeSet::new();
        let mut order = Vec::with_capacity(self.boards.len());
        while let Some((depth, board)) = stack.pop() {
            if !seen.insert(board) {
                continue;
            }
            order.push((depth, board));
            let below = inside.get(board).into_iter().flatten().rev();
            stack.extend(below.map(|&child| (depth + 1, child)));
        }
        order
    }
}

#[cfg(test)]
mod tests;
