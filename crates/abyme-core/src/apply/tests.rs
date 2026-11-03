//! Tests of turning operations into file writes.

#![expect(
    clippy::disallowed_fields,
    reason = "the tests check the fields as the board file stores them"
)]
use super::*;
use std::num::NonZeroU32;

use abyme_model::{Mode, Rect, StatementKind, hides_or_breaks_text};
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

fn placed(x: i32, y: i32) -> Placement {
    sized(x, y, 180, 90)
}

fn element(id: &str) -> Element {
    Element::new(
        element_id(id),
        ElementFields {
            statement: None,
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
        owner: None,
        summary: None,
        zone: None,
        placements: BTreeMap::from([
            (element_id("orders-db"), placed(0, 0)),
            (element_id("ghost"), placed(220, 0)),
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
        Some(&placed(0, 0))
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
    assert_eq!(
        board.placements.get(&element_id("a")),
        Some(&placed(440, 0))
    );
    assert_eq!(
        board.placements.get(&element_id("b")),
        Some(&placed(660, 0))
    );
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
            owner: None,
            summary: None,
            zone: None,
            placements: BTreeMap::from([(element_id("loose"), placed(0, 0))]),
        },
    );
    snapshot
}

fn sized(x: i32, y: i32, width: u32, height: u32) -> Placement {
    Placement {
        rect: Rect {
            x,
            y,
            width: NonZeroU32::new(width).unwrap(),
            height: NonZeroU32::new(height).unwrap(),
        },
        pinned: false,
        attached_to: None,
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
        Some(&placed(x, y))
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
        Some(&placed(1000, -50))
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
        Some(&placed(30, 40))
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
        Some(&placed(440, 0))
    );
    assert_eq!(
        board.placements.get(&element_id("loose")),
        Some(&placed(660, 0))
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
        Some(&placed(0, 8360 + 90 + 40))
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
fn a_note_without_a_letter_or_digit_in_its_name_gets_the_id_note() {
    let json = r#"[
        {"op": "create_element", "key": "a", "type_ref": "note", "name": "?!"},
        {"op": "create_element", "key": "b", "type_ref": "note", "name": "…"}
    ]"#;

    let applied = apply(&snapshot(), &operations(json)).unwrap();

    assert_eq!(
        applied.created,
        vec![
            (key("a"), Created::Element(element_id("note"))),
            (key("b"), Created::Element(element_id("note-2"))),
        ]
    );
}

#[rstest]
#[case::other_type("t")]
#[case::capitalized("Note")]
#[case::upper_case("NOTE")]
#[case::longer("notes")]
#[case::in_a_namespace("c4/note")]
#[case::with_a_space("note ")]
fn another_type_keeps_the_fallback_element(#[case] type_ref: &str) {
    let json = format!(
        r#"[{{"op": "create_element", "key": "a", "type_ref": "{type_ref}", "name": "?!"}}]"#
    );

    let applied = apply(&snapshot(), &operations(&json)).unwrap();

    assert_eq!(
        applied.created,
        vec![(key("a"), Created::Element(element_id("element")))]
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
            owner: None,
            summary: None,
            zone: None,
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
            owner: None,
            summary: None,
            zone: None,
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
        .insert(
            element_id("huge"),
            Placement {
                rect: huge,
                pinned: false,
                attached_to: None,
            },
        );
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
            statement: None,
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
        owner: None,
        summary: None,
        zone: None,
        placements: BTreeMap::from([(element_id("orders-db"), placed(0, 0))]),
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

const COMMIT: &str = "0123456789abcdef0123456789abcdef01234567";

/// The snapshot with `anchored`, anchored on `crates/anchored` and related
/// to `ghost`, which has no file, and `native`, without an anchor; both
/// carry a stamp.
fn stamped_snapshot() -> Snapshot {
    let mut snapshot = snapshot();
    for (id, anchor, relations) in [
        (
            "anchored",
            "anchor: crates/anchored\n",
            "relations:\n  - {kind: uses, to: ghost}\n",
        ),
        ("native", "", ""),
    ] {
        let element = Element::from_markdown(
            &element_id(id),
            &format!(
                "---\nid: {id}\ntype: t\nname: N\n{anchor}verified: \"{COMMIT}\"\nverified_by: person\n{relations}---\n"
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
#[case::name(r#""name": "M""#, Some("crates/anchored"))]
#[case::same_name(r#""name": "N""#, Some("crates/anchored"))]
#[case::type_ref(r#""type_ref": "c4/container""#, Some("crates/anchored"))]
#[case::summary(r#""summary": "S""#, Some("crates/anchored"))]
#[case::summary_removed(r#""summary": null"#, Some("crates/anchored"))]
#[case::body(r#""body": "B""#, Some("crates/anchored"))]
#[case::same_anchor(r#""anchor": "crates/anchored""#, Some("crates/anchored"))]
#[case::other_anchor(r#""anchor": "crates/other""#, Some("crates/other"))]
#[case::other_case(r#""anchor": "Crates/anchored""#, Some("Crates/anchored"))]
#[case::removed_anchor(r#""anchor": null"#, None)]
fn removes_the_stamp_on_every_update(#[case] fields: &str, #[case] anchor: Option<&str>) {
    let json =
        format!(r#"[{{"op": "update_element", "element": {{"existing": "anchored"}}, {fields}}}]"#);

    let applied = apply(&stamped_snapshot(), &operations(&json)).unwrap();

    let updated = written(&applied, "anchored");
    assert_eq!(updated.anchor().map(Anchor::as_str), anchor, "{fields}");
    assert_eq!(updated.verified(), None, "{fields}");
}

#[rstest]
#[case::first_anchor(r#""anchor": "docs""#)]
#[case::no_anchor_removed(r#""anchor": null"#)]
fn removes_the_stamp_of_an_element_without_an_anchor(#[case] fields: &str) {
    let json =
        format!(r#"[{{"op": "update_element", "element": {{"existing": "native"}}, {fields}}}]"#);

    let applied = apply(&stamped_snapshot(), &operations(&json)).unwrap();

    assert_eq!(written(&applied, "native").verified(), None, "{fields}");
}

#[rstest]
#[case::created(
    r#"[{"op": "create_relation", "from": {"existing": "anchored"}, "kind": "uses", "to": {"existing": "native"}}]"#
)]
#[case::deleted_to_an_element_without_a_file(
    r#"[{"op": "delete_relation", "from": {"existing": "anchored"}, "kind": "uses", "to": {"existing": "ghost"}}]"#
)]
fn removes_the_stamp_of_the_source_of_a_changed_relation_only(#[case] json: &str) {
    let snapshot = stamped_snapshot();

    let applied = apply(&snapshot, &operations(json)).unwrap();

    let written: Vec<&str> = written_elements(&applied)
        .iter()
        .map(|element| element.id().as_str())
        .collect();
    assert_eq!(written, ["anchored"]);
    assert_eq!(written_elements(&applied).first().unwrap().verified(), None);
}

#[rstest]
#[case::placed_and_moved(
    r#"[{"op": "place_element", "element": {"existing": "anchored"}, "board": {"existing": "main"}},
        {"op": "move_element", "element": {"existing": "anchored"}, "board": {"existing": "main"}, "x": 600, "y": 0}]"#
)]
#[case::other_element_deleted(r#"[{"op": "delete_element", "element": {"existing": "loose"}}]"#)]
fn writes_no_stamped_element_that_the_request_does_not_change(#[case] json: &str) {
    let applied = apply(&stamped_snapshot(), &operations(json)).unwrap();

    assert_eq!(written_elements(&applied), Vec::<&Element>::new());
}

#[test]
fn a_failing_relation_change_writes_nothing() {
    let json = r#"[
        {"op": "update_element", "element": {"existing": "native"}, "name": "M"},
        {"op": "create_relation", "from": {"existing": "anchored"}, "kind": "uses", "to": {"existing": "native"}},
        {"op": "create_relation", "from": {"existing": "anchored"}, "kind": "uses", "to": {"existing": "native"}}
    ]"#;

    assert_eq!(
        problem(&stamped_snapshot(), json),
        OperationProblem::RelationExists {
            from: element_id("anchored"),
            kind: RelationKind::Uses,
            to: element_id("native"),
        }
    );
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

/// The board `id`, the inside of `owner`, placing `elements` side by side.
fn owned_board(id: &str, owner: Option<&str>, elements: &[&str]) -> (BoardId, Board) {
    let placements = elements
        .iter()
        .zip(0..)
        .map(|(element, column)| (element_id(element), placed(column * 220, 0)))
        .collect();
    (
        board_id(id),
        Board {
            id: board_id(id),
            title: id.to_owned(),
            owner: owner.map(element_id),
            summary: None,
            zone: None,
            placements,
        },
    )
}

/// A snapshot with the `elements` and the `boards`.
fn snapshot_of(elements: &[&str], boards: Vec<(BoardId, Board)>) -> Snapshot {
    Snapshot {
        elements: elements
            .iter()
            .map(|id| (element_id(id), element(id)))
            .collect(),
        boards: boards.into_iter().collect(),
    }
}

/// `context` places `shop`; `containers`, the inside of `shop`, places
/// `payment`; `components`, the inside of `payment`, places `api`;
/// `floating` and `loose` lie on no board, and `drafts`, the inside of
/// `floating`, places `sketch`.
fn hierarchy() -> Snapshot {
    snapshot_of(
        &["shop", "payment", "api", "floating", "sketch", "loose"],
        vec![
            owned_board("context", None, &["shop"]),
            owned_board("containers", Some("shop"), &["payment"]),
            owned_board("components", Some("payment"), &["api"]),
            owned_board("drafts", Some("floating"), &["sketch"]),
        ],
    )
}

#[test]
fn builds_a_board_its_element_and_the_element_s_inner_board_in_one_request() {
    let json = r#"[
        {"op": "create_board", "key": "ctx", "title": "Context"},
        {"op": "create_element", "key": "shop", "type_ref": "c4/system", "name": "Shop"},
        {"op": "place_element", "element": {"new": "shop"}, "board": {"new": "ctx"}},
        {"op": "create_board", "key": "inside", "title": "Shop Containers",
         "owner": {"new": "shop"}, "summary": "What the shop runs"}
    ]"#;

    let applied = apply(&snapshot(), &operations(json)).unwrap();

    let inside = written_board(&applied, "shop-containers");
    assert_eq!(inside.owner, Some(element_id("shop")));
    assert_eq!(inside.summary.as_deref(), Some("What the shop runs"));
    assert!(inside.placements.is_empty());
}

#[test]
fn accepts_an_owner_that_gets_its_home_only_later_in_the_request() {
    let json = r#"[
        {"op": "create_element", "key": "x", "type_ref": "t", "name": "X"},
        {"op": "create_board", "key": "inside", "title": "Inside", "owner": {"new": "x"}},
        {"op": "place_element", "element": {"new": "x"}, "board": {"existing": "main"}}
    ]"#;

    let applied = apply(&snapshot(), &operations(json)).unwrap();

    assert_eq!(
        written_board(&applied, "inside").owner,
        Some(element_id("x"))
    );
}

#[rstest]
#[case::an_unknown_id(
    r#"[{"op": "create_board", "key": "b", "title": "B", "owner": {"existing": "nobody"}}]"#,
    OperationProblem::UnknownElement(element_id("nobody"))
)]
#[case::a_placement_without_a_file(
    r#"[{"op": "create_board", "key": "b", "title": "B", "owner": {"existing": "ghost"}}]"#,
    OperationProblem::UnknownElement(element_id("ghost"))
)]
#[case::an_element_deleted_before(
    r#"[{"op": "delete_element", "element": {"existing": "loose"}},
        {"op": "create_board", "key": "b", "title": "B", "owner": {"existing": "loose"}}]"#,
    OperationProblem::UnknownElement(element_id("loose"))
)]
#[case::a_board(
    r#"[{"op": "create_board", "key": "b", "title": "B"},
        {"op": "create_board", "key": "c", "title": "C", "owner": {"new": "b"}}]"#,
    OperationProblem::WrongKind { key: key("b"), expected: Kind::Element, actual: Kind::Board }
)]
#[case::an_update_to_an_unknown_id(
    r#"[{"op": "update_board", "board": {"existing": "main"}, "owner": {"existing": "nobody"}}]"#,
    OperationProblem::UnknownElement(element_id("nobody"))
)]
fn rejects_an_owner_that_is_no_element(#[case] json: &str, #[case] expected: OperationProblem) {
    assert_eq!(problem(&snapshot(), json), expected);
}

