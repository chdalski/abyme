//! Tests of turning operations into file writes.

use super::*;
use std::num::NonZeroU32;

use abyme_model::Rect;
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
        TypeRef::try_from("t".to_owned()).unwrap(),
        id.to_owned(),
        None,
        String::new(),
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
            Write::Board(_) | Write::Element(_) => None,
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
