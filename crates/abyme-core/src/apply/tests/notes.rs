//! Tests of moving and laying out boxes that carry notes, and of deleting such boxes.

#![expect(
    clippy::disallowed_fields,
    reason = "the tests check the fields as the board file stores them"
)]
use super::*;

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

#[expect(
    clippy::disallowed_methods,
    reason = "the test checks the field as written"
)]
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
