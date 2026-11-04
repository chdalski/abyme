//! Tests of moving elements and of their pins.

#![expect(
    clippy::disallowed_fields,
    reason = "the tests check the fields as the board file stores them"
)]
use super::*;

/// `snapshot()` with `orders-db` sized 300 × 50 and a second board `side`
/// on which `loose` lies.
fn snapshot_with_two_boards() -> Snapshot {
    let mut snapshot = snapshot();
    placements_on(&mut snapshot, "main").insert(element_id("orders-db"), sized(0, 0, 300, 50));
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
    placements_on(&mut before, "main")
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
