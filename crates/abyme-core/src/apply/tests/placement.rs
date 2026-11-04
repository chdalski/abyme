//! Tests of placing elements in free space.

#![expect(
    clippy::disallowed_fields,
    reason = "the tests check the fields as the board file stores them"
)]
use super::*;

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
    placements_on(&mut walled, "main").insert(element_id("wall"), wall);
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
    placements_on(&mut large, "main")
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

#[test]
fn reports_a_full_board() {
    let mut full = snapshot();
    let huge = Rect {
        x: i32::MIN,
        y: i32::MIN,
        width: NonZeroU32::MAX,
        height: NonZeroU32::MAX,
    };
    placements_on(&mut full, "main").insert(
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

#[rstest]
#[case::move_(r#"{"op": "move_element", "element": {"existing": "tall"}, "board": {"existing": "main"}, "x": 5, "y": 6}"#)]
#[case::update(r#"{"op": "update_element", "element": {"existing": "tall"}, "summary": "short"}"#)]
fn a_move_or_an_update_leaves_a_placement_its_size(#[case] operation: &str) {
    let mut before = snapshot();
    let tall = ElementFixture::new("tall")
        .type_ref("c4/container")
        .name("Tall")
        .build();
    before.elements.insert(element_id("tall"), tall);
    placements_on(&mut before, "main").insert(element_id("tall"), sized(1000, 0, 240, 170));

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
