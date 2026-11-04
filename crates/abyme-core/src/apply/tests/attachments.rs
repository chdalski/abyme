//! Tests of attaching and detaching notes, and of deleting their concerns relation.

use super::*;

/// A note with the given relations, stamped as verified.
fn stamped_note(id: &str, relations: &[(RelationKind, &str, Option<&str>)]) -> Element {
    let mut fields = element(id).into_fields();
    fields.type_ref = TypeRef::try_from(NOTE_TYPE.to_owned()).unwrap();
    fields.relations = relations
        .iter()
        .map(|(kind, to, label)| ((*kind, element_id(to)), label.map(str::to_owned)))
        .collect();
    fields.verified = Some(abyme_model::Verification {
        commit: abyme_model::CommitId::try_from("a".repeat(40)).unwrap(),
        by: abyme_model::Verifier::Person,
    });
    Element::new(element_id(id), fields).unwrap()
}

/// The board `main` with the boxes `b` at 100, 200 and `c` at -50, 0, the
/// notes `n` at 30, 40 and `m` at 0, 500, and the given lines; `n` and `m`
/// are notes, stamped, without relations.
fn notes_on_main(more: &[String]) -> Snapshot {
    let mut lines = vec![
        placement_line("b", 100, 200, ""),
        placement_line("c", -50, 0, ""),
        placement_line("m", 0, 500, ""),
        placement_line("n", 30, 40, ""),
    ];
    lines.extend_from_slice(more);
    let mut snapshot = with_files_for(vec![board_file("main", &lines)]);
    for id in ["m", "n"] {
        snapshot
            .elements
            .insert(element_id(id), stamped_note(id, &[]));
    }
    snapshot
}

fn attach(note: &str, to: &str, rest: &str) -> String {
    format!(
        r#"{{"op": "attach_note", "note": {{"existing": "{note}"}}, "to": {{"existing": "{to}"}}, "board": {{"existing": "main"}}{rest}}}"#
    )
}

fn detach(note: &str) -> String {
    format!(
        r#"{{"op": "detach_note", "note": {{"existing": "{note}"}}, "board": {{"existing": "main"}}}}"#
    )
}

#[test]
fn attaching_hangs_the_note_where_it_lies_and_adds_concerns_without_the_stamp() {
    let before = notes_on_main(&[]);

    let applied = apply(&before, &operations(&format!("[{}]", attach("n", "b", "")))).unwrap();

    let board = written_board(&applied, "main");
    assert_eq!(board.rect_of(&element_id("n")), Some(rect_at(30, 40)));
    assert_eq!(board.attachment(&element_id("n")), Some(&element_id("b")));
    assert!(
        line_of(board, "n").contains(r#""x": -70, "y": -160"#),
        "{}",
        line_of(board, "n")
    );
    let note = written_element(&applied, "n").unwrap();
    assert_eq!(relations_of(note), [(RelationKind::Concerns, "b", None)]);
    assert_eq!(note.clone().into_fields().verified, None);
    assert_eq!(
        line_of(board, "b"),
        line_of(&before.boards[&board_id("main")], "b")
    );
}

#[test]
fn attaching_with_the_relation_in_place_keeps_its_label_and_the_stamp_and_writes_no_element() {
    let mut before = notes_on_main(&[]);
    before.elements.insert(
        element_id("n"),
        stamped_note("n", &[(RelationKind::Concerns, "b", Some("why"))]),
    );

    let applied = apply(&before, &operations(&format!("[{}]", attach("n", "b", "")))).unwrap();

    assert!(
        matches!(applied.writes.as_slice(), [Write::Board(_)]),
        "{:?}",
        applied.writes
    );
    assert_eq!(
        written_board(&applied, "main").attachment(&element_id("n")),
        Some(&element_id("b"))
    );
}

#[test]
fn attaching_to_another_box_moves_the_attachment_and_keeps_both_relations() {
    let json = format!(
        "[{}, {}, {}]",
        attach("n", "b", ""),
        attach("n", "c", ""),
        attach("n", "c", "")
    );

    let applied = apply(&notes_on_main(&[]), &operations(&json)).unwrap();

    let board = written_board(&applied, "main");
    assert_eq!(board.rect_of(&element_id("n")), Some(rect_at(30, 40)));
    assert_eq!(board.attachment(&element_id("n")), Some(&element_id("c")));
    assert_eq!(
        relations_of(written_element(&applied, "n").unwrap()),
        [
            (RelationKind::Concerns, "b", None),
            (RelationKind::Concerns, "c", None)
        ]
    );
}

#[test]
fn a_note_is_created_placed_and_attached_in_one_request() {
    let json = r#"[
        {"op": "create_element", "key": "q", "type_ref": "note", "name": "Why"},
        {"op": "place_element", "element": {"new": "q"}, "board": {"existing": "main"}},
        {"op": "attach_note", "note": {"new": "q"}, "to": {"existing": "b"}, "board": {"existing": "main"}}
    ]"#;

    let applied = apply(&notes_on_main(&[]), &operations(json)).unwrap();

    let board = written_board(&applied, "main");
    assert_eq!(board.attachment(&element_id("why")), Some(&element_id("b")));
    assert_eq!(
        relations_of(written_element(&applied, "why").unwrap()),
        [(RelationKind::Concerns, "b", None)]
    );
}