#[rstest]
#[case::a_new_board(
    r#"[{"op": "create_board", "key": "b", "title": "B", "owner": {"existing": "shop"}}]"#
)]
#[case::an_update(
    r#"[{"op": "update_board", "board": {"existing": "drafts"}, "owner": {"existing": "shop"}}]"#
)]
fn rejects_a_second_board_for_an_element(#[case] json: &str) {
    assert_eq!(
        problem(&hierarchy(), json),
        OperationProblem::AlreadyOwns {
            element: element_id("shop"),
            board: board_id("containers"),
        }
    );
}

#[rstest]
#[case::owner_on_its_own_board(
    r#"[{"op": "update_board", "board": {"existing": "components"}, "owner": {"existing": "api"}}]"#,
    "components"
)]
#[case::owner_three_levels_below(
    r#"[{"op": "update_board", "board": {"existing": "context"}, "owner": {"existing": "api"}}]"#,
    "context"
)]
#[case::placing_the_owner_on_its_own_board(
    r#"[{"op": "place_element", "element": {"existing": "floating"}, "board": {"existing": "drafts"}}]"#,
    "drafts"
)]
#[case::placing_the_owner_inside_its_board(
    r#"[{"op": "create_element", "key": "s", "type_ref": "t", "name": "S"},
        {"op": "create_board", "key": "below", "title": "Below", "owner": {"existing": "sketch"}},
        {"op": "place_element", "element": {"new": "s"}, "board": {"new": "below"}},
        {"op": "place_element", "element": {"existing": "floating"}, "board": {"new": "below"}}]"#,
    "drafts"
)]
#[case::an_owner_created_in_the_request(
    r#"[{"op": "create_element", "key": "x", "type_ref": "t", "name": "X"},
        {"op": "create_board", "key": "inside", "title": "Inside", "owner": {"new": "x"}},
        {"op": "place_element", "element": {"new": "x"}, "board": {"new": "inside"}}]"#,
    "inside"
)]
fn rejects_a_board_that_would_lie_inside_itself(#[case] json: &str, #[case] board: &str) {
    assert_eq!(
        problem(&hierarchy(), json),
        OperationProblem::InsideItself(board_id(board))
    );
}

/// A snapshot that hand edits damaged: `twice`, which has no home, owns
/// `first` and `second`; `first` places `part`, whose inside is `deeper`;
/// `left` and `right` lie inside each other; `orphaned` is the inside of
/// `gone`, which has no file; `spare` is empty.
fn damaged() -> Snapshot {
    snapshot_of(
        &["twice", "part", "l", "r", "loose", "other"],
        vec![
            owned_board("first", Some("twice"), &["part"]),
            owned_board("second", Some("twice"), &[]),
            owned_board("deeper", Some("part"), &[]),
            owned_board("left", Some("r"), &["l"]),
            owned_board("right", Some("l"), &["r"]),
            owned_board("orphaned", Some("gone"), &[]),
            owned_board("spare", None, &[]),
        ],
    )
}

