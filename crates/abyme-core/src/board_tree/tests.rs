//! Tests of the board hierarchy.

use std::collections::{BTreeMap, BTreeSet};
use std::num::NonZeroU32;

use abyme_model::{Placement, Rect};
use proptest::prelude::*;
use rstest::rstest;

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
            zone: None,
            placements: elements
                .iter()
                .map(|element| {
                    (
                        element_id(element),
                        Placement {
                            rect,
                            pinned: false,
                        },
                    )
                })
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

#[rstest]
#[case::the_home_of_its_owner("inner", Some("top"))]
#[case::no_owner("top", None)]
#[case::an_owner_without_a_home("homeless", None)]
#[case::an_owner_without_a_file("fileless", None)]
#[case::inside_a_cycle("loop-a", None)]
fn says_which_board_lies_above_a_board(#[case] board_name: &str, #[case] above: Option<&str>) {
    let tree = tree(&[
        board("top", None, &["owner", "no-file"]),
        board("inner", Some("owner"), &[]),
        board("homeless", Some("nowhere"), &[]),
        board("fileless", Some("no-file"), &[]),
        board("loop-a", Some("in-b"), &["in-a"]),
        board("loop-b", Some("in-a"), &["in-b"]),
    ]);
    let has_file = |element: &ElementId| element.as_str() != "no-file";

    assert_eq!(
        tree.above(&board_id(board_name), has_file),
        above.map(board_id).as_ref()
    );
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

/// `a` is the inside of `x`, which lies on `b`, the inside of `y`, which lies
/// on `c`, the inside of `z`, which lies on `a`: a cycle of three. `s` lies
/// on `self`, its own inside. `below` is the inside of `w`, which lies on
/// `a`; `chain` is the inside of `v`, which lies on `top`; `loose` is the
/// inside of an element no board places.
fn cycles() -> BoardTree {
    tree(&[
        board("a", Some("x"), &["w", "z"]),
        board("b", Some("y"), &["x"]),
        board("c", Some("z"), &["y"]),
        board("self", Some("s"), &["s"]),
        board("below", Some("w"), &[]),
        board("top", None, &["v"]),
        board("chain", Some("v"), &[]),
        board("loose", Some("nowhere"), &[]),
    ])
}

#[rstest]
#[case::a_board_whose_owner_lies_on_it("self", true)]
#[case::each_board_of_a_cycle_of_three("a", true)]
#[case::the_last_board_of_a_cycle_of_three("c", true)]
#[case::a_board_below_a_cycle("below", false)]
#[case::a_board_below_a_top_level_board("chain", false)]
#[case::a_board_without_an_owner("top", false)]
#[case::a_board_whose_owner_has_no_home("loose", false)]
fn tells_whether_a_board_lies_inside_itself(#[case] board: &str, #[case] expected: bool) {
    assert_eq!(cycles().lies_inside_itself(&board_id(board)), expected);
}

#[test]
fn finds_a_cycle_of_two_boards_from_either_side() {
    // `left` is the inside of `r`, which lies on `right`, the inside of `l`,
    // which lies on `left`.
    let tree = tree(&[
        board("left", Some("r"), &["l"]),
        board("right", Some("l"), &["r"]),
    ]);

    assert!(tree.lies_inside_itself(&board_id("left")));
    assert!(tree.lies_inside_itself(&board_id("right")));
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

fn depth_first_of(tree: &BoardTree) -> Vec<(usize, &str)> {
    tree.depth_first(|_| true)
        .into_iter()
        .map(|(depth, board)| (depth, board.as_str()))
        .collect()
}

#[test]
fn lists_every_board_depth_first_with_the_inner_boards_by_id() {
    let mut boards = three_levels();
    boards.extend([
        board("deployment", Some("shop"), &[]),
        board("archive", None, &[]),
    ]);
    let tree = tree(&boards);

    assert_eq!(
        depth_first_of(&tree),
        [
            (0, "archive"),
            (0, "context"),
            (1, "containers"),
            (2, "components"),
            (1, "deployment"),
        ]
    );
}

#[test]
fn puts_a_board_below_the_lowest_board_its_owner_lies_on() {
    let tree = tree(&[
        board("zeta", None, &["x"]),
        board("alpha", None, &["x"]),
        board("inside", Some("x"), &[]),
    ]);

    assert_eq!(
        depth_first_of(&tree),
        [(0, "alpha"), (1, "inside"), (0, "zeta")]
    );
}

#[test]
fn lists_boards_out_of_the_tree_at_the_top_level_and_what_lies_below_them_inside() {
    // `left` and `right` lie inside each other, and `below` lies inside
    // `left`; `self` lies inside itself; `homeless` belongs to an element no
    // board places; `fileless` to `ghost`, which `top` places but which has
    // no file; `first` and `second` both belong to `twice`, on `top`.
    let tree = tree(&[
        board("left", Some("r"), &["l", "w"]),
        board("right", Some("l"), &["r"]),
        board("below", Some("w"), &[]),
        board("self", Some("s"), &["s"]),
        board("homeless", Some("loner"), &[]),
        board("top", None, &["ghost", "twice"]),
        board("fileless", Some("ghost"), &[]),
        board("first", Some("twice"), &[]),
        board("second", Some("twice"), &[]),
    ]);

    let order: Vec<(usize, &str)> = tree
        .depth_first(|owner| owner.as_str() != "ghost")
        .into_iter()
        .map(|(depth, board)| (depth, board.as_str()))
        .collect();

    assert_eq!(
        order,
        [
            (0, "fileless"),
            (0, "homeless"),
            (0, "left"),
            (1, "below"),
            (0, "right"),
            (0, "self"),
            (0, "top"),
            (1, "first"),
            (1, "second"),
        ]
    );
}

/// Board `b{n}` is the inside of `e{n}`, which lies on `b{n+1}`. When the
/// chain is `closed`, `e9999` lies on `b0`, which makes it a cycle.
fn ten_thousand_boards(closed: bool) -> Vec<(BoardId, Board)> {
    (0..10_000_usize)
        .map(|index| {
            let placed: Vec<String> = match index.checked_sub(1) {
                Some(below) => vec![format!("e{below}")],
                None if closed => vec!["e9999".to_owned()],
                None => Vec::new(),
            };
            let placed: Vec<&str> = placed.iter().map(String::as_str).collect();
            board(&format!("b{index}"), Some(&format!("e{index}")), &placed)
        })
        .collect()
}

#[test]
fn lists_a_chain_and_a_cycle_of_ten_thousand_boards_without_recursion() {
    let chain = tree(&ten_thousand_boards(false));
    let cycle = tree(&ten_thousand_boards(true));

    let chain_order = chain.depth_first(|_| true);
    let cycle_order = cycle.depth_first(|_| true);

    assert_eq!(chain_order.len(), 10_000);
    assert_eq!(chain_order.first(), Some(&(0, &board_id("b9999"))));
    assert_eq!(chain_order.last(), Some(&(9_999, &board_id("b0"))));
    assert_eq!(cycle_order.len(), 10_000);
    assert!(cycle_order.iter().all(|&(depth, _)| depth == 0));
    assert!(cycle.lies_inside_itself(&board_id("b0")));
    assert!(!chain.lies_inside_itself(&board_id("b0")));
}

/// Up to eight boards `b0`… with any owner among `e0` to `e5` or none, each
/// placing any of those elements.
fn arbitrary_boards() -> impl Strategy<Value = Vec<(BoardId, Board)>> {
    proptest::collection::vec(
        (
            proptest::option::of(0..6_usize),
            proptest::collection::btree_set(0..6_usize, 0..3),
        ),
        0..9,
    )
    .prop_map(|specs| {
        specs
            .into_iter()
            .enumerate()
            .map(|(index, (owner, placed))| {
                let owner = owner.map(|owner| format!("e{owner}"));
                let placed: Vec<String> =
                    placed.iter().map(|placed| format!("e{placed}")).collect();
                let placed: Vec<&str> = placed.iter().map(String::as_str).collect();
                board(&format!("b{index}"), owner.as_deref(), &placed)
            })
            .collect()
    })
}

proptest! {
    #[test]
    fn lists_every_board_once_below_the_board_above_it_unless_it_is_out_of_the_tree(
        boards in arbitrary_boards(),
        with_file in proptest::collection::btree_set(0..6_usize, 0..7),
    ) {
        let with_file: BTreeSet<String> = with_file.iter().map(|index| format!("e{index}")).collect();
        let has_file = |owner: &ElementId| with_file.contains(owner.as_str());
        let tree = tree(&boards);

        let order = tree.depth_first(has_file);

        let listed: BTreeSet<&BoardId> = order.iter().map(|&(_, board)| board).collect();
        let all: BTreeSet<&BoardId> = boards.iter().map(|(id, _)| id).collect();
        prop_assert_eq!(order.len(), boards.len());
        prop_assert_eq!(listed, all);
        let depths: BTreeMap<&BoardId, usize> =
            order.iter().map(|&(depth, board)| (board, depth)).collect();
        for (id, _) in &boards {
            let walked = tree.parent(id).is_some_and(|parent| tree.lies_inside(parent, id));
            prop_assert_eq!(tree.lies_inside_itself(id), walked, "{}", id);
            let top_level = tree.parent(id).is_none()
                || walked
                || tree.owner(id).is_some_and(|owner| !has_file(owner));
            let expected = if top_level {
                Some(0)
            } else {
                tree.parent(id).and_then(|parent| depths.get(parent)).map(|depth| depth + 1)
            };
            prop_assert_eq!(depths.get(id).copied(), expected, "{}", id);
        }
    }
}
