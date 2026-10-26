//! Tests of turning operations into file writes.

use super::*;
use std::num::NonZeroU32;

use abyme_model::{Rect, hides_or_breaks_text};
use rstest::rstest;

use crate::operation::Operations;

fn key(value: &str) -> LocalKey {
    LocalKey::try_from(value.to_owned()).unwrap()
}

fn element_id(value: &str) -> ElementId {
    value.parse().unwrap()
}

fn board_id(value: &str) -> BoardId {
    value.parse().unwrap()
}

fn rect(x: i32, y: i32) -> Rect {
    Rect {
        x,
        y,
        width: NonZeroU32::new(180).unwrap(),
        height: NonZeroU32::new(90).unwrap(),
    }
}

fn element(id: &str) -> Element {
    Element::new(
        element_id(id),
        ElementFields {
            type_ref: TypeRef::try_from("t".to_owned()).unwrap(),
            name: id.to_owned(),
            summary: None,
            body: String::new(),
            relations: Relations::new(),
            anchor: None,
            verified: None,
        },
    )
    .unwrap()
}

/// A snapshot with the elements `orders-db` (placed on `main`) and `loose`
/// (on no board), the board `main`, and an orphan placement `ghost` whose
/// element file is missing.
fn snapshot() -> Snapshot {
    let main = Board {
        id: board_id("main"),
        title: "Main".to_owned(),
        placements: BTreeMap::from([
            (element_id("orders-db"), rect(0, 0)),
            (element_id("ghost"), rect(220, 0)),
        ]),
    };
    Snapshot {
        elements: BTreeMap::from([
            (element_id("orders-db"), element("orders-db")),
            (element_id("loose"), element("loose")),
        ]),
        boards: BTreeMap::from([(board_id("main"), main)]),
    }
}

fn operations(json: &str) -> Vec<Operation> {
    serde_json::from_str::<Operations>(json).unwrap().into_vec()
}

fn problem(snapshot: &Snapshot, json: &str) -> OperationProblem {
    match apply(snapshot, &operations(json)) {
        Err(ApplyError::Operation { problem, .. }) => problem,
        other => panic!("expected an operation error, got {other:?}"),
    }
}

fn written_board<'a>(applied: &'a Applied, id: &str) -> &'a Board {
    applied
        .writes
        .iter()
        .find_map(|write| match write {
            Write::Board(board) if board.id.as_str() == id => Some(board),
            Write::Board(_) | Write::Element(_) | Write::RemoveElement(_) => None,
        })
        .unwrap()
}

#[test]
fn creates_and_places_in_one_request() {
    let json = r#"[
        {"op": "create_board", "key": "b", "title": "Container Level"},
        {"op": "create_element", "key": "pay", "type_ref": "c4/container", "name": "Payment Service"},
        {"op": "place_element", "element": {"new": "pay"}, "board": {"new": "b"}}
    ]"#;

    let applied = apply(&snapshot(), &operations(json)).unwrap();

    assert_eq!(
        applied.created,
        vec![
            (key("b"), Created::Board(board_id("container-level"))),
            (key("pay"), Created::Element(element_id("payment-service"))),
        ]
    );
    let board = written_board(&applied, "container-level");
    assert_eq!(
        board.placements.get(&element_id("payment-service")),
        Some(&rect(0, 0))
    );
}

#[test]
fn an_empty_request_writes_nothing() {
    let applied = apply(&snapshot(), &[]).unwrap();

    assert_eq!(applied.writes, Vec::new());
    assert_eq!(applied.created, Vec::new());
}

#[test]
fn writes_elements_before_boards() {
    let json = r#"[
        {"op": "create_board", "key": "b", "title": "B"},
        {"op": "create_element", "key": "e", "type_ref": "t", "name": "E"},
        {"op": "place_element", "element": {"new": "e"}, "board": {"new": "b"}}
    ]"#;

    let applied = apply(&snapshot(), &operations(json)).unwrap();

    assert!(matches!(
        applied.writes.as_slice(),
        [Write::Element(_), Write::Board(_)]
    ));
}

#[test]
fn writes_new_elements_in_the_order_of_their_creation() {
    let json = r#"[
        {"op": "create_element", "key": "z", "type_ref": "t", "name": "Zeta"},
        {"op": "create_element", "key": "a", "type_ref": "t", "name": "Alpha"},
        {"op": "create_board", "key": "b", "title": "B"}
    ]"#;

    let applied = apply(&snapshot(), &operations(json)).unwrap();

    let written: Vec<&str> = applied
        .writes
        .iter()
        .map(|write| match write {
            Write::Element(element) => element.id().as_str(),
            Write::Board(board) => board.id.as_str(),
            Write::RemoveElement(id) => id.as_str(),
        })
        .collect();
    assert_eq!(written, ["zeta", "alpha", "b"]);
    assert!(matches!(
        applied.writes.as_slice(),
        [Write::Element(_), Write::Element(_), Write::Board(_)]
    ));
}

#[test]
fn keeps_every_existing_placement_and_the_title_of_a_changed_board() {
    let json = r#"[{"op": "place_element", "element": {"existing": "loose"}, "board": {"existing": "main"}}]"#;

    let applied = apply(&snapshot(), &operations(json)).unwrap();

    let board = written_board(&applied, "main");
    let before = &snapshot().boards[&board_id("main")];
    assert_eq!(board.title, before.title);
    for (id, rect) in &before.placements {
        assert_eq!(board.placements.get(id), Some(rect));
    }
    assert_eq!(board.placements.len(), before.placements.len() + 1);
    assert_eq!(applied.writes.len(), 1);
}

#[test]
fn places_two_elements_of_one_request_without_overlap() {
    let json = r#"[
        {"op": "create_element", "key": "a", "type_ref": "t", "name": "A"},
        {"op": "create_element", "key": "b", "type_ref": "t", "name": "B"},
        {"op": "place_element", "element": {"new": "a"}, "board": {"existing": "main"}},
        {"op": "place_element", "element": {"new": "b"}, "board": {"existing": "main"}}
    ]"#;

    let applied = apply(&snapshot(), &operations(json)).unwrap();

    let board = written_board(&applied, "main");
    assert_eq!(board.placements.get(&element_id("a")), Some(&rect(440, 0)));
    assert_eq!(board.placements.get(&element_id("b")), Some(&rect(660, 0)));
}

/// `snapshot()` with `orders-db` sized 300 × 50 and a second board `side`
/// on which `loose` lies.
fn snapshot_with_two_boards() -> Snapshot {
    let mut snapshot = snapshot();
    snapshot
        .boards
        .get_mut(&board_id("main"))
        .unwrap()
        .placements
        .insert(element_id("orders-db"), sized(0, 0, 300, 50));
    snapshot.boards.insert(
        board_id("side"),
        Board {
            id: board_id("side"),
            title: "Side".to_owned(),
            placements: BTreeMap::from([(element_id("loose"), rect(0, 0))]),
        },
    );
    snapshot
}

fn sized(x: i32, y: i32, width: u32, height: u32) -> Rect {
    Rect {
        x,
        y,
        width: NonZeroU32::new(width).unwrap(),
        height: NonZeroU32::new(height).unwrap(),
    }
}

#[test]
fn moves_only_the_position_of_that_placement() {
    let json = r#"[{"op": "move_element", "element": {"existing": "orders-db"}, "board": {"existing": "main"}, "x": 500, "y": -300}]"#;
    let before = snapshot_with_two_boards();

    let applied = apply(&before, &operations(json)).unwrap();

    assert!(matches!(applied.writes.as_slice(), [Write::Board(_)]));
    let board = written_board(&applied, "main");
    let mut expected = before.boards[&board_id("main")].clone();
    expected
        .placements
        .insert(element_id("orders-db"), sized(500, -300, 300, 50));
    assert_eq!(board, &expected);
    assert_eq!(applied.created, Vec::new());
}