#[rstest]
#[case::a_box_as_the_note(attach("c", "b", ""), OperationProblem::NotANote(element_id("c")))]
#[case::the_note_itself(
    attach("n", "n", ""),
    OperationProblem::AttachToItself(element_id("n"))
)]
#[case::a_note_as_the_box(attach("n", "m", ""), OperationProblem::AttachToNote(element_id("m")))]
#[case::an_unknown_box(
    attach("n", "nowhere", ""),
    OperationProblem::UnknownElement(element_id("nowhere"))
)]
#[case::a_note_off_the_board(attach("loose-note", "b", ""), OperationProblem::NotOnBoard {
    element: element_id("loose-note"),
    board: board_id("main"),
})]
#[case::a_box_off_the_board(attach("n", "loose", ""), OperationProblem::NotOnBoard {
    element: element_id("loose"),
    board: board_id("main"),
})]
fn an_attachment_of_the_wrong_elements_is_refused(
    #[case] operation: String,
    #[case] expected: OperationProblem,
) {
    let mut before = notes_on_main(&[]);
    before
        .elements
        .insert(element_id("loose"), element("loose"));
    before
        .elements
        .insert(element_id("loose-note"), stamped_note("loose-note", &[]));

    assert_eq!(problem(&before, &format!("[{operation}]")), expected);
}

#[test]
fn an_attachment_of_a_new_element_that_is_no_note_is_refused() {
    let json = r#"[
        {"op": "create_element", "key": "q", "type_ref": "t", "name": "Box"},
        {"op": "place_element", "element": {"new": "q"}, "board": {"existing": "main"}},
        {"op": "attach_note", "note": {"new": "q"}, "to": {"existing": "b"}, "board": {"existing": "main"}}
    ]"#;

    assert_eq!(
        problem(&notes_on_main(&[]), json),
        OperationProblem::NotANote(element_id("box"))
    );
}

#[rstest]
#[case::something_hangs_on_the_note(
    vec![placement_line("x", 0, 0, r#", "attached_to": "n""#)],
    "b",
    OperationProblem::CannotAttach { note: element_id("n"), element: element_id("b"), board: board_id("main") }
)]
#[case::the_box_hangs_on_another(
    vec![placement_line("d", 900, 900, ""), placement_line("e", 0, 0, r#", "attached_to": "d""#)],
    "e",
    OperationProblem::CannotAttach { note: element_id("n"), element: element_id("e"), board: board_id("main") }
)]
fn an_attachment_that_would_make_a_chain_is_refused(
    #[case] more: Vec<String>,
    #[case] to: &str,
    #[case] expected: OperationProblem,
) {
    let before = {
        let mut snapshot = notes_on_main(&more);
        for id in ["d", "e", "x"] {
            snapshot.elements.insert(element_id(id), element(id));
        }
        snapshot
    };

    assert_eq!(
        problem(&before, &format!("[{}]", attach("n", to, ""))),
        expected
    );
}

#[test]
fn an_attachment_of_a_note_beyond_the_range_is_refused() {
    let before = notes_on_main(&[placement_line("far", 2_000_000, 0, "")]);
    let mut before = before;
    before
        .elements
        .insert(element_id("far"), stamped_note("far", &[]));

    assert_eq!(
        problem(&before, &format!("[{}]", attach("far", "b", ""))),
        OperationProblem::CannotAttach {
            note: element_id("far"),
            element: element_id("b"),
            board: board_id("main"),
        }
    );
}

#[test]
fn an_attachment_whose_offset_does_not_fit_is_refused() {
    let mut before = with_files_for(vec![board_file(
        "main",
        &[
            placement_line("b", i32::MIN, 0, ""),
            placement_line("n", i32::MAX, 0, ""),
        ],
    )]);
    before
        .elements
        .insert(element_id("n"), stamped_note("n", &[]));

    assert_eq!(
        problem(&before, &format!("[{}]", attach("n", "b", ""))),
        OperationProblem::PositionNotStored {
            element: element_id("n"),
            board: board_id("main"),
        }
    );
}