#[rstest]
#[case::placing_on_a_board_in_a_cycle(
    r#"[{"op": "place_element", "element": {"existing": "loose"}, "board": {"existing": "left"}}]"#
)]
#[case::renaming_a_board_in_a_cycle(
    r#"[{"op": "update_board", "board": {"existing": "left"}, "title": "Left", "summary": "s"}]"#
)]
#[case::keeping_an_owner_of_two_boards(
    r#"[{"op": "update_board", "board": {"existing": "first"}, "owner": {"existing": "twice"}}]"#
)]
#[case::unlinking_one_of_two_boards(
    r#"[{"op": "update_board", "board": {"existing": "second"}, "owner": null}]"#
)]
#[case::unlinking_a_board_of_a_cycle(
    r#"[{"op": "update_board", "board": {"existing": "left"}, "owner": null}]"#
)]
#[case::unlinking_a_board_whose_owner_has_no_file(
    r#"[{"op": "update_board", "board": {"existing": "orphaned"}, "owner": null}]"#
)]
#[case::renaming_a_board_whose_owner_has_no_file(
    r#"[{"op": "update_board", "board": {"existing": "orphaned"}, "title": "Orphaned"}]"#
)]
#[case::an_owner_unrelated_to_the_breaches(
    r#"[{"op": "update_board", "board": {"existing": "spare"}, "owner": {"existing": "other"}}]"#
)]
fn accepts_requests_that_bring_in_no_new_breach(#[case] json: &str) {
    assert!(apply(&damaged(), &operations(json)).is_ok());
}

#[test]
fn never_takes_over_the_board_of_an_owner_without_a_file() {
    let json = r#"[
        {"op": "create_element", "key": "g", "type_ref": "t", "name": "Gone"},
        {"op": "create_board", "key": "b", "title": "Inside", "owner": {"new": "g"}}
    ]"#;

    let applied = apply(&damaged(), &operations(json)).unwrap();

    assert_eq!(
        applied.created,
        vec![
            (key("g"), Created::Element(element_id("gone-2"))),
            (key("b"), Created::Board(board_id("inside"))),
        ]
    );
    assert_eq!(applied.writes.len(), 2);
}

#[rstest]
#[case::a_third_board_for_an_owner_of_two(
    r#"[{"op": "update_board", "board": {"existing": "spare"}, "owner": {"existing": "twice"}}]"#,
    OperationProblem::AlreadyOwns { element: element_id("twice"), board: board_id("first") }
)]
#[case::an_owner_of_two_placed_inside_one_of_them(
    r#"[{"op": "place_element", "element": {"existing": "twice"}, "board": {"existing": "deeper"}}]"#,
    OperationProblem::InsideItself(board_id("first"))
)]
fn rejects_a_new_breach_next_to_breaches_the_files_hold(
    #[case] json: &str,
    #[case] expected: OperationProblem,
) {
    assert_eq!(problem(&damaged(), json), expected);
}

#[test]
fn repairs_one_of_two_breaches_and_leaves_the_other() {
    let json = r#"[{"op": "update_board", "board": {"existing": "second"}, "owner": null}]"#;

    let applied = apply(&damaged(), &operations(json)).unwrap();

    assert_eq!(written_board(&applied, "second").owner, None);
    assert_eq!(applied.writes.len(), 1);
}

#[test]
fn rejects_deleting_an_owner_and_names_its_board() {
    let json = r#"[{"op": "delete_element", "element": {"existing": "payment"}}]"#;

    assert_eq!(
        problem(&hierarchy(), json),
        OperationProblem::OwnsBoard {
            element: element_id("payment"),
            board: board_id("components"),
        }
    );
}

#[test]
fn deletes_an_owner_after_its_board_is_unlinked_in_the_same_request() {
    let json = r#"[
        {"op": "update_board", "board": {"existing": "components"}, "owner": null},
        {"op": "delete_element", "element": {"existing": "payment"}}
    ]"#;

    let applied = apply(&hierarchy(), &operations(json)).unwrap();

    assert_eq!(written_board(&applied, "components").owner, None);
    assert!(
        !written_board(&applied, "containers")
            .placements
            .contains_key(&element_id("payment"))
    );
}

#[test]
fn updates_only_the_named_fields_and_keeps_id_and_placements() {
    let json = r#"[
        {"op": "update_board", "board": {"existing": "components"},
         "title": "Payment Components", "summary": "The parts of payment"}
    ]"#;

    let applied = apply(&hierarchy(), &operations(json)).unwrap();

    let board = written_board(&applied, "components");
    assert_eq!(board.id, board_id("components"));
    assert_eq!(board.title, "Payment Components");
    assert_eq!(board.summary.as_deref(), Some("The parts of payment"));
    assert_eq!(board.owner, Some(element_id("payment")));
    assert_eq!(
        board.placements,
        hierarchy().boards[&board_id("components")].placements
    );
}

#[test]
fn removes_a_summary_and_an_owner_with_null() {
    let mut snapshot = hierarchy();
    if let Some(board) = snapshot.boards.get_mut(&board_id("components")) {
        board.summary = Some("old".to_owned());
    }
    let json = r#"[
        {"op": "update_board", "board": {"existing": "components"}, "summary": null, "owner": null}
    ]"#;

    let applied = apply(&snapshot, &operations(json)).unwrap();

    let board = written_board(&applied, "components");
    assert_eq!((&board.summary, &board.owner), (&None, &None));
}

#[test]
fn creates_a_board_with_a_zone() {
    let json = r#"[{"op": "create_board", "key": "b", "title": "Ideas",
        "zone": {"mode": "free", "statement": "idea"}}]"#;

    let applied = apply(&snapshot(), &operations(json)).unwrap();

    assert_eq!(
        written_board(&applied, "ideas").zone,
        Some(Zone {
            mode: Mode::Free,
            statement: StatementKind::Idea,
        })
    );
}

#[test]
fn sets_changes_and_removes_a_zone_and_keeps_every_other_field() {
    let set = r#"[{"op": "update_board", "board": {"existing": "components"},
        "zone": {"mode": "free", "statement": "target"}}]"#;
    let change = r#"[{"op": "update_board", "board": {"existing": "components"},
        "zone": {"mode": "mirrored", "statement": "current"}}]"#;
    let remove = r#"[{"op": "update_board", "board": {"existing": "components"}, "zone": null}]"#;
    let before = hierarchy().boards[&board_id("components")].clone();
    let mut snapshot = hierarchy();
    let mut zones = Vec::new();

    for json in [set, change, remove] {
        let applied = apply(&snapshot, &operations(json)).unwrap();
        let board = written_board(&applied, "components").clone();
        assert_eq!(
            (
                &board.id,
                &board.title,
                &board.summary,
                &board.owner,
                &board.placements
            ),
            (
                &before.id,
                &before.title,
                &before.summary,
                &before.owner,
                &before.placements
            )
        );
        zones.push(board.zone);
        snapshot.boards.insert(board.id.clone(), board);
    }

    assert_eq!(
        zones,
        [
            Some(Zone {
                mode: Mode::Free,
                statement: StatementKind::Target,
            }),
            Some(Zone {
                mode: Mode::Mirrored,
                statement: StatementKind::Current,
            }),
            None,
        ]
    );
}

#[test]
fn moves_a_board_to_another_owner_that_owns_none() {
    let json = r#"[
        {"op": "update_board", "board": {"existing": "drafts"}, "owner": {"existing": "loose"}}
    ]"#;

    let applied = apply(&hierarchy(), &operations(json)).unwrap();

    assert_eq!(
        written_board(&applied, "drafts").owner,
        Some(element_id("loose"))
    );
}