#[rstest]
#[case::far_right_and_down(1_000_000, 1_000_000)]
#[case::far_left_and_up(-1_000_000, -1_000_000)]
fn accepts_a_move_to_the_ends_of_the_range(#[case] x: i32, #[case] y: i32) {
    let json = format!(
        r#"[{{"op": "move_element", "element": {{"existing": "orders-db"}}, "board": {{"existing": "main"}}, "x": {x}, "y": {y}}}]"#
    );

    let applied = apply(&snapshot(), &operations(&json)).unwrap();

    let board = written_board(&applied, "main");
    assert_eq!(
        board.placements.get(&element_id("orders-db")),
        Some(&rect(x, y))
    );
}

#[test]
fn moves_an_element_that_the_request_creates_and_places() {
    let json = r#"[
        {"op": "create_board", "key": "b", "title": "B"},
        {"op": "create_element", "key": "e", "type_ref": "t", "name": "E"},
        {"op": "place_element", "element": {"new": "e"}, "board": {"new": "b"}},
        {"op": "move_element", "element": {"new": "e"}, "board": {"new": "b"}, "x": 1000, "y": -50}
    ]"#;

    let applied = apply(&snapshot(), &operations(json)).unwrap();

    let board = written_board(&applied, "b");
    assert_eq!(
        board.placements.get(&element_id("e")),
        Some(&rect(1000, -50))
    );
}

#[test]
fn keeps_the_later_of_two_moves() {
    let json = r#"[
        {"op": "move_element", "element": {"existing": "orders-db"}, "board": {"existing": "main"}, "x": 10, "y": 20},
        {"op": "move_element", "element": {"existing": "orders-db"}, "board": {"existing": "main"}, "x": 30, "y": 40}
    ]"#;

    let applied = apply(&snapshot(), &operations(json)).unwrap();

    let board = written_board(&applied, "main");
    assert_eq!(
        board.placements.get(&element_id("orders-db")),
        Some(&rect(30, 40))
    );
}

#[test]
fn places_no_element_on_a_free_cell_that_a_move_took() {
    let json = r#"[
        {"op": "move_element", "element": {"existing": "orders-db"}, "board": {"existing": "main"}, "x": 440, "y": 0},
        {"op": "place_element", "element": {"existing": "loose"}, "board": {"existing": "main"}}
    ]"#;

    let applied = apply(&snapshot(), &operations(json)).unwrap();

    let board = written_board(&applied, "main");
    assert_eq!(
        board.placements.get(&element_id("orders-db")),
        Some(&rect(440, 0))
    );
    assert_eq!(
        board.placements.get(&element_id("loose")),
        Some(&rect(660, 0))
    );
}

#[test]
fn places_no_element_where_a_move_went_below_the_content() {
    let mut walled = snapshot();
    let wall = sized(0, 0, 4 * 220, 64 * 130);
    walled
        .boards
        .get_mut(&board_id("main"))
        .unwrap()
        .placements
        .insert(element_id("wall"), wall);
    // Without the move, the next placement would go to y 8320 + 40.
    let json = r#"[
        {"op": "move_element", "element": {"existing": "orders-db"}, "board": {"existing": "main"}, "x": 0, "y": 8360},
        {"op": "place_element", "element": {"existing": "loose"}, "board": {"existing": "main"}}
    ]"#;

    let applied = apply(&walled, &operations(json)).unwrap();

    let board = written_board(&applied, "main");
    assert_eq!(
        board.placements.get(&element_id("loose")),
        Some(&rect(0, 8360 + 90 + 40))
    );
}

/// The element covers every grid cell before it moves, and its bottom
/// after the move lies beyond `i32`, so no position is left; the request
/// fails without a panic.
#[test]
fn reports_a_full_board_after_moving_an_element_of_the_largest_size() {
    let mut large = snapshot();
    large
        .boards
        .get_mut(&board_id("main"))
        .unwrap()
        .placements
        .insert(element_id("orders-db"), sized(0, 0, u32::MAX, u32::MAX));
    let json = r#"[
        {"op": "move_element", "element": {"existing": "orders-db"}, "board": {"existing": "main"}, "x": 1000000, "y": 1000000},
        {"op": "place_element", "element": {"existing": "loose"}, "board": {"existing": "main"}}
    ]"#;

    assert_eq!(
        apply(&large, &operations(json)),
        Err(ApplyError::Operation {
            position: 2,
            problem: OperationProblem::BoardFull(board_id("main"))
        })
    );
}

#[rstest]
#[case::not_on_the_board(
    r#"[{"op": "move_element", "element": {"existing": "loose"}, "board": {"existing": "main"}, "x": 0, "y": 0}]"#,
    "element loose does not lie on board main"
)]
#[case::out_of_range(
    r#"[{"op": "move_element", "element": {"existing": "orders-db"}, "board": {"existing": "main"}, "x": 1000001, "y": -5}]"#,
    "the position x 1000001, y -5 lies outside -1000000 to 1000000"
)]
fn names_only_ids_and_numbers_when_it_rejects_a_move(#[case] json: &str, #[case] expected: &str) {
    let mut marked = snapshot();
    marked.boards.get_mut(&board_id("main")).unwrap().title = "MARKER \u{1b}[31m".to_owned();

    assert_eq!(problem(&marked, json).to_string(), expected);
}

#[test]
fn gives_colliding_names_suffixes() {
    let json = r#"[
        {"op": "create_element", "key": "one", "type_ref": "t", "name": "Orders DB"},
        {"op": "create_element", "key": "two", "type_ref": "t", "name": "Orders DB"}
    ]"#;

    let applied = apply(&snapshot(), &operations(json)).unwrap();

    assert_eq!(
        applied.created,
        vec![
            (key("one"), Created::Element(element_id("orders-db-2"))),
            (key("two"), Created::Element(element_id("orders-db-3"))),
        ]
    );
}

#[test]
fn gives_colliding_titles_suffixes() {
    let json = r#"[
        {"op": "create_board", "key": "one", "title": "Main"},
        {"op": "create_board", "key": "two", "title": "Main"}
    ]"#;

    let applied = apply(&snapshot(), &operations(json)).unwrap();

    assert_eq!(
        applied.created,
        vec![
            (key("one"), Created::Board(board_id("main-2"))),
            (key("two"), Created::Board(board_id("main-3"))),
        ]
    );
    assert_eq!(written_board(&applied, "main-2").title, "Main");
    assert_eq!(written_board(&applied, "main-3").title, "Main");
    assert_eq!(applied.writes.len(), 2);
}

#[test]
fn never_takes_over_an_orphan_placement() {
    let json = r#"[{"op": "create_element", "key": "g", "type_ref": "t", "name": "Ghost"}]"#;

    let applied = apply(&snapshot(), &operations(json)).unwrap();

    assert_eq!(
        applied.created,
        vec![(key("g"), Created::Element(element_id("ghost-2")))]
    );
}

#[test]
fn shortens_a_long_name_to_a_valid_slug() {
    let json = format!(
        r#"[
            {{"op": "create_element", "key": "a", "type_ref": "t", "name": "{long}"}},
            {{"op": "create_element", "key": "b", "type_ref": "t", "name": "{long}"}}
        ]"#,
        long = "x".repeat(100)
    );

    let applied = apply(&snapshot(), &operations(&json)).unwrap();

    assert_eq!(
        applied.created,
        vec![
            (key("a"), Created::Element(element_id(&"x".repeat(64)))),
            (
                key("b"),
                Created::Element(element_id(&format!("{}-2", "x".repeat(62))))
            ),
        ]
    );
}

