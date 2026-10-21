//! Properties of the board file that hold for any board and any input.

use std::collections::BTreeMap;
use std::num::NonZeroU32;

use abyme_model::{Board, BoardId, ElementId, Rect};
use proptest::prelude::*;

/// Valid IDs: hyphen-joined words of lowercase letters and digits that pass
/// the ID check (which also drops the reserved device names).
fn id_text() -> impl Strategy<Value = String> {
    "[a-z0-9]{1,8}(-[a-z0-9]{1,8}){0,3}"
}

fn element_id() -> impl Strategy<Value = ElementId> {
    id_text().prop_filter_map("reserved device name", |text| text.parse().ok())
}

fn board_id() -> impl Strategy<Value = BoardId> {
    id_text().prop_filter_map("reserved device name", |text| text.parse().ok())
}

fn size() -> impl Strategy<Value = NonZeroU32> {
    (1..=u32::MAX).prop_filter_map("zero size", NonZeroU32::new)
}

fn rect() -> impl Strategy<Value = Rect> {
    (any::<i32>(), any::<i32>(), size(), size()).prop_map(|(x, y, width, height)| Rect {
        x,
        y,
        width,
        height,
    })
}

fn board() -> impl Strategy<Value = Board> {
    (
        board_id(),
        any::<String>(),
        proptest::collection::btree_map(element_id(), rect(), 0..8),
    )
        .prop_map(
            |(id, title, placements): (BoardId, String, BTreeMap<ElementId, Rect>)| Board {
                id,
                title,
                placements,
            },
        )
}

proptest! {
    #[test]
    fn reads_back_any_board_unchanged(board in board()) {
        let read = Board::from_json(&board.id, &board.to_json());

        prop_assert_eq!(read, Ok(board));
    }

    #[test]
    fn writes_each_placement_on_its_own_line(board in board()) {
        let json = board.to_json();

        let placement_lines = json
            .lines()
            .filter(|line| line.starts_with("    {\"element\": "))
            .count();
        let frame_lines = if board.placements.is_empty() { 6 } else { 7 };
        prop_assert_eq!(placement_lines, board.placements.len());
        prop_assert_eq!(json.lines().count(), board.placements.len() + frame_lines);
    }

    #[test]
    fn rejects_or_reads_any_text_without_panicking(text in any::<String>()) {
        let expected: BoardId = "main".parse().unwrap();

        let _ = Board::from_json(&expected, &text);
    }

    #[test]
    fn rejects_or_reads_a_damaged_board_file_without_panicking(
        board in board(),
        position in any::<prop::sample::Index>(),
        replacement in any::<char>(),
    ) {
        let mut chars: Vec<char> = board.to_json().chars().collect();
        let at = position.index(chars.len());
        chars[at] = replacement;
        let damaged: String = chars.into_iter().collect();

        let _ = Board::from_json(&board.id, &damaged);
    }
}
