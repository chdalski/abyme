//! Tests of a request as a whole: what it writes, in which order, its size, and its validation.

use super::*;

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
        OperationProblem::InvalidElement(ElementProblem::InvalidAnchor { .. })
    ));
}