#[rstest]
#[case::unknown_element(
    r#"[{"op": "place_element", "element": {"existing": "nope"}, "board": {"existing": "main"}}]"#,
    OperationProblem::UnknownElement(element_id("nope"))
)]
#[case::orphan_is_not_an_element(
    r#"[{"op": "place_element", "element": {"existing": "ghost"}, "board": {"existing": "main"}}]"#,
    OperationProblem::UnknownElement(element_id("ghost"))
)]
#[case::existing_names_no_element_the_request_creates(
    r#"[
        {"op": "create_element", "key": "e", "type_ref": "t", "name": "E"},
        {"op": "place_element", "element": {"existing": "e"}, "board": {"existing": "main"}}
    ]"#,
    OperationProblem::UnknownElement(element_id("e"))
)]
#[case::unknown_board(
    r#"[{"op": "place_element", "element": {"existing": "loose"}, "board": {"existing": "nope"}}]"#,
    OperationProblem::UnknownBoard(board_id("nope"))
)]
#[case::unknown_key(
    r#"[{"op": "place_element", "element": {"new": "x"}, "board": {"existing": "main"}}]"#,
    OperationProblem::UnknownKey(key("x"))
)]
#[case::key_used_before_its_creation(
    r#"[
        {"op": "place_element", "element": {"new": "e"}, "board": {"existing": "main"}},
        {"op": "create_element", "key": "e", "type_ref": "t", "name": "E"}
    ]"#,
    OperationProblem::UnknownKey(key("e"))
)]
#[case::key_created_twice(
    r#"[
        {"op": "create_element", "key": "k", "type_ref": "t", "name": "A"},
        {"op": "create_element", "key": "k", "type_ref": "t", "name": "B"}
    ]"#,
    OperationProblem::DuplicateKey(key("k"))
)]
#[case::key_for_an_element_and_a_board(
    r#"[
        {"op": "create_element", "key": "k", "type_ref": "t", "name": "A"},
        {"op": "create_board", "key": "k", "title": "B"}
    ]"#,
    OperationProblem::DuplicateKey(key("k"))
)]
#[case::board_key_as_element(
    r#"[
        {"op": "create_board", "key": "b", "title": "B"},
        {"op": "place_element", "element": {"new": "b"}, "board": {"new": "b"}}
    ]"#,
    OperationProblem::WrongKind { key: key("b"), expected: Kind::Element, actual: Kind::Board }
)]
#[case::element_key_as_board(
    r#"[
        {"op": "create_element", "key": "e", "type_ref": "t", "name": "E"},
        {"op": "place_element", "element": {"new": "e"}, "board": {"new": "e"}}
    ]"#,
    OperationProblem::WrongKind { key: key("e"), expected: Kind::Board, actual: Kind::Element }
)]
#[case::element_with_a_home_on_disk(
    r#"[{"op": "place_element", "element": {"existing": "orders-db"}, "board": {"existing": "main"}}]"#,
    OperationProblem::AlreadyHomed(element_id("orders-db"))
)]
#[case::element_placed_twice_in_one_request(
    r#"[
        {"op": "create_board", "key": "b", "title": "B"},
        {"op": "place_element", "element": {"existing": "loose"}, "board": {"new": "b"}},
        {"op": "place_element", "element": {"existing": "loose"}, "board": {"existing": "main"}}
    ]"#,
    OperationProblem::AlreadyHomed(element_id("loose"))
)]
#[case::move_of_an_element_on_no_board(
    r#"[{"op": "move_element", "element": {"existing": "loose"}, "board": {"existing": "main"}, "x": 0, "y": 0}]"#,
    OperationProblem::NotOnBoard { element: element_id("loose"), board: board_id("main") }
)]
#[case::move_on_a_board_where_the_element_does_not_lie(
    r#"[
        {"op": "create_board", "key": "b", "title": "B"},
        {"op": "move_element", "element": {"existing": "orders-db"}, "board": {"new": "b"}, "x": 0, "y": 0}
    ]"#,
    OperationProblem::NotOnBoard { element: element_id("orders-db"), board: board_id("b") }
)]
#[case::move_of_an_orphan_placement(
    r#"[{"op": "move_element", "element": {"existing": "ghost"}, "board": {"existing": "main"}, "x": 0, "y": 0}]"#,
    OperationProblem::UnknownElement(element_id("ghost"))
)]
#[case::move_of_an_unknown_element(
    r#"[{"op": "move_element", "element": {"existing": "nope"}, "board": {"existing": "main"}, "x": 0, "y": 0}]"#,
    OperationProblem::UnknownElement(element_id("nope"))
)]
#[case::move_on_an_unknown_board(
    r#"[{"op": "move_element", "element": {"existing": "orders-db"}, "board": {"existing": "nope"}, "x": 0, "y": 0}]"#,
    OperationProblem::UnknownBoard(board_id("nope"))
)]
#[case::move_of_an_unknown_key(
    r#"[{"op": "move_element", "element": {"new": "x"}, "board": {"existing": "main"}, "x": 0, "y": 0}]"#,
    OperationProblem::UnknownKey(key("x"))
)]
#[case::move_right_of_the_range(
    r#"[{"op": "move_element", "element": {"existing": "orders-db"}, "board": {"existing": "main"}, "x": 1000001, "y": 0}]"#,
    OperationProblem::OutOfRange { x: 1_000_001, y: 0 }
)]
#[case::move_left_of_the_range(
    r#"[{"op": "move_element", "element": {"existing": "orders-db"}, "board": {"existing": "main"}, "x": -1000001, "y": 0}]"#,
    OperationProblem::OutOfRange { x: -1_000_001, y: 0 }
)]
#[case::move_below_the_range(
    r#"[{"op": "move_element", "element": {"existing": "orders-db"}, "board": {"existing": "main"}, "x": 0, "y": 1000001}]"#,
    OperationProblem::OutOfRange { x: 0, y: 1_000_001 }
)]
#[case::move_above_the_range(
    r#"[{"op": "move_element", "element": {"existing": "orders-db"}, "board": {"existing": "main"}, "x": 0, "y": -1000001}]"#,
    OperationProblem::OutOfRange { x: 0, y: -1_000_001 }
)]
#[case::move_to_the_largest_i32(
    r#"[{"op": "move_element", "element": {"existing": "orders-db"}, "board": {"existing": "main"}, "x": 2147483647, "y": 0}]"#,
    OperationProblem::OutOfRange { x: i32::MAX, y: 0 }
)]
#[case::move_to_the_smallest_i32(
    r#"[{"op": "move_element", "element": {"existing": "orders-db"}, "board": {"existing": "main"}, "x": 0, "y": -2147483648}]"#,
    OperationProblem::OutOfRange { x: 0, y: i32::MIN }
)]
#[case::empty_type(
    r#"[{"op": "create_element", "key": "e", "type_ref": "", "name": "E"}]"#,
    OperationProblem::InvalidElement(ElementProblem::EmptyType)
)]
#[case::disallowed_character_in_type(
    r#"[{"op": "create_element", "key": "e", "type_ref": "a\u0001", "name": "E"}]"#,
    OperationProblem::InvalidElement(ElementProblem::InvalidCharacter { field: "type", code: 1 })
)]
#[case::disallowed_character_in_name(
    r#"[{"op": "create_element", "key": "e", "type_ref": "t", "name": "a\u007f"}]"#,
    OperationProblem::InvalidElement(ElementProblem::InvalidCharacter { field: "name", code: 0x7f })
)]
#[case::disallowed_character_in_summary(
    r#"[{"op": "create_element", "key": "e", "type_ref": "t", "name": "E", "summary": "￿"}]"#,
    OperationProblem::InvalidElement(ElementProblem::InvalidCharacter { field: "summary", code: 0xffff })
)]
fn rejects_operations_that_cannot_apply(#[case] json: &str, #[case] expected: OperationProblem) {
    assert_eq!(problem(&snapshot(), json), expected);
}