#[rstest]
#[case::nothing(
    r#"[{"op": "update_board", "board": {"existing": "main"}}]"#,
    OperationProblem::NothingToUpdateBoard(board_id("main"))
)]
#[case::an_unknown_board(
    r#"[{"op": "update_board", "board": {"existing": "nowhere"}, "title": "T"}]"#,
    OperationProblem::UnknownBoard(board_id("nowhere"))
)]
fn rejects_updates_of_boards_that_cannot_apply(
    #[case] json: &str,
    #[case] expected: OperationProblem,
) {
    assert_eq!(problem(&snapshot(), json), expected);
}

#[rstest]
#[case::creating_one(
    r#"[{"op": "create_board", "key": "b", "title": "SECRET-TITLE", "summary": "SECRET-SUMMARY",
         "owner": {"existing": "shop"}}]"#
)]
#[case::updating_one(
    r#"[{"op": "update_board", "board": {"existing": "context"}, "title": "SECRET-TITLE",
         "summary": "SECRET-SUMMARY", "owner": {"existing": "api"}}]"#
)]
fn shows_neither_title_nor_summary_when_it_rejects_a_board_operation(#[case] json: &str) {
    let message = problem(&hierarchy(), json).to_string();

    assert!(!message.contains("SECRET"), "{message}");
}

/// `snapshot()` with `orders-db` pinned on `main`.
fn with_a_pin() -> Snapshot {
    let mut snapshot = snapshot();
    snapshot
        .boards
        .get_mut(&board_id("main"))
        .unwrap()
        .placements
        .get_mut(&element_id("orders-db"))
        .unwrap()
        .pinned = true;
    snapshot
}

/// A move of `orders-db` on `main` to 30, 40, with `pin` as its last fields.
fn a_move_of_orders_db(pin: &str) -> String {
    format!(
        r#"{{"op": "move_element", "element": {{"existing": "orders-db"}}, "board": {{"existing": "main"}}, "x": 30, "y": 40{pin}}}"#
    )
}

#[rstest]
#[case::unpinned_without_a_pin(snapshot(), "", false)]
#[case::unpinned_with_true(snapshot(), r#", "pinned": true"#, true)]
#[case::unpinned_with_false(snapshot(), r#", "pinned": false"#, false)]
#[case::pinned_with_true(with_a_pin(), r#", "pinned": true"#, true)]
#[case::pinned_with_false(with_a_pin(), r#", "pinned": false"#, false)]
fn a_move_sets_the_position_and_the_pin(
    #[case] before: Snapshot,
    #[case] pin: &str,
    #[case] pinned: bool,
) {
    let json = format!("[{}]", a_move_of_orders_db(pin));

    let applied = apply(&before, &operations(&json)).unwrap();

    let moved = &written_board(&applied, "main").placements[&element_id("orders-db")];
    assert_eq!((moved.rect.x, moved.rect.y), (30, 40));
    assert_eq!(moved.pinned, pinned);
}

#[test]
fn a_move_of_a_pinned_element_without_a_pin_is_refused_and_tells_the_ai_to_ask() {
    let json = format!("[{}]", a_move_of_orders_db(""));

    let refused = problem(&with_a_pin(), &json);

    assert_eq!(
        refused,
        OperationProblem::Pinned {
            element: element_id("orders-db"),
            board: board_id("main"),
        }
    );
    assert_eq!(
        refused.to_string(),
        "element orders-db is pinned on board main: a person placed it there. Move it only after \
         the person has said so, with pinned: true to keep it pinned or pinned: false to release it"
    );
}

#[test]
fn a_pin_the_request_set_refuses_a_later_plain_move() {
    let json = format!(
        "[{}, {}]",
        a_move_of_orders_db(r#", "pinned": true"#),
        a_move_of_orders_db("")
    );

    assert!(matches!(
        problem(&snapshot(), &json),
        OperationProblem::Pinned { .. }
    ));
}

#[test]
fn a_pin_the_request_released_lets_a_later_plain_move_through() {
    let json = format!(
        "[{}, {}]",
        a_move_of_orders_db(r#", "pinned": false"#),
        a_move_of_orders_db("")
    );

    let applied = apply(&with_a_pin(), &operations(&json)).unwrap();

    let moved = &written_board(&applied, "main").placements[&element_id("orders-db")];
    assert_eq!((moved.rect.x, moved.pinned), (30, false));
}

#[test]
fn a_refused_move_of_a_pinned_element_writes_nothing_of_the_request() {
    let json = format!(
        r#"[{{"op": "place_element", "element": {{"existing": "loose"}}, "board": {{"existing": "main"}}}}, {}]"#,
        a_move_of_orders_db("")
    );

    assert!(matches!(
        apply(&with_a_pin(), &operations(&json)),
        Err(ApplyError::Operation { .. })
    ));
}

#[test]
fn a_pin_holds_for_one_board_only() {
    let mut before = snapshot_for_deletion();
    before
        .boards
        .get_mut(&board_id("main"))
        .unwrap()
        .placements
        .get_mut(&element_id("orders-db"))
        .unwrap()
        .pinned = true;
    let json = r#"[{"op": "move_element", "element": {"existing": "orders-db"}, "board": {"existing": "side"}, "x": 30, "y": 40}]"#;

    let applied = apply(&before, &operations(json)).unwrap();

    let moved = &written_board(&applied, "side").placements[&element_id("orders-db")];
    assert_eq!((moved.rect.x, moved.pinned), (30, false));
}

#[test]
fn deleting_a_pinned_element_removes_its_placement() {
    let json = r#"[{"op": "delete_element", "element": {"existing": "orders-db"}}]"#;

    let applied = apply(&with_a_pin(), &operations(json)).unwrap();

    assert!(
        !written_board(&applied, "main")
            .placements
            .contains_key(&element_id("orders-db"))
    );
}

#[test]
fn other_operations_on_the_board_keep_a_pin() {
    let json = r#"[
        {"op": "place_element", "element": {"existing": "loose"}, "board": {"existing": "main"}},
        {"op": "update_board", "board": {"existing": "main"}, "title": "Renamed"}
    ]"#;

    let applied = apply(&with_a_pin(), &operations(json)).unwrap();

    assert!(written_board(&applied, "main").placements[&element_id("orders-db")].pinned);
}

/// A request that creates an element with `summary` and places it on `main`.
fn place_with_summary(summary: &str) -> String {
    format!(
        r#"[
        {{"op": "create_element", "key": "e", "type_ref": "c4/container", "name": "E", "summary": "{summary}"}},
        {{"op": "place_element", "element": {{"new": "e"}}, "board": {{"existing": "main"}}}}
    ]"#
    )
}

/// The placement `key` received in `applied`, on `main`.
fn placement_of<'a>(applied: &'a Applied, key_name: &str) -> &'a Placement {
    let id = applied
        .created
        .iter()
        .find_map(|(key_of, created)| match created {
            Created::Element(id) if key_of == &key(key_name) => Some(id),
            Created::Element(_) | Created::Board(_) => None,
        })
        .unwrap();
    &written_board(applied, "main").placements[id]
}

#[test]
fn place_gives_a_short_text_the_default_size() {
    let applied = apply(&snapshot(), &operations(&place_with_summary("short"))).unwrap();

    let placed = placement_of(&applied, "e");
    assert_eq!(
        (placed.rect.width.get(), placed.rect.height.get()),
        (180, 90)
    );
}

#[test]
fn place_gives_a_long_text_a_tall_box_that_overlaps_nothing() {
    let json = place_with_summary(&"word ".repeat(40));

    let applied = apply(&snapshot(), &operations(&json)).unwrap();

    let placed = placement_of(&applied, "e");
    assert_eq!(placed.rect.width.get(), 180);
    assert!(placed.rect.height.get() > 90, "{placed:?}");
    let board = written_board(&applied, "main");
    for (id, other) in &board.placements {
        assert!(
            *id == element_id("e") || !other.rect.overlaps(&placed.rect),
            "{id} overlaps the tall box"
        );
    }
}

