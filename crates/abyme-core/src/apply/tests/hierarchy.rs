//! Tests of boards, their owners and zones, and inner boards.

use super::*;

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