#[test]
fn a_pinned_note_hangs_on_a_box_only_with_pinned_true_and_keeps_every_pin() {
    let pinned_note = notes_on_main(&[]);
    let mut pinned_note = pinned_note;
    for id in ["n", "b"] {
        placements_on(&mut pinned_note, "main")
            .get_mut(&element_id(id))
            .unwrap()
            .pinned = true;
    }

    let refused = problem(&pinned_note, &format!("[{}]", attach("n", "b", "")));
    let applied = apply(
        &pinned_note,
        &operations(&format!("[{}]", attach("n", "b", r#", "pinned": true"#))),
    )
    .unwrap();
    let unpinned = apply(
        &notes_on_main(&[]),
        &operations(&format!("[{}]", attach("n", "b", r#", "pinned": true"#))),
    )
    .unwrap();

    assert_eq!(
        refused,
        OperationProblem::PinnedAttach {
            note: element_id("n"),
            board: board_id("main"),
        }
    );
    assert_eq!(
        refused.to_string(),
        "note n is pinned on board main: a person placed it there. Attach it only after the \
         person has said so, with pinned: true"
    );
    let board = written_board(&applied, "main");
    assert!(board.placements[&element_id("n")].pinned);
    assert!(board.placements[&element_id("b")].pinned);
    let board = written_board(&unpinned, "main");
    assert!(!board.placements[&element_id("n")].pinned);
    assert!(!board.placements[&element_id("b")].pinned);
}

#[test]
fn a_move_that_pins_the_note_makes_a_later_attachment_of_the_request_need_pinned_true() {
    let json = format!(
        r#"[{{"op": "move_element", "element": {{"existing": "n"}}, "board": {{"existing": "main"}}, "x": 30, "y": 40, "pinned": true}}, {}]"#,
        attach("n", "b", "")
    );

    assert_eq!(
        problem(&notes_on_main(&[]), &json),
        OperationProblem::PinnedAttach {
            note: element_id("n"),
            board: board_id("main"),
        }
    );
}

#[test]
fn detaching_keeps_where_the_note_lies_and_the_relation_and_writes_no_element() {
    let mut before = notes_on_main(&[]);
    before.elements.insert(
        element_id("n"),
        stamped_note("n", &[(RelationKind::Concerns, "b", None)]),
    );
    placements_on(&mut before, "main")
        .get_mut(&element_id("n"))
        .unwrap()
        .pinned = true;
    let json = format!(
        "[{}, {}, {}]",
        attach("n", "b", r#", "pinned": true"#),
        a_move("b", 0, 0),
        detach("n")
    );

    let applied = apply(&before, &operations(&json)).unwrap();

    assert!(
        matches!(applied.writes.as_slice(), [Write::Board(_)]),
        "{:?}",
        applied.writes
    );
    let board = written_board(&applied, "main");
    assert_eq!(board.rect_of(&element_id("n")), Some(rect_at(-70, -160)));
    assert_eq!(board.attachment(&element_id("n")), None);
    assert!(!line_of(board, "n").contains("attached_to"));
    assert!(board.placements[&element_id("n")].pinned);
}

#[test]
fn detaching_clears_an_attachment_that_does_not_hold_where_the_note_lies() {
    let before = notes_on_main(&[]);
    let mut before = before;
    let main = before.boards.get_mut(&board_id("main")).unwrap();
    *main = board_file(
        "main",
        &[
            placement_line("b", 100, 200, ""),
            placement_line("c", -50, 0, ""),
            placement_line("m", 0, 500, ""),
            placement_line("n", 30, 40, r#", "attached_to": "gone""#),
        ],
    );

    let applied = apply(&before, &operations(&format!("[{}]", detach("n")))).unwrap();

    let board = written_board(&applied, "main");
    assert_eq!(board.rect_of(&element_id("n")), Some(rect_at(30, 40)));
    assert!(!line_of(board, "n").contains("attached_to"));
}

#[rstest]
#[case::a_note_that_hangs_on_nothing(detach("n"), OperationProblem::NotAttached {
    note: element_id("n"),
    board: board_id("main"),
})]
#[case::a_box(detach("b"), OperationProblem::NotANote(element_id("b")))]
#[case::a_note_off_the_board(detach("loose-note"), OperationProblem::NotOnBoard {
    element: element_id("loose-note"),
    board: board_id("main"),
})]
fn a_detachment_of_the_wrong_element_is_refused(
    #[case] operation: String,
    #[case] expected: OperationProblem,
) {
    let mut before = notes_on_main(&[]);
    before
        .elements
        .insert(element_id("loose-note"), stamped_note("loose-note", &[]));

    assert_eq!(problem(&before, &format!("[{operation}]")), expected);
}

fn delete_concerns(from: &str, to: &str, kind: &str) -> String {
    format!(
        r#"{{"op": "delete_relation", "from": {{"existing": "{from}"}}, "kind": "{kind}", "to": {{"existing": "{to}"}}}}"#
    )
}

#[test]
fn deleting_concerns_detaches_the_note_only_on_the_boards_that_hang_it_there() {
    let mut before = notes_on_main(&[placement_line("x", 5, 5, r#", "attached_to": "b""#)]);
    before.elements.insert(
        element_id("x"),
        stamped_note("x", &[(RelationKind::Concerns, "b", None)]),
    );
    before.elements.insert(
        element_id("n"),
        stamped_note("n", &[(RelationKind::Concerns, "b", None)]),
    );
    let main = before.boards.get_mut(&board_id("main")).unwrap();
    main.attach(&element_id("n"), &element_id("b")).unwrap();
    for (id, rest) in [("hung", r#", "attached_to": "b""#), ("placed", "")] {
        let side = board_file(
            id,
            &[
                placement_line("b", 0, 0, ""),
                placement_line("n", 7, 7, rest),
            ],
        );
        before.boards.insert(side.id.clone(), side);
    }

    let applied = apply(
        &before,
        &operations(&format!("[{}]", delete_concerns("n", "b", "concerns"))),
    )
    .unwrap();

    let boards: BTreeSet<&str> = applied
        .writes
        .iter()
        .filter_map(|write| match write {
            Write::Board(board) => Some(board.id.as_str()),
            Write::Element(_) | Write::RemoveElement(_) => None,
        })
        .collect();
    assert_eq!(boards, BTreeSet::from(["hung", "main"]));
    let main = written_board(&applied, "main");
    assert_eq!(main.rect_of(&element_id("n")), Some(rect_at(30, 40)));
    assert_eq!(main.attachment(&element_id("n")), None);
    assert_eq!(main.attachment(&element_id("x")), Some(&element_id("b")));
    let hung = written_board(&applied, "hung");
    assert_eq!(hung.rect_of(&element_id("n")), Some(rect_at(7, 7)));
    assert!(!line_of(hung, "n").contains("attached_to"));
}

#[test]
fn deleting_concerns_also_clears_an_attachment_that_does_not_hold() {
    let mut before = notes_on_main(&[]);
    before.elements.insert(
        element_id("n"),
        stamped_note("n", &[(RelationKind::Concerns, "gone", None)]),
    );
    let side = board_file(
        "side",
        &[placement_line("n", 7, 7, r#", "attached_to": "gone""#)],
    );
    before.boards.insert(side.id.clone(), side);

    let applied = apply(
        &before,
        &operations(&format!("[{}]", delete_concerns("n", "gone", "concerns"))),
    )
    .unwrap();

    assert!(!line_of(written_board(&applied, "side"), "n").contains("attached_to"));
}

#[test]
fn deleting_a_relation_of_another_kind_writes_no_board() {
    let mut before = notes_on_main(&[]);
    before.elements.insert(
        element_id("n"),
        stamped_note(
            "n",
            &[
                (RelationKind::SeeAlso, "b", None),
                (RelationKind::Concerns, "b", None),
            ],
        ),
    );
    before
        .boards
        .get_mut(&board_id("main"))
        .unwrap()
        .attach(&element_id("n"), &element_id("b"))
        .unwrap();

    let applied = apply(
        &before,
        &operations(&format!("[{}]", delete_concerns("n", "b", "see_also"))),
    )
    .unwrap();

    assert!(
        matches!(applied.writes.as_slice(), [Write::Element(_)]),
        "{:?}",
        applied.writes
    );
}

#[test]
fn the_texts_of_the_new_refusals_name_only_ids() {
    let note = || element_id("n");
    let board = || board_id("main");
    let texts = [
        OperationProblem::NotANote(note()).to_string(),
        OperationProblem::AttachToItself(note()).to_string(),
        OperationProblem::AttachToNote(element_id("m")).to_string(),
        OperationProblem::CannotAttach {
            note: note(),
            element: element_id("b"),
            board: board(),
        }
        .to_string(),
        OperationProblem::NotAttached {
            note: note(),
            board: board(),
        }
        .to_string(),
    ];

    assert_eq!(
        texts,
        [
            "element n is no note",
            "element n cannot be attached to itself",
            "element m is a note; a note hangs only on an element that is no note",
            "note n cannot hang on b on board main: b hangs on another element, something hangs \
             on n, or n would lie outside -1000000 to 1000000",
            "note n hangs on nothing on board main",
        ]
    );
}