#[test]
fn place_measures_the_text_as_the_request_stands() {
    let json = format!(
        r#"[
        {{"op": "create_element", "key": "e", "type_ref": "c4/container", "name": "E"}},
        {{"op": "update_element", "element": {{"new": "e"}}, "summary": "{}"}},
        {{"op": "place_element", "element": {{"new": "e"}}, "board": {{"existing": "main"}}}}
    ]"#,
        "word ".repeat(40)
    );

    let applied = apply(&snapshot(), &operations(&json)).unwrap();

    assert!(placement_of(&applied, "e").rect.height.get() > 90);
}

#[test]
fn place_refuses_an_element_the_request_deleted() {
    let json = r#"[
        {"op": "delete_element", "element": {"existing": "loose"}},
        {"op": "place_element", "element": {"existing": "loose"}, "board": {"existing": "main"}}
    ]"#;

    assert_eq!(
        problem(&snapshot(), json),
        OperationProblem::UnknownElement(element_id("loose"))
    );
}

#[rstest]
#[case::move_(r#"{"op": "move_element", "element": {"existing": "tall"}, "board": {"existing": "main"}, "x": 5, "y": 6}"#)]
#[case::update(r#"{"op": "update_element", "element": {"existing": "tall"}, "summary": "short"}"#)]
fn a_move_or_an_update_leaves_a_placement_its_size(#[case] operation: &str) {
    let mut before = snapshot();
    let tall = Element::new(
        element_id("tall"),
        ElementFields {
            statement: None,
            type_ref: TypeRef::try_from("c4/container".to_owned()).unwrap(),
            name: "Tall".to_owned(),
            summary: None,
            body: String::new(),
            relations: Relations::new(),
            anchor: None,
            verified: None,
        },
    )
    .unwrap();
    before.elements.insert(element_id("tall"), tall);
    before
        .boards
        .get_mut(&board_id("main"))
        .unwrap()
        .placements
        .insert(element_id("tall"), sized(1000, 0, 240, 170));

    let applied = apply(&before, &operations(&format!("[{operation}]"))).unwrap();

    let board = applied.writes.iter().find_map(|write| match write {
        Write::Board(board) => Some(board),
        Write::Element(_) | Write::RemoveElement(_) => None,
    });
    let placements = &board
        .unwrap_or_else(|| &before.boards[&board_id("main")])
        .placements;
    let rect = placements[&element_id("tall")].rect;
    assert_eq!((rect.width.get(), rect.height.get()), (240, 170));
}

#[test]
fn a_full_request_of_long_texts_lays_out_without_overlap() {
    let summary = "word ".repeat(800);
    let elements = MAX_OPERATIONS / 2;
    let requests: Vec<String> = (0..elements)
        .flat_map(|index| {
            [
                format!(
                    r#"{{"op": "create_element", "key": "e{index}", "type_ref": "c4/container", "name": "E{index}", "summary": "{summary}"}}"#
                ),
                format!(
                    r#"{{"op": "place_element", "element": {{"new": "e{index}"}}, "board": {{"existing": "main"}}}}"#
                ),
            ]
        })
        .collect();

    let applied = apply(
        &snapshot(),
        &operations(&format!("[{}]", requests.join(","))),
    )
    .unwrap();

    let board = written_board(&applied, "main");
    let rects: Vec<&Rect> = board
        .placements
        .values()
        .map(|placed| &placed.rect)
        .collect();
    for (index, rect) in rects.iter().enumerate() {
        assert!(
            rects
                .iter()
                .skip(index + 1)
                .all(|other| !other.overlaps(rect))
        );
    }
}

/// A request that builds the board `b` with `a` using `c`, and lays it out.
const BUILD_AND_LAY_OUT: &str = r#"[
    {"op": "create_board", "key": "b", "title": "B"},
    {"op": "create_element", "key": "a", "type_ref": "c4/container", "name": "A"},
    {"op": "create_element", "key": "c", "type_ref": "c4/container", "name": "C"},
    {"op": "place_element", "element": {"new": "c"}, "board": {"new": "b"}},
    {"op": "place_element", "element": {"new": "a"}, "board": {"new": "b"}},
    {"op": "create_relation", "from": {"new": "c"}, "kind": "uses", "to": {"new": "a"}},
    {"op": "layout_board", "board": {"new": "b"}}
]"#;

#[test]
fn layout_board_lays_out_a_board_of_the_same_request_by_its_relations() {
    let applied = apply(&snapshot(), &operations(BUILD_AND_LAY_OUT)).unwrap();

    let board = written_board(&applied, "b");
    let (a, c) = (
        &board.placements[&element_id("a")],
        &board.placements[&element_id("c")],
    );
    assert!(c.rect.bottom() <= i64::from(a.rect.y), "{board:?}");
    assert!(!a.pinned && !c.pinned);
}

#[test]
fn layout_board_sees_the_request_as_it_stands() {
    let later_relation = r#"[
        {"op": "create_board", "key": "b", "title": "B"},
        {"op": "create_element", "key": "a", "type_ref": "c4/container", "name": "A"},
        {"op": "create_element", "key": "c", "type_ref": "c4/container", "name": "C"},
        {"op": "place_element", "element": {"new": "a"}, "board": {"new": "b"}},
        {"op": "place_element", "element": {"new": "c"}, "board": {"new": "b"}},
        {"op": "layout_board", "board": {"new": "b"}},
        {"op": "create_relation", "from": {"new": "c"}, "kind": "uses", "to": {"new": "a"}}
    ]"#;

    let applied = apply(&snapshot(), &operations(later_relation)).unwrap();

    let board = written_board(&applied, "b");
    assert_eq!(
        board.placements[&element_id("a")].rect.y,
        board.placements[&element_id("c")].rect.y,
        "a relation that comes after the layout counts for nothing"
    );
}

#[test]
fn layout_board_does_not_see_an_element_the_request_deleted() {
    let json = r#"[
        {"op": "delete_element", "element": {"existing": "orders-db"}},
        {"op": "layout_board", "board": {"existing": "main"}}
    ]"#;

    let applied = apply(&snapshot(), &operations(json)).unwrap();

    let board = written_board(&applied, "main");
    assert!(!board.placements.contains_key(&element_id("orders-db")));
    assert_eq!(
        board.placements[&element_id("ghost")].rect,
        placed(0, 0).rect
    );
}

#[test]
fn layout_board_measures_text_the_request_changed() {
    let json = format!(
        r#"[
        {{"op": "update_element", "element": {{"existing": "orders-db"}}, "summary": "{}"}},
        {{"op": "layout_board", "board": {{"existing": "main"}}}}
    ]"#,
        "word ".repeat(40)
    );

    let applied = apply(&snapshot(), &operations(&json)).unwrap();

    let moved = &written_board(&applied, "main").placements[&element_id("orders-db")];
    assert!(moved.rect.height.get() > 90, "{moved:?}");
}

#[test]
fn layout_board_keeps_a_pin_and_lays_a_box_without_a_file_out_at_the_default_size() {
    let before = with_a_pin();
    let pinned = before.boards[&board_id("main")].placements[&element_id("orders-db")].clone();
    let json = r#"[{"op": "layout_board", "board": {"existing": "main"}}]"#;

    let applied = apply(&before, &operations(json)).unwrap();

    let board = written_board(&applied, "main");
    assert_eq!(board.placements[&element_id("orders-db")], pinned);
    let ghost = &board.placements[&element_id("ghost")];
    assert_eq!((ghost.rect.width.get(), ghost.rect.height.get()), (180, 90));
    assert!(!ghost.rect.overlaps(&pinned.rect));
}

#[test]
fn layout_board_writes_only_the_board_file() {
    let json = r#"[{"op": "layout_board", "board": {"existing": "main"}}]"#;

    let applied = apply(&snapshot(), &operations(json)).unwrap();

    assert!(matches!(applied.writes.as_slice(), [Write::Board(_)]));
}

