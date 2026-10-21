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