#[test]
fn rejects_an_element_whose_frontmatter_would_exceed_the_limit() {
    let json = format!(
        r#"[{{"op": "create_element", "key": "e", "type_ref": "t", "name": "{}"}}]"#,
        "a".repeat(70_000)
    );

    assert!(matches!(
        problem(&snapshot(), &json),
        OperationProblem::InvalidElement(ElementProblem::FrontmatterTooLarge { .. })
    ));
}

#[test]
fn rejects_the_whole_request_when_the_last_operation_fails() {
    let json = r#"[
        {"op": "create_board", "key": "b", "title": "B"},
        {"op": "create_element", "key": "e", "type_ref": "t", "name": "E"},
        {"op": "place_element", "element": {"new": "e"}, "board": {"existing": "nope"}}
    ]"#;

    let result = apply(&snapshot(), &operations(json));

    assert_eq!(
        result,
        Err(ApplyError::Operation {
            position: 3,
            problem: OperationProblem::UnknownBoard(board_id("nope")),
        })
    );
}

#[test]
fn accepts_exactly_the_most_operations() {
    let operations: Vec<Operation> = (0..MAX_OPERATIONS)
        .map(|index| Operation::CreateBoard {
            key: key(&format!("k{index}")),
            title: "B".to_owned(),
        })
        .collect();

    assert!(apply(&snapshot(), &operations).is_ok());
}

#[test]
fn rejects_one_operation_more_than_the_most() {
    let operations: Vec<Operation> = (0..=MAX_OPERATIONS)
        .map(|index| Operation::CreateBoard {
            key: key(&format!("k{index}")),
            title: "B".to_owned(),
        })
        .collect();

    assert_eq!(
        apply(&snapshot(), &operations),
        Err(ApplyError::TooManyOperations {
            count: MAX_OPERATIONS + 1,
        })
    );
}

#[test]
fn reports_a_full_board() {
    let mut full = snapshot();
    let huge = Rect {
        x: i32::MIN,
        y: i32::MIN,
        width: NonZeroU32::MAX,
        height: NonZeroU32::MAX,
    };
    full.boards
        .get_mut(&board_id("main"))
        .unwrap()
        .placements
        .insert(element_id("huge"), huge);
    let json = r#"[{"op": "place_element", "element": {"existing": "loose"}, "board": {"existing": "main"}}]"#;

    assert_eq!(
        problem(&full, json),
        OperationProblem::BoardFull(board_id("main"))
    );
}

/// The element files a request writes, by ID, in the order of the writes.
fn written_elements(applied: &Applied) -> Vec<&Element> {
    applied
        .writes
        .iter()
        .filter_map(|write| match write {
            Write::Element(element) => Some(element),
            Write::Board(_) | Write::RemoveElement(_) => None,
        })
        .collect()
}

fn relations_of(element: &Element) -> Vec<(RelationKind, &str, Option<&str>)> {
    element
        .relations()
        .iter()
        .map(|((kind, to), label)| (*kind, to.as_str(), label.as_deref()))
        .collect()
}

#[test]
fn creates_a_relation_and_writes_only_its_source() {
    let json = r#"[{"op": "create_relation", "from": {"existing": "loose"}, "kind": "uses", "to": {"existing": "orders-db"}, "label": "SQL"}]"#;

    let applied = apply(&snapshot(), &operations(json)).unwrap();

    let written = written_elements(&applied);
    assert_eq!(written.len(), 1);
    let source = written.first().unwrap();
    assert_eq!(source.id(), &element_id("loose"));
    assert_eq!(
        relations_of(source),
        [(RelationKind::Uses, "orders-db", Some("SQL"))]
    );
    assert_eq!(applied.writes.len(), 1);
}

#[test]
fn writes_a_source_with_two_new_relations_once() {
    let json = r#"[
        {"op": "create_relation", "from": {"existing": "loose"}, "kind": "uses", "to": {"existing": "orders-db"}},
        {"op": "create_relation", "from": {"existing": "loose"}, "kind": "concerns", "to": {"existing": "orders-db"}}
    ]"#;

    let applied = apply(&snapshot(), &operations(json)).unwrap();

    let written = written_elements(&applied);
    assert_eq!(written.len(), 1);
    assert_eq!(
        relations_of(written.first().unwrap()),
        [
            (RelationKind::Concerns, "orders-db", None),
            (RelationKind::Uses, "orders-db", None)
        ]
    );
}

#[test]
fn writes_a_created_element_once_with_its_relations_before_changed_ones_and_boards() {
    let json = r#"[
        {"op": "create_element", "key": "pay", "type_ref": "t", "name": "Payment"},
        {"op": "create_relation", "from": {"new": "pay"}, "kind": "depends_on", "to": {"existing": "orders-db"}},
        {"op": "create_relation", "from": {"existing": "loose"}, "kind": "see_also", "to": {"new": "pay"}},
        {"op": "place_element", "element": {"new": "pay"}, "board": {"existing": "main"}}
    ]"#;

    let applied = apply(&snapshot(), &operations(json)).unwrap();

    assert!(matches!(
        applied.writes.as_slice(),
        [Write::Element(_), Write::Element(_), Write::Board(_)]
    ));
    let written = written_elements(&applied);
    let ids: Vec<&str> = written
        .iter()
        .map(|element| element.id().as_str())
        .collect();
    assert_eq!(ids, ["payment", "loose"]);
    assert_eq!(
        relations_of(written.first().unwrap()),
        [(RelationKind::DependsOn, "orders-db", None)]
    );
}

#[test]
fn deletes_a_relation() {
    let mut snapshot = snapshot();
    let json = r#"[{"op": "create_relation", "from": {"existing": "loose"}, "kind": "uses", "to": {"existing": "orders-db"}}]"#;
    let source = written_elements(&apply(&snapshot, &operations(json)).unwrap())
        .first()
        .copied()
        .cloned()
        .unwrap();
    snapshot.elements.insert(element_id("loose"), source);
    let json = r#"[{"op": "delete_relation", "from": {"existing": "loose"}, "kind": "uses", "to": {"existing": "orders-db"}}]"#;

    let applied = apply(&snapshot, &operations(json)).unwrap();

    let written = written_elements(&applied);
    assert_eq!(written.len(), 1);
    assert_eq!(relations_of(written.first().unwrap()), []);
}

#[test]
fn deletes_a_relation_whose_target_file_is_gone() {
    let mut snapshot = snapshot();
    let source = Element::new(
        element_id("loose"),
        ElementFields {
            relations: Relations::from([((RelationKind::Uses, element_id("gone")), None)]),
            ..element("loose").into_fields()
        },
    )
    .unwrap();
    snapshot.elements.insert(element_id("loose"), source);
    let json = r#"[{"op": "delete_relation", "from": {"existing": "loose"}, "kind": "uses", "to": {"existing": "gone"}}]"#;

    let applied = apply(&snapshot, &operations(json)).unwrap();

    assert_eq!(
        relations_of(written_elements(&applied).first().unwrap()),
        []
    );
}