#[test]
fn layout_board_keeps_a_verification_stamp() {
    let before = stamped_snapshot();
    let boards: Vec<BoardId> = before.boards.keys().cloned().collect();
    for board in boards {
        let json = format!(r#"[{{"op": "layout_board", "board": {{"existing": "{board}"}}}}]"#);

        let applied = apply(&before, &operations(&json)).unwrap();

        assert!(
            applied
                .writes
                .iter()
                .all(|write| matches!(write, Write::Board(_))),
            "{board}"
        );
    }
}

#[test]
fn a_placement_after_a_layout_overlaps_no_box_the_layout_produced() {
    let json = r#"[
        {"op": "layout_board", "board": {"existing": "main"}},
        {"op": "place_element", "element": {"existing": "loose"}, "board": {"existing": "main"}}
    ]"#;

    let applied = apply(&snapshot(), &operations(json)).unwrap();

    let board = written_board(&applied, "main");
    let placed = &board.placements[&element_id("loose")];
    for (id, other) in &board.placements {
        assert!(
            *id == element_id("loose") || !other.rect.overlaps(&placed.rect),
            "{id} overlaps the new box"
        );
    }
}

#[test]
fn a_placement_after_a_layout_overlaps_a_pin_outside_the_grid_nowhere() {
    let mut before = with_a_pin();
    before
        .boards
        .get_mut(&board_id("main"))
        .unwrap()
        .placements
        .get_mut(&element_id("orders-db"))
        .unwrap()
        .rect = sized(0, 9_000, 1_000, 1_000).rect;
    let json = r#"[
        {"op": "layout_board", "board": {"existing": "main"}},
        {"op": "place_element", "element": {"existing": "loose"}, "board": {"existing": "main"}}
    ]"#;

    let applied = apply(&before, &operations(json)).unwrap();

    let board = written_board(&applied, "main");
    let placed = &board.placements[&element_id("loose")];
    assert!(
        !placed
            .rect
            .overlaps(&board.placements[&element_id("orders-db")].rect)
    );
}

#[test]
fn a_later_failure_writes_nothing_of_a_layout() {
    let json = r#"[
        {"op": "layout_board", "board": {"existing": "main"}},
        {"op": "place_element", "element": {"existing": "nope"}, "board": {"existing": "main"}}
    ]"#;

    assert!(matches!(
        apply(&snapshot(), &operations(json)),
        Err(ApplyError::Operation { position: 2, .. })
    ));
}

#[test]
fn layout_board_refuses_a_board_no_position_fits() {
    let mut before = with_a_pin();
    before
        .boards
        .get_mut(&board_id("main"))
        .unwrap()
        .placements
        .get_mut(&element_id("orders-db"))
        .unwrap()
        .rect = sized(i32::MIN, i32::MIN, u32::MAX, u32::MAX).rect;
    let json = r#"[{"op": "layout_board", "board": {"existing": "main"}}]"#;

    assert_eq!(
        problem(&before, json),
        OperationProblem::BoardFull(board_id("main"))
    );
}

#[test]
fn layout_board_refuses_a_board_it_does_not_know() {
    let json = r#"[{"op": "layout_board", "board": {"existing": "nope"}}]"#;

    assert_eq!(
        problem(&snapshot(), json),
        OperationProblem::UnknownBoard(board_id("nope"))
    );
}

#[test]
fn the_layouts_of_a_request_share_their_work() {
    // 100 boxes of text that cost 902 units each, and so 90,200 per layout.
    let text = " ".repeat(64 * 900);
    let mut before = snapshot();
    let main = before.boards.get_mut(&board_id("main")).unwrap();
    main.placements.clear();
    for index in 0..100 {
        let id = element_id(&format!("e{index}"));
        before.elements.insert(
            id.clone(),
            Element::new(
                id.clone(),
                ElementFields {
                    statement: None,
                    type_ref: TypeRef::try_from("t".to_owned()).unwrap(),
                    name: "n".to_owned(),
                    summary: Some(text.clone()),
                    body: String::new(),
                    relations: Relations::new(),
                    anchor: None,
                    verified: None,
                },
            )
            .unwrap(),
        );
        before
            .boards
            .get_mut(&board_id("main"))
            .unwrap()
            .placements
            .insert(id, placed(index * 220, 0));
    }
    let layouts = |count: usize| {
        let one = r#"{"op": "layout_board", "board": {"existing": "main"}}"#;
        format!("[{}]", vec![one; count].join(","))
    };

    assert!(apply(&before, &operations(&layouts(44))).is_ok());
    assert_eq!(
        problem(&before, &layouts(45)),
        OperationProblem::LayoutTooLarge(board_id("main"))
    );
}

/// A `create_element` under the key `k` with an empty type and an invalid
/// anchor, each of which alone refuses the request.
const INVALID_NEW_ELEMENT: &str =
    r#"{"op": "create_element", "key": "k", "type_ref": "", "name": "N", "anchor": "../x"}"#;

#[rstest]
#[case::a_taken_key_before_the_type_and_the_anchor(
    &format!(
        r#"[{{"op": "create_board", "key": "k", "title": "B"}}, {INVALID_NEW_ELEMENT}]"#
    ),
    OperationProblem::DuplicateKey(key("k"))
)]
#[case::the_type_before_the_anchor_on_create(
    &format!("[{INVALID_NEW_ELEMENT}]"),
    OperationProblem::InvalidElement(ElementProblem::EmptyType)
)]
#[case::the_type_before_the_anchor_on_update(
    r#"[{"op": "update_element", "element": {"existing": "loose"}, "type_ref": "", "anchor": "../x"}]"#,
    OperationProblem::InvalidElement(ElementProblem::EmptyType)
)]
fn a_request_with_several_invalid_values_gets_the_first_error_in_a_fixed_order(
    #[case] json: &str,
    #[case] expected: OperationProblem,
) {
    assert_eq!(problem(&snapshot(), json), expected);
}

#[rstest]
#[case::on_create(
    r#"[{"op": "create_element", "key": "k", "type_ref": "t", "name": "N", "anchor": "../x"}]"#
)]
#[case::on_update(
    r#"[{"op": "update_element", "element": {"existing": "loose"}, "anchor": "../x"}]"#
)]
fn the_anchor_of_those_requests_is_invalid_on_its_own(#[case] json: &str) {
    assert!(matches!(
        problem(&snapshot(), json),
        OperationProblem::InvalidElement(_)
    ));
}

#[test]
fn creates_an_element_with_a_statement_or_without_one_on_null() {
    let json = r#"[
        {"op": "create_element", "key": "a", "type_ref": "t", "name": "A", "statement": "question"},
        {"op": "create_element", "key": "b", "type_ref": "t", "name": "B", "statement": null},
        {"op": "create_element", "key": "c", "type_ref": "t", "name": "C"}
    ]"#;

    let applied = apply(&snapshot(), &operations(json)).unwrap();

    let kinds: Vec<Option<StatementKind>> = applied
        .writes
        .iter()
        .filter_map(|write| match write {
            Write::Element(element) => Some(element.statement()),
            Write::Board(_) | Write::RemoveElement(_) => None,
        })
        .collect();
    assert_eq!(kinds, [Some(StatementKind::Question), None, None]);
}

/// What an update with `fields` writes for `loose`, after an earlier update
/// gave it a kind.
fn updated_kind(fields: &str) -> Element {
    let json = format!(
        r#"[
        {{"op": "update_element", "element": {{"existing": "loose"}}, "statement": "idea"}},
        {{"op": "update_element", "element": {{"existing": "loose"}}{fields}}}
    ]"#
    );
    let applied = apply(&snapshot(), &operations(&json)).unwrap();
    applied
        .writes
        .iter()
        .find_map(|write| match write {
            Write::Element(element) if element.id() == &element_id("loose") => {
                Some(element.clone())
            }
            Write::Element(_) | Write::Board(_) | Write::RemoveElement(_) => None,
        })
        .unwrap()
}

