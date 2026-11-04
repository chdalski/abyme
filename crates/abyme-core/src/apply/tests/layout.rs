//! Tests of laying out a board.

#![expect(
    clippy::disallowed_fields,
    reason = "the tests check the fields as the board file stores them"
)]
use super::*;

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
    placements_on(&mut before, "main")
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
    placements_on(&mut before, "main")
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
    placements_on(&mut before, "main").clear();
    for index in 0..100 {
        let id = element_id(&format!("e{index}"));
        before.elements.insert(
            id.clone(),
            ElementFixture::new(id.as_str())
                .name("n")
                .summary(Some(&text))
                .build(),
        );
        placements_on(&mut before, "main").insert(id, placed(index * 220, 0));
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