#[rstest]
#[case::unknown_source(
    r#"[{"op": "create_relation", "from": {"existing": "nope"}, "kind": "uses", "to": {"existing": "loose"}}]"#,
    OperationProblem::UnknownElement(element_id("nope"))
)]
#[case::unknown_target(
    r#"[{"op": "create_relation", "from": {"existing": "loose"}, "kind": "uses", "to": {"existing": "nope"}}]"#,
    OperationProblem::UnknownElement(element_id("nope"))
)]
#[case::target_without_a_file(
    r#"[{"op": "create_relation", "from": {"existing": "loose"}, "kind": "uses", "to": {"existing": "ghost"}}]"#,
    OperationProblem::UnknownElement(element_id("ghost"))
)]
#[case::board_key_as_target(
    r#"[
        {"op": "create_board", "key": "b", "title": "B"},
        {"op": "create_relation", "from": {"existing": "loose"}, "kind": "uses", "to": {"new": "b"}}
    ]"#,
    OperationProblem::WrongKind { key: key("b"), expected: Kind::Element, actual: Kind::Board }
)]
#[case::board_key_as_target_of_a_deletion(
    r#"[
        {"op": "create_board", "key": "b", "title": "B"},
        {"op": "delete_relation", "from": {"existing": "loose"}, "kind": "uses", "to": {"new": "b"}}
    ]"#,
    OperationProblem::WrongKind { key: key("b"), expected: Kind::Element, actual: Kind::Board }
)]
#[case::relation_twice(
    r#"[
        {"op": "create_relation", "from": {"existing": "loose"}, "kind": "uses", "to": {"existing": "orders-db"}, "label": "A"},
        {"op": "create_relation", "from": {"existing": "loose"}, "kind": "uses", "to": {"existing": "orders-db"}, "label": "B"}
    ]"#,
    OperationProblem::RelationExists { from: element_id("loose"), kind: RelationKind::Uses, to: element_id("orders-db") }
)]
#[case::relation_to_itself(
    r#"[{"op": "create_relation", "from": {"existing": "loose"}, "kind": "uses", "to": {"existing": "loose"}}]"#,
    OperationProblem::InvalidElement(ElementProblem::SelfRelation { kind: RelationKind::Uses })
)]
#[case::label_with_a_control_character(
    r#"[{"op": "create_relation", "from": {"existing": "loose"}, "kind": "uses", "to": {"existing": "orders-db"}, "label": "a\u0000b"}]"#,
    OperationProblem::InvalidElement(ElementProblem::InvalidCharacter { field: "label", code: 0 })
)]
#[case::missing_relation(
    r#"[{"op": "delete_relation", "from": {"existing": "loose"}, "kind": "uses", "to": {"existing": "orders-db"}}]"#,
    OperationProblem::NoSuchRelation { from: element_id("loose"), kind: RelationKind::Uses, to: element_id("orders-db") }
)]
#[case::unknown_source_of_a_deletion(
    r#"[{"op": "delete_relation", "from": {"existing": "ghost"}, "kind": "uses", "to": {"existing": "loose"}}]"#,
    OperationProblem::UnknownElement(element_id("ghost"))
)]
fn rejects_relations_that_cannot_apply(#[case] json: &str, #[case] expected: OperationProblem) {
    assert_eq!(problem(&snapshot(), json), expected);
}

#[test]
fn rejects_a_label_that_takes_the_frontmatter_past_the_limit() {
    let label = "a".repeat(abyme_model::MAX_FRONTMATTER_BYTES);
    let json = format!(
        r#"[{{"op": "create_relation", "from": {{"existing": "loose"}}, "kind": "uses", "to": {{"existing": "orders-db"}}, "label": "{label}"}}]"#
    );

    assert!(matches!(
        problem(&snapshot(), &json),
        OperationProblem::InvalidElement(ElementProblem::FrontmatterTooLarge { .. })
    ));
}

#[test]
fn shows_neither_label_nor_name_when_it_rejects_a_relation() {
    let json = r#"[
        {"op": "create_element", "key": "e", "type_ref": "t", "name": "SECRET-NAME"},
        {"op": "create_relation", "from": {"new": "e"}, "kind": "uses", "to": {"existing": "loose"}, "label": "SECRET-LABEL"},
        {"op": "create_relation", "from": {"new": "e"}, "kind": "uses", "to": {"existing": "loose"}, "label": "SECRET-LABEL"}
    ]"#;

    let message = apply(&snapshot(), &operations(json))
        .unwrap_err()
        .to_string();

    assert!(!message.contains("SECRET"), "{message}");
}

#[test]
fn deletes_a_relation_between_elements_created_in_the_same_request() {
    let json = r#"[
        {"op": "create_element", "key": "pay", "type_ref": "t", "name": "Payment"},
        {"op": "create_element", "key": "db", "type_ref": "t", "name": "Db"},
        {"op": "create_relation", "from": {"new": "pay"}, "kind": "uses", "to": {"new": "db"}},
        {"op": "create_relation", "from": {"new": "pay"}, "kind": "concerns", "to": {"existing": "orders-db"}},
        {"op": "delete_relation", "from": {"new": "pay"}, "kind": "uses", "to": {"new": "db"}}
    ]"#;

    let applied = apply(&snapshot(), &operations(json)).unwrap();

    let written = written_elements(&applied);
    let payment: Vec<&&Element> = written
        .iter()
        .filter(|element| element.id().as_str() == "payment")
        .collect();
    assert_eq!(payment.len(), 1);
    assert_eq!(
        relations_of(payment.first().unwrap()),
        [(RelationKind::Concerns, "orders-db", None)]
    );
}

#[test]
fn deletes_a_relation_from_an_existing_element_to_one_created_in_the_same_request() {
    let json = r#"[
        {"op": "create_element", "key": "db", "type_ref": "t", "name": "Db"},
        {"op": "create_relation", "from": {"existing": "loose"}, "kind": "uses", "to": {"new": "db"}},
        {"op": "create_relation", "from": {"existing": "loose"}, "kind": "see_also", "to": {"existing": "orders-db"}},
        {"op": "delete_relation", "from": {"existing": "loose"}, "kind": "uses", "to": {"new": "db"}}
    ]"#;

    let applied = apply(&snapshot(), &operations(json)).unwrap();

    let written = written_elements(&applied);
    let ids: Vec<&str> = written
        .iter()
        .map(|element| element.id().as_str())
        .collect();
    assert_eq!(ids, ["db", "loose"]);
    assert_eq!(
        relations_of(written.get(1).unwrap()),
        [(RelationKind::SeeAlso, "orders-db", None)]
    );
}

/// The snapshot with `loose` given a summary, a body, and a relation.
fn snapshot_with_full_loose() -> Snapshot {
    let mut snapshot = snapshot();
    let loose = Element::new(
        element_id("loose"),
        ElementFields {
            type_ref: TypeRef::try_from("c4/container".to_owned()).unwrap(),
            name: "Loose".to_owned(),
            summary: Some("first".to_owned()),
            body: "Body\r\nwith CRLF\n".to_owned(),
            relations: Relations::from([(
                (RelationKind::Uses, element_id("orders-db")),
                Some("SQL".to_owned()),
            )]),
            anchor: None,
            verified: None,
        },
    )
    .unwrap();
    snapshot.elements.insert(element_id("loose"), loose);
    snapshot
}

fn update(snapshot: &Snapshot, json: &str) -> Element {
    let applied = apply(snapshot, &operations(json)).unwrap();
    let written = written_elements(&applied);
    assert_eq!(written.len(), 1, "{applied:?}");
    (*written.first().unwrap()).clone()
}