#[rstest]
#[case::a_new_kind(r#", "statement": "target""#, Some(StatementKind::Target))]
#[case::null_removes_it(r#", "statement": null"#, None)]
#[case::another_field_keeps_it(r#", "summary": "S""#, Some(StatementKind::Idea))]
fn an_update_sets_removes_or_keeps_the_statement(
    #[case] fields: &str,
    #[case] expected: Option<StatementKind>,
) {
    assert_eq!(updated_kind(fields).statement(), expected);
}

#[test]
fn a_relation_keeps_the_statement() {
    let json = r#"[
        {"op": "update_element", "element": {"existing": "loose"}, "statement": "idea"},
        {"op": "create_relation", "from": {"existing": "loose"}, "kind": "uses", "to": {"existing": "orders-db"}},
        {"op": "delete_relation", "from": {"existing": "loose"}, "kind": "uses", "to": {"existing": "orders-db"}}
    ]"#;

    let applied = apply(&snapshot(), &operations(json)).unwrap();

    let kept = applied.writes.iter().any(|write| {
        matches!(write, Write::Element(element)
            if element.id() == &element_id("loose")
                && element.statement() == Some(StatementKind::Idea))
    });
    assert!(kept, "{:?}", applied.writes);
}

#[rstest]
#[case::a_kind(r#""statement": "idea""#)]
#[case::null_on_an_element_without_one(r#""statement": null"#)]
fn an_update_of_only_the_statement_is_an_update_and_removes_the_stamp(#[case] field: &str) {
    let before = stamped_snapshot();
    let stamped = before
        .elements
        .values()
        .find(|element| element.verified().is_some())
        .unwrap()
        .id()
        .clone();
    let json =
        format!(r#"[{{"op": "update_element", "element": {{"existing": "{stamped}"}}, {field}}}]"#);

    let applied = apply(&before, &operations(&json)).unwrap();

    let written = applied.writes.iter().find_map(|write| match write {
        Write::Element(element) if element.id() == &stamped => Some(element),
        Write::Element(_) | Write::Board(_) | Write::RemoveElement(_) => None,
    });
    assert_eq!(written.unwrap().verified(), None);
}

#[test]
fn a_statement_past_the_frontmatter_limit_writes_nothing_of_the_request() {
    let largest = (abyme_model::MAX_FRONTMATTER_BYTES - 200..=abyme_model::MAX_FRONTMATTER_BYTES)
        .rev()
        .find(|length| {
            let json = format!(
                r#"[{{"op": "update_element", "element": {{"existing": "loose"}}, "summary": "{}"}}]"#,
                "a".repeat(*length)
            );
            apply(&snapshot(), &operations(&json)).is_ok()
        })
        .unwrap();
    let json = format!(
        r#"[
        {{"op": "update_element", "element": {{"existing": "loose"}}, "summary": "{}"}},
        {{"op": "update_element", "element": {{"existing": "loose"}}, "statement": "idea"}}
    ]"#,
        "a".repeat(largest)
    );

    assert!(matches!(
        problem(&snapshot(), &json),
        OperationProblem::InvalidElement(ElementProblem::FrontmatterTooLarge { .. })
    ));
}

