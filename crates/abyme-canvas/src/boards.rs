//! The list of a project's boards, as the UI shows it.

use abyme_core::Snapshot;
use abyme_model::BoardId;
use serde::Serialize;

#[cfg(test)]
mod tests;

/// A board in the list: its ID and the title shown for it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct BoardEntry {
    /// The board's ID.
    pub id: BoardId,
    /// The title shown for the board, as the board file holds it.
    pub title: String,
}

/// The boards of `snapshot`, in board-ID order.
pub fn board_list(snapshot: &Snapshot) -> Vec<BoardEntry> {
    snapshot
        .boards
        .keys()
        .filter_map(|board| entry(snapshot, board))
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