#[test]
fn updates_only_the_body_and_keeps_every_other_field() {
    let snapshot = snapshot_with_full_loose();
    let before = snapshot.elements.get(&element_id("loose")).unwrap().clone();

    let after = update(
        &snapshot,
        r#"[{"op": "update_element", "element": {"existing": "loose"}, "body": "New"}]"#,
    );

    assert_eq!(after.body(), "New");
    assert_eq!(after.id(), before.id());
    assert_eq!(after.name(), before.name());
    assert_eq!(after.type_ref(), before.type_ref());
    assert_eq!(after.summary(), before.summary());
    assert_eq!(after.relations(), before.relations());
}

#[test]
fn renames_an_element_and_keeps_its_id_and_body_byte_for_byte() {
    let snapshot = snapshot_with_full_loose();

    let after = update(
        &snapshot,
        r#"[{"op": "update_element", "element": {"existing": "loose"}, "name": "Renamed"}]"#,
    );

    assert_eq!(after.id(), &element_id("loose"));
    assert_eq!(after.name(), "Renamed");
    assert_eq!(after.body(), "Body\r\nwith CRLF\n");
}

#[test]
fn gives_an_element_created_after_a_rename_no_taken_id() {
    let json = r#"[
        {"op": "update_element", "element": {"existing": "loose"}, "name": "Other"},
        {"op": "create_element", "key": "e", "type_ref": "t", "name": "Loose"}
    ]"#;

    let applied = apply(&snapshot(), &operations(json)).unwrap();

    assert_eq!(
        applied.created,
        vec![(key("e"), Created::Element(element_id("loose-2")))]
    );
}

#[rstest]
#[case::summary_removed(r#""summary": null"#, None)]
#[case::summary_replaced(r#""summary": "second""#, Some("second"))]
fn removes_or_replaces_the_summary(#[case] field: &str, #[case] expected: Option<&str>) {
    let json =
        format!(r#"[{{"op": "update_element", "element": {{"existing": "loose"}}, {field}}}]"#);

    let after = update(&snapshot_with_full_loose(), &json);

    assert_eq!(after.summary(), expected);
}

#[test]
fn changes_the_type() {
    let after = update(
        &snapshot(),
        r#"[{"op": "update_element", "element": {"existing": "loose"}, "type_ref": "c4/component"}]"#,
    );

    assert_eq!(after.type_ref().as_str(), "c4/component");
}

#[test]
fn writes_an_element_created_and_updated_in_one_request_once_as_new() {
    let json = r#"[
        {"op": "create_element", "key": "e", "type_ref": "t", "name": "Fresh"},
        {"op": "update_element", "element": {"new": "e"}, "summary": "updated"},
        {"op": "create_board", "key": "b", "title": "B"}
    ]"#;

    let applied = apply(&snapshot(), &operations(json)).unwrap();

    assert!(matches!(
        applied.writes.as_slice(),
        [Write::Element(_), Write::Board(_)]
    ));
    let written = written_elements(&applied);
    assert_eq!(written.first().unwrap().summary(), Some("updated"));
}

#[test]
fn writes_an_element_updated_twice_once_with_the_last_state() {
    let json = r#"[
        {"op": "update_element", "element": {"existing": "loose"}, "name": "First"},
        {"op": "update_element", "element": {"existing": "loose"}, "name": "Second"}
    ]"#;

    let after = update(&snapshot(), json);

    assert_eq!(after.name(), "Second");
}

#[rstest]
#[case::unknown_element(
    r#"[{"op": "update_element", "element": {"existing": "nope"}, "name": "x"}]"#,
    OperationProblem::UnknownElement(element_id("nope"))
)]
#[case::element_without_a_file(
    r#"[{"op": "update_element", "element": {"existing": "ghost"}, "name": "x"}]"#,
    OperationProblem::UnknownElement(element_id("ghost"))
)]
#[case::board_key(
    r#"[
        {"op": "create_board", "key": "b", "title": "B"},
        {"op": "update_element", "element": {"new": "b"}, "name": "x"}
    ]"#,
    OperationProblem::WrongKind { key: key("b"), expected: Kind::Element, actual: Kind::Board }
)]
#[case::no_field(
    r#"[{"op": "update_element", "element": {"existing": "loose"}}]"#,
    OperationProblem::NothingToUpdate(element_id("loose"))
)]
#[case::empty_type(
    r#"[{"op": "update_element", "element": {"existing": "loose"}, "type_ref": ""}]"#,
    OperationProblem::InvalidElement(ElementProblem::EmptyType)
)]
fn rejects_updates_that_cannot_apply(#[case] json: &str, #[case] expected: OperationProblem) {
    assert_eq!(problem(&snapshot(), json), expected);
}

#[rstest]
#[case::name("name", "name")]
#[case::summary("summary", "summary")]
#[case::type_ref("type_ref", "type")]
fn rejects_a_disallowed_character_without_showing_the_value(
    #[case] field: &str,
    #[case] reported: &'static str,
) {
    let json = format!(
        r#"[{{"op": "update_element", "element": {{"existing": "loose"}}, "{field}": "SECRET\u0000"}}]"#
    );

    let error = apply(&snapshot(), &operations(&json)).unwrap_err();

    assert_eq!(
        error,
        ApplyError::Operation {
            position: 1,
            problem: OperationProblem::InvalidElement(ElementProblem::InvalidCharacter {
                field: reported,
                code: 0
            })
        }
    );
    assert!(!error.to_string().contains("SECRET"), "{error}");
}

#[rstest]
#[case::name("name")]
#[case::summary("summary")]
fn rejects_a_value_that_takes_the_frontmatter_past_the_limit(#[case] field: &str) {
    let value = "a".repeat(abyme_model::MAX_FRONTMATTER_BYTES);
    let json = format!(
        r#"[{{"op": "update_element", "element": {{"existing": "loose"}}, "{field}": "{value}"}}]"#
    );

    assert!(matches!(
        problem(&snapshot(), &json),
        OperationProblem::InvalidElement(ElementProblem::FrontmatterTooLarge { .. })
    ));
}

/// The snapshot with a second board `side` that places `orders-db` too.
fn snapshot_for_deletion() -> Snapshot {
    let mut snapshot = snapshot();
    let side = Board {
        id: board_id("side"),
        title: "Side".to_owned(),
        placements: BTreeMap::from([(element_id("orders-db"), rect(0, 0))]),
    };
    snapshot.boards.insert(side.id.clone(), side);
    snapshot
}

/// `snapshot` with `source` relating to `target` with a secret label.
fn with_relation(
    mut snapshot: Snapshot,
    source: &str,
    kind: RelationKind,
    target: &str,
) -> Snapshot {
    let element = snapshot
        .elements
        .remove(&element_id(source))
        .unwrap_or_else(|| element(source));
    let mut fields = element.into_fields();
    fields
        .relations
        .insert((kind, element_id(target)), Some("SECRET-LABEL".to_owned()));
    "SECRET-NAME".clone_into(&mut fields.name);
    snapshot.elements.insert(
        element_id(source),
        Element::new(element_id(source), fields).unwrap(),
    );
    snapshot
}

fn removals(applied: &Applied) -> Vec<&str> {
    applied
        .writes
        .iter()
        .filter_map(|write| match write {
            Write::RemoveElement(id) => Some(id.as_str()),
            Write::Element(_) | Write::Board(_) => None,
        })
        .collect()
}

#[test]
fn deletes_an_element_with_its_file_and_its_placement_on_every_board() {
    let json = r#"[{"op": "delete_element", "element": {"existing": "orders-db"}}]"#;

    let applied = apply(&snapshot_for_deletion(), &operations(json)).unwrap();

    assert!(matches!(
        applied.writes.as_slice(),
        [Write::Board(_), Write::Board(_), Write::RemoveElement(_)]
    ));
    assert_eq!(removals(&applied), ["orders-db"]);
    for board in ["main", "side"] {
        assert!(
            !written_board(&applied, board)
                .placements
                .contains_key(&element_id("orders-db"))
        );
    }
    assert!(
        written_board(&applied, "main")
            .placements
            .contains_key(&element_id("ghost"))
    );
}