/// The placement line of `element` at `x` and `y`, 180 by 90, with `rest`
/// after the height.
fn placement_line(element: &str, x: i32, y: i32, rest: &str) -> String {
    format!(r#"{{"element": "{element}", "x": {x}, "y": {y}, "width": 180, "height": 90{rest}}}"#)
}

/// The board `id` read from a file with these placement lines.
fn board_file(id: &str, lines: &[String]) -> Board {
    let text = format!(
        r#"{{"format": 1, "id": "{id}", "title": "t", "placements": [{}]}}"#,
        lines.join(", ")
    );
    Board::from_json(&board_id(id), &text).unwrap()
}

/// A snapshot with the boards given and a file for each element they place.
fn with_files_for(boards: Vec<Board>) -> Snapshot {
    Snapshot {
        elements: boards
            .iter()
            .flat_map(|board| board.placements.keys())
            .map(|id| (id.clone(), element(id.as_str())))
            .collect(),
        boards: boards
            .into_iter()
            .map(|board| (board.id.clone(), board))
            .collect(),
    }
}

/// A snapshot whose board `main` has the box `b` at 100, 200 and the note `n`
/// hanging on it at the offset 5, 6, plus the given lines.
fn with_a_note(more: &[String]) -> Snapshot {
    let mut lines = vec![
        placement_line("b", 100, 200, ""),
        placement_line("n", 5, 6, r#", "attached_to": "b""#),
    ];
    lines.extend_from_slice(more);
    with_files_for(vec![board_file("main", &lines)])
}

fn a_move(element: &str, x: i32, y: i32) -> String {
    format!(
        r#"{{"op": "move_element", "element": {{"existing": "{element}"}}, "board": {{"existing": "main"}}, "x": {x}, "y": {y}}}"#
    )
}

fn rect_at(x: i32, y: i32) -> Rect {
    Rect {
        x,
        y,
        width: NonZeroU32::new(180).unwrap(),
        height: NonZeroU32::new(90).unwrap(),
    }
}

/// The line of `element` in the file of `board`.
fn line_of(board: &Board, element: &str) -> String {
    let needle = format!(r#""element": "{element}""#);
    board
        .to_json()
        .lines()
        .find(|line| line.contains(&needle))
        .unwrap()
        .to_owned()
}

#[test]
fn moving_a_box_carries_its_note_and_writes_only_the_box_line() {
    let before = with_a_note(&[]);
    let json = format!("[{}]", a_move("b", 300, 400));

    let applied = apply(&before, &operations(&json)).unwrap();

    let board = written_board(&applied, "main");
    let was = &before.boards[&board_id("main")];
    assert_eq!(board.rect_of(&element_id("n")), Some(rect_at(305, 406)));
    assert_eq!(line_of(board, "n"), line_of(was, "n"));
    assert_ne!(line_of(board, "b"), line_of(was, "b"));
}

#[test]
fn a_placement_after_moving_a_box_overlaps_no_note_where_it_now_lies() {
    let mut before = with_a_note(&[]);
    before
        .elements
        .insert(element_id("loose"), element("loose"));
    // The box moves far away and its note lands on the first free cell.
    let json = format!(
        r#"[{}, {{"op": "place_element", "element": {{"existing": "loose"}}, "board": {{"existing": "main"}}}}]"#,
        a_move("b", -5, -6)
    );

    let applied = apply(&before, &operations(&json)).unwrap();

    let board = written_board(&applied, "main");
    let note = board.rect_of(&element_id("n")).unwrap();
    let placed = board.rect_of(&element_id("loose")).unwrap();
    assert_eq!(note, rect_at(0, 0));
    assert!(!placed.overlaps(&note), "{placed:?} overlaps {note:?}");
}

#[test]
fn moving_a_note_takes_a_board_position_and_stores_it_relative_to_its_box() {
    let json = format!("[{}]", a_move("n", 50, 60));

    let applied = apply(&with_a_note(&[]), &operations(&json)).unwrap();

    let board = written_board(&applied, "main");
    assert_eq!(board.rect_of(&element_id("n")), Some(rect_at(50, 60)));
    assert_eq!(board.attachment(&element_id("n")), Some(&element_id("b")));
    assert!(
        line_of(board, "n").contains(r#""x": -50, "y": -140"#),
        "{}",
        line_of(board, "n")
    );
}

#[test]
fn a_move_of_a_box_that_would_carry_its_note_out_of_range_is_refused() {
    let before = with_a_note(&[placement_line("m", 500, 0, r#", "attached_to": "b""#)]);
    let json = format!("[{}]", a_move("b", MAX_COORDINATE, 0));

    let refused = problem(&before, &json);

    assert_eq!(
        refused,
        OperationProblem::LeavesRange {
            element: element_id("m"),
            board: board_id("main"),
        }
    );
    assert_eq!(
        refused.to_string(),
        "element m hangs on a box on board main and would lie outside -1000000 to 1000000"
    );
}

#[test]
fn a_move_of_a_note_whose_box_lies_too_far_to_store_an_offset_is_refused() {
    let before = with_files_for(vec![board_file(
        "main",
        &[
            placement_line("b", i32::MIN, 0, ""),
            placement_line("n", 0, 0, r#", "attached_to": "b""#),
        ],
    )]);
    let json = format!("[{}]", a_move("n", MAX_COORDINATE, 0));

    let refused = problem(&before, &json);

    assert_eq!(
        refused,
        OperationProblem::PositionNotStored {
            element: element_id("n"),
            board: board_id("main"),
        }
    );
}

fn layout_of_main() -> String {
    r#"[{"op": "layout_board", "board": {"existing": "main"}}]"#.to_owned()
}

#[test]
fn a_layout_leaves_a_note_on_its_box_and_its_line_unchanged() {
    // The note `b` lies between the boxes `a` and `c` in ID order and hangs on
    // `a`, so a write-back by position would give it a box's rect.
    let mut before = Snapshot {
        elements: BTreeMap::new(),
        boards: BTreeMap::from([(
            board_id("main"),
            board_file(
                "main",
                &[
                    placement_line("a", 900, 900, ""),
                    placement_line("b", 5, 6, r#", "pinned": true, "attached_to": "a""#),
                    placement_line("c", 0, 0, ""),
                ],
            ),
        )]),
    };
    for id in ["a", "c"] {
        before.elements.insert(element_id(id), element(id));
    }

    let applied = apply(&before, &operations(&layout_of_main())).unwrap();

    let board = written_board(&applied, "main");
    let was = &before.boards[&board_id("main")];
    let a = board.rect_of(&element_id("a")).unwrap();
    assert_eq!(line_of(board, "b"), line_of(was, "b"));
    assert_eq!(
        board.rect_of(&element_id("b")),
        Some(rect_at(a.x + 5, a.y + 6))
    );
    assert_ne!(a, rect_at(900, 900));
}

#[test]
fn a_layout_reads_no_relation_of_a_note_and_lets_a_box_lie_under_it() {
    let boxes = |with_note: bool| {
        let mut lines = vec![
            placement_line("a", 0, 0, ""),
            placement_line("c", 0, 300, ""),
        ];
        let mut elements = BTreeMap::from([
            (element_id("a"), element("a")),
            (element_id("c"), element("c")),
        ]);
        if with_note {
            lines.push(placement_line("n", 0, 0, r#", "attached_to": "c""#));
            let mut fields = element("n").into_fields();
            fields
                .relations
                .insert((RelationKind::DependsOn, element_id("a")), None);
            fields
                .relations
                .insert((RelationKind::Concerns, element_id("c")), None);
            elements.insert(
                element_id("n"),
                Element::new(element_id("n"), fields).unwrap(),
            );
        }
        Snapshot {
            elements,
            boards: BTreeMap::from([(board_id("main"), board_file("main", &lines))]),
        }
    };

    let without = apply(&boxes(false), &operations(&layout_of_main())).unwrap();
    let with = apply(&boxes(true), &operations(&layout_of_main())).unwrap();

    let (without, with) = (
        written_board(&without, "main"),
        written_board(&with, "main"),
    );
    for id in ["a", "c"] {
        assert_eq!(
            with.rect_of(&element_id(id)),
            without.rect_of(&element_id(id)),
            "{id}"
        );
    }
    let c = with.rect_of(&element_id("c")).unwrap();
    assert_eq!(with.rect_of(&element_id("n")), Some(rect_at(c.x, c.y)));
}

#[test]
fn a_layout_that_would_carry_a_note_out_of_range_is_refused() {
    let mut before = Snapshot {
        elements: BTreeMap::new(),
        boards: BTreeMap::from([(
            board_id("main"),
            board_file(
                "main",
                &[
                    placement_line("b", -999_000, 0, ""),
                    placement_line("n", 1_998_000, 0, r#", "attached_to": "b""#),
                ],
            ),
        )]),
    };
    before.elements.insert(element_id("b"), element("b"));

    let refused = problem(&before, &layout_of_main());

    assert_eq!(
        refused,
        OperationProblem::LeavesRange {
            element: element_id("n"),
            board: board_id("main"),
        }
    );
}

#[test]
fn deleting_a_box_keeps_each_note_where_it_lies_on_every_board() {
    let mut before = with_a_note(&[]);
    let side = board_file(
        "side",
        &[
            placement_line("b", -40, 70, ""),
            placement_line("n", 1, 2, r#", "attached_to": "b""#),
        ],
    );
    before.boards.insert(side.id.clone(), side);
    let json = r#"[{"op": "delete_element", "element": {"existing": "b"}}]"#;

    let applied = apply(&before, &operations(json)).unwrap();

    for (id, expected) in [("main", rect_at(105, 206)), ("side", rect_at(-39, 72))] {
        let board = written_board(&applied, id);
        assert_eq!(board.rect_of(&element_id("n")), Some(expected), "{id}");
        assert_eq!(board.named_attachment(&element_id("n")), None, "{id}");
        assert!(!board.to_json().contains("attached_to"), "{id}");
        assert_eq!(board.placements[&element_id("n")].rect, expected, "{id}");
    }
}

/// The boxes `a` and `c` and the note `b`, which names `a` but lies beyond the
/// range from it, so its attachment does not hold.
fn with_a_note_out_of_range() -> Snapshot {
    with_files_for(vec![board_file(
        "main",
        &[
            placement_line("a", 0, 0, ""),
            placement_line("b", 2_000_000, 0, r#", "attached_to": "a""#),
            placement_line("c", 0, 0, ""),
        ],
    )])
}

#[test]
fn a_layout_leaves_a_note_out_of_range_where_it_is_and_repeats_byte_for_byte() {
    let before = with_a_note_out_of_range();
    let once = layout_of_main();
    let twice = r#"[{"op": "layout_board", "board": {"existing": "main"}}, {"op": "layout_board", "board": {"existing": "main"}}]"#;

    let first = apply(&before, &operations(&once)).unwrap();
    let second = apply(&before, &operations(twice)).unwrap();

    let (first, second) = (
        written_board(&first, "main"),
        written_board(&second, "main"),
    );
    assert_eq!(first.to_json(), second.to_json());
    let was = &before.boards[&board_id("main")];
    assert_eq!(line_of(first, "b"), line_of(was, "b"));
    assert_eq!(first.rect_of(&element_id("b")), Some(rect_at(2_000_000, 0)));
}

#[test]
fn a_placement_after_a_layout_overlaps_no_note_out_of_range_where_it_lies() {
    // `b` names the pinned `a`, which lies too far away for the attachment to
    // hold, so `b` lies at 0, 0, where the first free cell would be.
    let mut before = with_files_for(vec![board_file(
        "main",
        &[
            placement_line("a", 2_000_000, 0, r#", "pinned": true"#),
            placement_line("b", 0, 0, r#", "attached_to": "a""#),
        ],
    )]);
    before
        .elements
        .insert(element_id("loose"), element("loose"));
    let json = r#"[{"op": "layout_board", "board": {"existing": "main"}}, {"op": "place_element", "element": {"existing": "loose"}, "board": {"existing": "main"}}]"#;

    let applied = apply(&before, &operations(json)).unwrap();

    let board = written_board(&applied, "main");
    let note = board.rect_of(&element_id("b")).unwrap();
    let placed = board.rect_of(&element_id("loose")).unwrap();
    assert!(!placed.overlaps(&note), "{placed:?} overlaps {note:?}");
}
