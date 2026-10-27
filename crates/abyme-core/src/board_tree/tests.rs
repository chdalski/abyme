//! Tests of the board hierarchy.

use std::collections::BTreeMap;
use std::num::NonZeroU32;

use abyme_model::Rect;

use super::*;

fn board_id(value: &str) -> BoardId {
    value.parse().unwrap()
}

fn element_id(value: &str) -> ElementId {
    value.parse().unwrap()
}

/// The board `id`, the inside of `owner`, placing `elements`.
fn board(id: &str, owner: Option<&str>, elements: &[&str]) -> (BoardId, Board) {
    let rect = Rect {
        x: 0,
        y: 0,
        width: NonZeroU32::new(1).unwrap(),
        height: NonZeroU32::new(1).unwrap(),
    };
    (
        board_id(id),
        Board {
            id: board_id(id),
            title: id.to_owned(),
            owner: owner.map(element_id),
            summary: None,
            placements: elements
                .iter()
                .map(|element| (element_id(element), rect))
                .collect(),
        },
    )
}

fn tree(boards: &[(BoardId, Board)]) -> BoardTree {
    BoardTree::of(boards.iter().map(|(id, board)| (id, board)))
}

/// Context places `shop`, whose inside is `containers`, which places
/// `payment`, whose inside is `components`.
fn three_levels() -> Vec<(BoardId, Board)> {
    vec![
        board("context", None, &["shop"]),
        board("containers", Some("shop"), &["payment"]),
        board("components", Some("payment"), &["api"]),
    ]
}

#[test]
fn names_the_owner_home_and_parent_of_each_board() {
    let tree = tree(&three_levels());

    assert_eq!(
        tree.owner(&board_id("components")),
        Some(&element_id("payment"))
    );
    assert_eq!(tree.owner(&board_id("context")), None);
    assert_eq!(
        tree.home(&element_id("payment")),
        Some(&board_id("containers"))
    );
    assert_eq!(
        tree.parent(&board_id("components")),
        Some(&board_id("containers"))
    );
    assert_eq!(
        tree.parent(&board_id("containers")),
        Some(&board_id("context"))
    );
    assert_eq!(tree.parent(&board_id("context")), None);
}

#[test]
fn keeps_every_board_an_element_owns() {
    let tree = tree(&[
        board("a", Some("x"), &[]),
        board("b", Some("x"), &[]),
        board("c", Some("y"), &[]),
    ]);

    let owned: Vec<&BoardId> = tree.owned_by(&element_id("x")).collect();

    assert_eq!(owned, [&board_id("a"), &board_id("b")]);
    assert_eq!(tree.owned_by(&element_id("z")).count(), 0);
}

#[test]
fn takes_the_lowest_board_id_as_the_home_of_an_element_placed_twice() {
    let boards = [board("zeta", None, &["x"]), board("alpha", None, &["x"])];

    let home = tree(&boards).home(&element_id("x")).cloned();

    assert_eq!(home, Some(board_id("alpha")));
}

#[test]
fn has_no_parent_for_an_owner_without_a_home() {
    let tree = tree(&[board("inside", Some("homeless"), &[])]);

    assert_eq!(tree.parent(&board_id("inside")), None);
}

#[test]
fn finds_a_board_inside_another_at_any_depth() {
    let tree = tree(&three_levels());

    assert!(tree.lies_inside(&board_id("components"), &board_id("context")));
    assert!(tree.lies_inside(&board_id("components"), &board_id("containers")));
    assert!(tree.lies_inside(&board_id("context"), &board_id("context")));
    assert!(!tree.lies_inside(&board_id("context"), &board_id("components")));
}

#[test]
fn ends_a_walk_in_a_cycle_that_does_not_hold_the_board_it_looks_for() {
    // `a` and `b` lie inside each other; `c` lies inside `a`.
    let tree = tree(&[
        board("a", Some("y"), &["x"]),
        board("b", Some("x"), &["y"]),
        board("c", Some("z"), &[]),
        board("d", None, &["z"]),
    ]);

    assert!(tree.lies_inside(&board_id("a"), &board_id("b")));
    assert!(!tree.lies_inside(&board_id("a"), &board_id("c")));
    assert!(!tree.lies_inside(&board_id("a"), &board_id("d")));
}

#[test]
fn walks_a_chain_of_ten_thousand_boards_without_recursion() {
    // Board `b{n}` is the inside of element `e{n}`, which lies on board
    // `b{n+1}`.
    let boards: Vec<(BoardId, Board)> = (0..10_000)
        .map(|index: usize| {
            let id = format!("b{index}");
            let owner = format!("e{index}");
            let placed: Vec<String> = index
                .checked_sub(1)
                .map(|below| format!("e{below}"))
                .into_iter()
                .collect();
            let placed: Vec<&str> = placed.iter().map(String::as_str).collect();
            board(&id, Some(&owner), &placed)
        })
        .collect();
    let tree = tree(&boards);

    assert!(tree.lies_inside(&board_id("b0"), &board_id("b9999")));
    assert!(!tree.lies_inside(&board_id("b0"), &board_id("missing")));
}

#[test]
fn builds_the_same_tree_from_boards_in_any_order() {
    let mut boards = three_levels();
    let forward = tree(&boards);
    boards.reverse();

    assert_eq!(tree(&boards), forward);
    assert_eq!(
        BoardTree::of(BTreeMap::<BoardId, Board>::new().iter()),
        BoardTree::default()
    );
}