#[test]
fn deletes_an_unplaced_element_with_only_its_file() {
    let json = r#"[{"op": "delete_element", "element": {"existing": "loose"}}]"#;

    let applied = apply(&snapshot(), &operations(json)).unwrap();

    assert_eq!(
        applied.writes,
        vec![Write::RemoveElement(element_id("loose"))]
    );
}

#[test]
fn deletes_a_placement_whose_file_is_missing_without_removing_a_file() {
    let json = r#"[{"op": "delete_element", "element": {"existing": "ghost"}}]"#;

    let applied = apply(&snapshot(), &operations(json)).unwrap();

    assert_eq!(removals(&applied), Vec::<&str>::new());
    let main = written_board(&applied, "main");
    assert!(!main.placements.contains_key(&element_id("ghost")));
    assert!(main.placements.contains_key(&element_id("orders-db")));
}

#[test]
fn deletes_an_element_created_in_the_same_request_without_writing_it() {
    let json = r#"[
        {"op": "create_element", "key": "e", "type_ref": "t", "name": "Fresh"},
        {"op": "place_element", "element": {"new": "e"}, "board": {"existing": "main"}},
        {"op": "delete_element", "element": {"new": "e"}}
    ]"#;

    let applied = apply(&snapshot(), &operations(json)).unwrap();

    assert_eq!(written_elements(&applied), Vec::<&Element>::new());
    assert_eq!(removals(&applied), Vec::<&str>::new());
    assert!(
        !written_board(&applied, "main")
            .placements
            .contains_key(&element_id("fresh"))
    );
}

#[test]
fn writes_changed_elements_then_boards_then_removals() {
    let json = r#"[
        {"op": "update_element", "element": {"existing": "loose"}, "name": "Changed"},
        {"op": "delete_element", "element": {"existing": "orders-db"}},
        {"op": "create_element", "key": "e", "type_ref": "t", "name": "New"}
    ]"#;

    let applied = apply(&snapshot_for_deletion(), &operations(json)).unwrap();

    let kinds: Vec<&str> = applied
        .writes
        .iter()
        .map(|write| match write {
            Write::Element(element) if element.id().as_str() == "new" => "new",
            Write::Element(_) => "changed",
            Write::Board(_) => "board",
            Write::RemoveElement(_) => "removal",
        })
        .collect();
    assert_eq!(kinds, ["new", "changed", "board", "board", "removal"]);
}

#[test]
fn rejects_an_element_that_others_still_relate_to_and_shows_only_ids_and_kinds() {
    let snapshot = with_relation(snapshot(), "loose", RelationKind::Uses, "orders-db");
    let json = r#"[{"op": "delete_element", "element": {"existing": "orders-db"}}]"#;

    let error = apply(&snapshot, &operations(json)).unwrap_err();

    assert_eq!(
        error,
        ApplyError::Operation {
            position: 1,
            problem: OperationProblem::StillRelated {
                element: element_id("orders-db"),
                sources: RelationSources(vec![(element_id("loose"), RelationKind::Uses)]),
            }
        }
    );
    let message = error.to_string();
    assert_eq!(
        message,
        "operation 1: element orders-db is still the target of relations from loose (uses); delete those first"
    );
    assert!(!message.contains("SECRET"), "{message}");
}

#[test]
fn names_at_most_ten_sources_and_counts_the_rest() {
    let mut snapshot = snapshot();
    for index in 0..12 {
        snapshot = with_relation(
            snapshot,
            &format!("s{index:02}"),
            RelationKind::DependsOn,
            "orders-db",
        );
    }
    let json = r#"[{"op": "delete_element", "element": {"existing": "orders-db"}}]"#;

    let message = apply(&snapshot, &operations(json)).unwrap_err().to_string();

    let shown: Vec<String> = (0..10)
        .map(|index| format!("s{index:02} (depends_on)"))
        .collect();
    assert!(
        message.contains(&format!("{}, and 2 more;", shown.join(", "))),
        "{message}"
    );
    assert!(!message.contains("s10"), "{message}");
}

#[rstest]
#[case::relation_deleted_first(
    r#"[
        {"op": "delete_relation", "from": {"existing": "loose"}, "kind": "uses", "to": {"existing": "orders-db"}},
        {"op": "delete_element", "element": {"existing": "orders-db"}}
    ]"#
)]
#[case::source_deleted_first(
    r#"[
        {"op": "delete_element", "element": {"existing": "loose"}},
        {"op": "delete_element", "element": {"existing": "orders-db"}}
    ]"#
)]
fn deletes_an_element_once_the_request_removed_what_related_to_it(#[case] json: &str) {
    let snapshot = with_relation(snapshot(), "loose", RelationKind::Uses, "orders-db");

    let applied = apply(&snapshot, &operations(json)).unwrap();

    assert!(removals(&applied).contains(&"orders-db"));
}

#[test]
fn rejects_a_deletion_while_a_new_element_relates_to_it() {
    let json = r#"[
        {"op": "create_element", "key": "b", "type_ref": "t", "name": "B"},
        {"op": "create_relation", "from": {"new": "b"}, "kind": "uses", "to": {"existing": "loose"}},
        {"op": "delete_element", "element": {"existing": "loose"}}
    ]"#;

    assert_eq!(
        problem(&snapshot(), json),
        OperationProblem::StillRelated {
            element: element_id("loose"),
            sources: RelationSources(vec![(element_id("b"), RelationKind::Uses)]),
        }
    );
}

#[test]
fn rejects_deleting_a_placement_without_a_file_while_an_element_relates_to_it() {
    let snapshot = with_relation(snapshot(), "loose", RelationKind::Concerns, "ghost");

    assert_eq!(
        problem(
            &snapshot,
            r#"[{"op": "delete_element", "element": {"existing": "ghost"}}]"#
        ),
        OperationProblem::StillRelated {
            element: element_id("ghost"),
            sources: RelationSources(vec![(element_id("loose"), RelationKind::Concerns)]),
        }
    );
}

#[rstest]
#[case::update(r#"{"op": "update_element", "element": {"existing": "loose"}, "name": "x"}"#)]
#[case::relation_from(
    r#"{"op": "create_relation", "from": {"existing": "loose"}, "kind": "uses", "to": {"existing": "orders-db"}}"#
)]
#[case::relation_to(
    r#"{"op": "create_relation", "from": {"existing": "orders-db"}, "kind": "uses", "to": {"existing": "loose"}}"#
)]
#[case::relation_deleted(
    r#"{"op": "delete_relation", "from": {"existing": "loose"}, "kind": "uses", "to": {"existing": "orders-db"}}"#
)]
#[case::placement(
    r#"{"op": "place_element", "element": {"existing": "loose"}, "board": {"existing": "main"}}"#
)]
#[case::move_(r#"{"op": "move_element", "element": {"existing": "loose"}, "board": {"existing": "main"}, "x": 0, "y": 0}"#)]
#[case::deletion_again(r#"{"op": "delete_element", "element": {"existing": "loose"}}"#)]
fn knows_a_deleted_element_no_longer(#[case] operation: &str) {
    let json =
        format!(r#"[{{"op": "delete_element", "element": {{"existing": "loose"}}}}, {operation}]"#);

    assert_eq!(
        problem(&snapshot(), &json),
        OperationProblem::UnknownElement(element_id("loose"))
    );
}

#[test]
fn keeps_the_id_of_a_deleted_element_taken_for_the_rest_of_the_request() {
    let json = r#"[
        {"op": "delete_element", "element": {"existing": "loose"}},
        {"op": "create_element", "key": "e", "type_ref": "t", "name": "Loose"}
    ]"#;

    let applied = apply(&snapshot(), &operations(json)).unwrap();

    assert_eq!(
        applied.created,
        vec![(key("e"), Created::Element(element_id("loose-2")))]
    );
    assert_eq!(removals(&applied), ["loose"]);
}

