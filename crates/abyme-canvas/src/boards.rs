//! The list of a project's boards, as the UI shows it.

use abyme_core::{BoardTree, Snapshot};
use abyme_model::BoardId;
use serde::Serialize;

#[cfg(test)]
mod tests;

/// A board as the UI names it: its ID and the title shown for it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct BoardEntry {
    /// The board's ID.
    pub id: BoardId,
    /// The title shown for the board, as the board file holds it.
    pub title: String,
}

/// A board in the sidebar's list: its entry and how deep it lies in the tree
/// of inner boards, 0 for a board at the top level.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct ListedBoard {
    /// The board's ID and title.
    #[serde(flatten)]
    #[cfg_attr(test, ts(flatten))]
    pub entry: BoardEntry,
    /// How many boards lie above it.
    pub depth: usize,
}

/// The boards of `snapshot` as `overview` lists them: depth first, each
/// board followed by the boards inside it, every board once. A board whose
/// chain is broken, by a cycle or an owner without an element file, stands at
/// the top level.
pub fn board_list(snapshot: &Snapshot) -> Vec<ListedBoard> {
    BoardTree::of(&snapshot.boards)
        .depth_first(|owner| snapshot.elements.contains_key(owner))
        .into_iter()
        .filter_map(|(depth, board)| {
            entry(snapshot, board).map(|entry| ListedBoard { entry, depth })
        })
        .collect()
}

/// The ID and the title of `board`, and nothing else of it: what the UI may
/// see of a board in the list, in the breadcrumb, and on a box's buttons.
pub fn entry(snapshot: &Snapshot, board: &BoardId) -> Option<BoardEntry> {
    snapshot.boards.get(board).map(|found| BoardEntry {
        id: found.id.clone(),
        title: found.title.clone(),
    })
}