#[test]
fn rejects_deleting_an_unknown_element() {
    assert_eq!(
        problem(
            &snapshot(),
            r#"[{"op": "delete_element", "element": {"existing": "nope"}}]"#
        ),
        OperationProblem::UnknownElement(element_id("nope"))
    );
}

#[test]
fn creates_elements_without_an_anchor_or_a_stamp() {
    let json = r#"[{"op": "create_element", "key": "e", "type_ref": "t", "name": "Fresh"}]"#;

    let applied = apply(&snapshot(), &operations(json)).unwrap();

    let created = written_elements(&applied);
    let fresh = created.first().unwrap();
    assert_eq!(fresh.anchor(), None);
    assert_eq!(fresh.verified(), None);
}

#[test]
fn keeps_the_anchor_and_the_stamp_when_a_relation_changes() {
    let mut snapshot = snapshot();
    let stamped = Element::from_markdown(
        &element_id("loose"),
        "---\nid: loose\ntype: t\nname: Loose\nanchor: crates/loose\n\
         verified: \"0123456789abcdef0123456789abcdef01234567\"\nverified_by: person\n---\n",
    )
    .unwrap();
    snapshot
        .elements
        .insert(element_id("loose"), stamped.clone());
    let json = r#"[{"op": "create_relation", "from": {"existing": "loose"}, "kind": "uses", "to": {"existing": "orders-db"}}]"#;

    let applied = apply(&snapshot, &operations(json)).unwrap();

    let changed = written_elements(&applied);
    let loose = changed.first().unwrap();
    assert_eq!(loose.anchor(), stamped.anchor());
    assert_eq!(loose.verified(), stamped.verified());
}

const COMMIT: &str = "0123456789abcdef0123456789abcdef01234567";

/// The snapshot with `anchored`, anchored on `crates/anchored`, and
/// `native`, without an anchor; both carry a stamp.
fn stamped_snapshot() -> Snapshot {
    let mut snapshot = snapshot();
    for (id, anchor) in [("anchored", "anchor: crates/anchored\n"), ("native", "")] {
        let element = Element::from_markdown(
            &element_id(id),
            &format!(
                "---\nid: {id}\ntype: t\nname: N\n{anchor}verified: \"{COMMIT}\"\nverified_by: person\n---\n"
            ),
        )
        .unwrap();
        snapshot.elements.insert(element_id(id), element);
    }
    snapshot
}

/// The element `id` as the request wrote it.
fn written<'a>(applied: &'a Applied, id: &str) -> &'a Element {
    written_elements(applied)
        .into_iter()
        .find(|element| element.id().as_str() == id)
        .unwrap()
}

#[test]
fn creates_an_element_with_an_anchor_and_no_stamp() {
    let json = r#"[{"op": "create_element", "key": "e", "type_ref": "t", "name": "Fresh", "anchor": "crates/fresh"}]"#;

    let applied = apply(&snapshot(), &operations(json)).unwrap();

    let fresh = written(&applied, "fresh");
    assert_eq!(fresh.anchor().map(Anchor::as_str), Some("crates/fresh"));
    assert_eq!(fresh.verified(), None);
}

#[rstest]
#[case::other_anchor("anchored", serde_json::json!("crates/other"), Some("crates/other"), false)]
#[case::removed_anchor("anchored", serde_json::Value::Null, None, false)]
#[case::other_case("anchored", serde_json::json!("Crates/anchored"), Some("Crates/anchored"), false)]
#[case::same_anchor("anchored", serde_json::json!("crates/anchored"), Some("crates/anchored"), true)]
#[case::first_anchor("native", serde_json::json!("docs"), Some("docs"), false)]
#[case::no_anchor_removed("native", serde_json::Value::Null, None, true)]
fn keeps_the_stamp_only_while_the_anchor_stays_the_same(
    #[case] id: &str,
    #[case] anchor: serde_json::Value,
    #[case] expected: Option<&str>,
    #[case] keeps_the_stamp: bool,
) {
    let json = serde_json::json!([
        {"op": "update_element", "element": {"existing": id}, "anchor": anchor}
    ]);

    let applied = apply(&stamped_snapshot(), &operations(&json.to_string())).unwrap();

    let updated = written(&applied, id);
    assert_eq!(updated.anchor().map(Anchor::as_str), expected);
    assert_eq!(updated.verified().is_some(), keeps_the_stamp);
}

#[test]
fn drops_the_stamp_of_an_anchor_changed_and_changed_back_in_one_request() {
    let json = r#"[
        {"op": "update_element", "element": {"existing": "anchored"}, "anchor": "crates/other"},
        {"op": "update_element", "element": {"existing": "anchored"}, "anchor": "crates/anchored"}
    ]"#;

    let applied = apply(&stamped_snapshot(), &operations(json)).unwrap();

    let updated = written(&applied, "anchored");
    assert_eq!(
        updated.anchor().map(Anchor::as_str),
        Some("crates/anchored")
    );
    assert_eq!(updated.verified(), None);
}

#[test]
fn keeps_the_anchor_and_the_stamp_when_other_fields_change() {
    let json = r#"[{"op": "update_element", "element": {"existing": "anchored"}, "name": "M", "summary": "S", "body": "B"}]"#;
    let snapshot = stamped_snapshot();

    let applied = apply(&snapshot, &operations(json)).unwrap();

    let before = snapshot.elements.get(&element_id("anchored")).unwrap();
    let updated = written(&applied, "anchored");
    assert_eq!(updated.anchor(), before.anchor());
    assert_eq!(updated.verified(), before.verified());
}

#[rstest]
#[case::dot_dot_segment("crates/../etc")]
#[case::leading_slash("/etc/passwd")]
#[case::reserved_folder("abyme/elements")]
#[case::git_folder_in_any_case("a/.GIT/b")]
#[case::colon("C:x")]
#[case::right_to_left_override("a\u{202e}b")]
#[case::line_feed("a\nb")]
#[case::too_long(&"a".repeat(4097))]
fn rejects_an_invalid_anchor_in_a_creation_and_an_update(#[case] anchor: &str) {
    let snapshot = stamped_snapshot();
    let requests = [
        serde_json::json!([
            {"op": "create_element", "key": "e", "type_ref": "t", "name": "N", "anchor": anchor}
        ]),
        serde_json::json!([
            {"op": "update_element", "element": {"existing": "anchored"}, "anchor": anchor}
        ]),
    ];

    for request in requests {
        let problem = problem(&snapshot, &request.to_string());

        assert!(
            matches!(
                problem,
                OperationProblem::InvalidElement(ElementProblem::InvalidAnchor { .. })
            ),
            "{problem:?}"
        );
    }
}

#[test]
fn shows_a_rejected_anchor_escaped_and_shortened() {
    let anchor = format!("\u{202e}\u{1b}[31m{}", "x".repeat(10_000));
    let json = serde_json::json!([
        {"op": "update_element", "element": {"existing": "anchored"}, "anchor": anchor}
    ]);

    let error = apply(&stamped_snapshot(), &operations(&json.to_string())).unwrap_err();

    let message = error.to_string();
    assert!(message.len() < 600, "{} bytes", message.len());
    assert!(!message.chars().any(hides_or_breaks_text), "{message:?}");
    assert!(message.contains("\\u{202e}"), "{message:?}");
}
