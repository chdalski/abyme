//! Tests of which operations the canvas may send.

use super::*;
use std::collections::BTreeMap;
use std::num::NonZeroU32;

use abyme_core::LocalKey;
use abyme_model::{ElementFixture, Mode, Placement, Rect, StatementKind, Zone};
use serde_json::{Value, json};

fn operation(value: Value) -> Operation {
    serde_json::from_value(value).unwrap()
}

fn move_existing() -> Operation {
    operation(json!({
        "op": "move_element",
        "element": {"existing": "payments"},
        "board": {"existing": "context"},
        "x": 10,
        "y": 20,
        "pinned": true
    }))
}

#[test]
fn a_move_of_an_existing_element_on_an_existing_board_is_admitted() {
    assert_eq!(admit(&[move_existing()]), Ok(()));
}

#[test]
fn a_release_of_an_existing_element_on_an_existing_board_is_admitted() {
    let release = operation(json!({
        "op": "move_element",
        "element": {"existing": "payments"},
        "board": {"existing": "context"},
        "x": 1,
        "y": 2,
        "pinned": false
    }));

    assert_eq!(admit(&[release]), Ok(()));
}

#[test]
fn a_move_that_does_not_say_pinned_is_refused_with_its_position() {
    let without_pinned = operation(json!({
        "op": "move_element",
        "element": {"existing": "payments"},
        "board": {"existing": "context"},
        "x": 1,
        "y": 2
    }));

    assert_eq!(
        admit(&[move_existing(), without_pinned]),
        Err(Refused::Operation {
            position: 2,
            kind: "move_element without pinned",
        })
    );
}

#[test]
fn pinned_does_not_admit_a_move_with_a_new_element_or_board() {
    for with_a_new_ref in [
        json!({"op": "move_element", "element": {"new": "e"}, "board": {"existing": "context"}, "x": 1, "y": 2, "pinned": true}),
        json!({"op": "move_element", "element": {"existing": "payments"}, "board": {"new": "b"}, "x": 1, "y": 2, "pinned": true}),
        json!({"op": "move_element", "element": {"new": "e"}, "board": {"existing": "context"}, "x": 1, "y": 2, "pinned": false}),
        json!({"op": "move_element", "element": {"existing": "payments"}, "board": {"new": "b"}, "x": 1, "y": 2, "pinned": false}),
    ] {
        assert_eq!(
            admit(&[operation(with_a_new_ref)]),
            Err(Refused::Operation {
                position: 1,
                kind: "move_element with a new element or board",
            })
        );
    }
}

#[test]
fn an_empty_request_is_admitted() {
    assert_eq!(admit(&[]), Ok(()));
}

#[test]
fn every_other_operation_is_refused_by_its_position_and_kind() {
    let refused = [
        (
            json!({"op": "create_element", "key": "secret-key", "type_ref": "c4/container", "name": "SECRET-NAME", "body": "SECRET-BODY"}),
            "create_element",
        ),
        (
            json!({"op": "create_board", "key": "secret-key", "title": "SECRET-TITLE"}),
            "create_board",
        ),
        (
            json!({"op": "update_board", "board": {"existing": "context"}, "title": "SECRET-TITLE", "summary": "SECRET-SUMMARY"}),
            "update_board",
        ),
        (
            json!({"op": "update_board", "board": {"new": "secret-key"}, "owner": {"existing": "secret-id"}}),
            "update_board",
        ),
        (
            json!({"op": "place_element", "element": {"existing": "secret-id"}, "board": {"existing": "context"}}),
            "place_element",
        ),
        (
            json!({"op": "move_element", "element": {"new": "secret-key"}, "board": {"existing": "context"}, "x": 1, "y": 2}),
            "move_element with a new element or board",
        ),
        (
            json!({"op": "move_element", "element": {"existing": "secret-id"}, "board": {"new": "secret-key"}, "x": 1, "y": 2}),
            "move_element with a new element or board",
        ),
        (
            json!({"op": "move_element", "element": {"new": "secret-key"}, "board": {"new": "other-key"}, "x": 1, "y": 2}),
            "move_element with a new element or board",
        ),
        (
            json!({"op": "layout_board", "board": {"existing": "secret-id"}}),
            "layout_board",
        ),
        (
            json!({"op": "update_element", "element": {"existing": "secret-id"}, "name": "SECRET-NAME"}),
            "update_element",
        ),
        (
            json!({"op": "update_element", "element": {"existing": "secret-id"}, "statement": "idea"}),
            "update_element",
        ),
        (
            json!({"op": "update_element", "element": {"new": "secret-key"}, "body": "SECRET-BODY"}),
            "update_element",
        ),
        (
            json!({"op": "delete_element", "element": {"new": "secret-key"}}),
            "delete_element",
        ),
        (
            json!({"op": "create_relation", "from": {"existing": "secret-id"}, "kind": "uses", "to": {"existing": "payments"}, "label": "SECRET-LABEL"}),
            "create_relation",
        ),
        (
            json!({"op": "delete_relation", "from": {"existing": "secret-id"}, "kind": "uses", "to": {"existing": "payments"}}),
            "delete_relation",
        ),
    ];

    assert_refused_by_kind(refused);
}

/// Checks that each operation of `refused`, after an admitted move, is refused
/// by its position and kind, naming nothing the operation holds.
fn assert_refused_by_kind<const N: usize>(refused: [(Value, &'static str); N]) {
    for (refused, kind) in refused {
        let refusal = admit(&[move_existing(), operation(refused.clone())]).unwrap_err();

        assert_eq!(
            refusal,
            Refused::Operation { position: 2, kind },
            "{refused}"
        );
        let message = refusal.to_string();
        assert_eq!(
            message,
            format!("operation 2: the canvas may not send {kind}")
        );
        assert!(
            !message.contains("SECRET") && !message.contains("secret"),
            "{message}"
        );
        assert!(!message.chars().any(char::is_control), "{message}");
    }
}

#[test]
fn an_attachment_or_a_detachment_of_another_form_is_refused_by_its_position_and_kind() {
    assert_refused_by_kind([
        (
            json!({"op": "attach_note", "note": {"existing": "secret-id"}, "to": {"existing": "payments"}, "board": {"existing": "context"}}),
            "attach_note without pinned",
        ),
        (
            json!({"op": "attach_note", "note": {"new": "secret-key"}, "to": {"existing": "payments"}, "board": {"existing": "mirrored"}, "pinned": true}),
            "attach_note with a new element or board",
        ),
        (
            json!({"op": "attach_note", "note": {"existing": "secret-id"}, "to": {"new": "secret-key"}, "board": {"existing": "context"}, "pinned": true}),
            "attach_note with a new element or board",
        ),
        (
            json!({"op": "attach_note", "note": {"existing": "secret-id"}, "to": {"existing": "payments"}, "board": {"new": "secret-key"}, "pinned": true}),
            "attach_note with a new element or board",
        ),
        (
            json!({"op": "detach_note", "note": {"new": "secret-key"}, "board": {"existing": "context"}}),
            "detach_note with a new element or board",
        ),
        (
            json!({"op": "detach_note", "note": {"existing": "secret-id"}, "board": {"new": "secret-key"}}),
            "detach_note with a new element or board",
        ),
    ]);
}

#[test]
fn the_first_refused_operation_is_named() {
    let create = Operation::CreateBoard {
        key: LocalKey::try_from("b".to_owned()).unwrap(),
        title: "Context".to_owned(),
        owner: None,
        summary: None,
        zone: None,
    };

    assert_eq!(
        admit(&[create.clone(), move_existing(), create]),
        Err(Refused::Operation {
            position: 1,
            kind: "create_board"
        })
    );
}

#[test]
fn a_request_with_more_operations_than_the_core_takes_is_refused() {
    let operations = vec![move_existing(); MAX_OPERATIONS + 1];

    assert_eq!(
        admit(&operations),
        Err(Refused::TooMany {
            count: MAX_OPERATIONS + 1
        })
    );
}

#[test]
fn a_request_with_as_many_operations_as_the_core_takes_is_admitted() {
    assert_eq!(admit(&vec![move_existing(); MAX_OPERATIONS]), Ok(()));
}

/// A board `id` that is a zone of `mode`, if one is given.
fn board(id: &str, mode: Option<Mode>) -> Board {
    Board {
        id: id.parse().unwrap(),
        title: id.to_owned(),
        owner: None,
        summary: None,
        zone: mode.map(|mode| Zone {
            mode,
            statement: StatementKind::Current,
        }),
        placements: BTreeMap::new(),
    }
}

/// A move of `payments` on the existing board `board`.
fn a_move_on(board: &str) -> Operation {
    operation(json!({
        "op": "move_element",
        "element": {"existing": "payments"},
        "board": {"existing": board},
        "x": 10,
        "y": 20,
        "pinned": true
    }))
}

/// The boards `free`, `plain`, and `mirrored`, each a zone of its name, or
/// none for `plain`.
fn snapshot() -> Snapshot {
    Snapshot {
        elements: BTreeMap::new(),
        boards: [
            board("free", Some(Mode::Free)),
            board("plain", None),
            board("mirrored", Some(Mode::Mirrored)),
        ]
        .into_iter()
        .map(|board| (board.id.clone(), board))
        .collect(),
    }
}

#[test]
fn lets_a_move_and_pin_of_any_element_pass_on_every_board() {
    for board in ["free", "plain", "mirrored", "nowhere"] {
        assert_eq!(
            check_board_files(&snapshot(), &[a_move_on(board)]),
            Ok(()),
            "{board}"
        );
    }
    assert_eq!(
        check_board_files(&snapshot(), &["free", "plain", "mirrored"].map(a_move_on)),
        Ok(())
    );
}

/// A release of the pin of `payments` on the existing board `board`, at 10, 20.
fn a_release_on(board: &str) -> Operation {
    operation(json!({
        "op": "move_element",
        "element": {"existing": "payments"},
        "board": {"existing": board},
        "x": 10,
        "y": 20,
        "pinned": false
    }))
}

/// `snapshot` with `payments` on the board `plain` at `x` and `y`.
fn snapshot_with_payments_at(x: i32, y: i32) -> Snapshot {
    let mut snapshot = snapshot();
    snapshot
        .boards
        .get_mut(&"plain".parse::<BoardId>().unwrap())
        .unwrap()
        .placements
        .insert(
            "payments".parse().unwrap(),
            Placement {
                rect: Rect {
                    x,
                    y,
                    width: NonZeroU32::new(180).unwrap(),
                    height: NonZeroU32::new(90).unwrap(),
                },
                pinned: true,
                attached_to: None,
            },
        );
    snapshot
}

#[test]
fn lets_a_release_pass_only_at_the_position_in_the_file_of_its_board() {
    assert_eq!(
        check_board_files(&snapshot_with_payments_at(10, 20), &[a_release_on("plain")]),
        Ok(())
    );
    for (x, y) in [(11, 20), (10, 21)] {
        assert_eq!(
            check_board_files(&snapshot_with_payments_at(x, y), &[a_release_on("plain")]),
            Err(Refused::ReleaseElsewhere {
                position: 1,
                board: "plain".parse().unwrap(),
            }),
            "{x}, {y}"
        );
    }
}

#[test]
fn refuses_a_release_of_an_element_its_board_does_not_place() {
    for board in ["free", "plain", "mirrored"] {
        assert_eq!(
            check_board_files(&snapshot(), &[a_release_on(board)]),
            Err(Refused::ReleaseElsewhere {
                position: 1,
                board: board.parse().unwrap(),
            }),
            "{board}"
        );
    }
}

#[test]
fn lets_a_release_on_a_board_it_does_not_have_pass_to_the_core() {
    assert_eq!(
        check_board_files(&snapshot(), &[a_release_on("nowhere")]),
        Ok(())
    );
}

#[test]
fn names_only_the_position_and_the_board_of_a_refused_release() {
    let refused = Refused::ReleaseElsewhere {
        position: 2,
        board: "plain".parse().unwrap(),
    };

    assert_eq!(
        refused.to_string(),
        "operation 2: the canvas releases a pin on board plain only at the position in the board \
         file"
    );
}

/// `snapshot` with the note `payments`, which no board places.
fn snapshot_with_a_note() -> Snapshot {
    let id: ElementId = "payments".parse().unwrap();
    let note = ElementFixture::new("payments")
        .type_ref("note")
        .name("Ask")
        .build();
    let mut snapshot = snapshot();
    snapshot.elements.insert(id, note);
    snapshot
}

/// The three operations that create a note on the board `board`.
fn a_new_note_on(board: &str) -> Vec<Operation> {
    [
        json!({"op": "create_element", "key": "note", "type_ref": "note", "name": "Ask",
               "body": "", "statement": "idea"}),
        json!({"op": "place_element", "element": {"new": "note"}, "board": {"existing": board}}),
        json!({"op": "move_element", "element": {"new": "note"}, "board": {"existing": board},
               "x": 1, "y": 2, "pinned": true}),
    ]
    .into_iter()
    .map(operation)
    .collect()
}

#[test]
fn admits_and_lets_a_new_note_pass_as_a_whole_request_on_any_board() {
    for board in ["free", "plain", "mirrored", "nowhere"] {
        let request = a_new_note_on(board);

        assert_eq!(admit(&request), Ok(()), "{board}");
        assert_eq!(check_board_files(&snapshot(), &request), Ok(()), "{board}");
    }
}

#[test]
fn a_new_note_with_anything_beside_it_is_no_creation_of_a_note() {
    let beside = a_move_on("mirrored");
    let new_note = a_new_note_on("mirrored");
    let before: Vec<Operation> = std::iter::once(beside.clone())
        .chain(new_note.iter().cloned())
        .collect();
    let after: Vec<Operation> = new_note.iter().cloned().chain([beside]).collect();

    for request in [before, after] {
        assert!(admit(&request).is_err());
    }
}

/// `snapshot_with_a_note` with the box `checkout` at 100, 200 on the boards
/// `plain` and `mirrored`, and `payments` hanging on it at the offset `dx`,
/// `dy`, pinned.
fn snapshot_with_an_attached_note(dx: i32, dy: i32) -> Snapshot {
    let mut snapshot = snapshot_with_a_note();
    for board in ["plain", "mirrored"] {
        let id: BoardId = board.parse().unwrap();
        let mode = snapshot.boards[&id].zone.map(|zone| zone.mode.as_str());
        let zone = mode.map_or(String::new(), |mode| {
            format!(r#""zone": {{"mode": "{mode}", "statement": "current"}}, "#)
        });
        let text = format!(
            r#"{{"format": 1, "id": "{board}", "title": "{board}", {zone}"placements": [
                {{"element": "checkout", "x": 100, "y": 200, "width": 180, "height": 90}},
                {{"element": "payments", "x": {dx}, "y": {dy}, "width": 180, "height": 90, "pinned": true, "attached_to": "checkout"}}
            ]}}"#
        );
        snapshot
            .boards
            .insert(id.clone(), Board::from_json(&id, &text).unwrap());
    }
    snapshot
}

#[test]
fn lets_a_release_of_an_attached_note_pass_only_at_its_position_on_the_board() {
    // The note lies at 10, 20 on the board: 100 - 90 and 200 - 180.
    assert_eq!(
        check_board_files(
            &snapshot_with_an_attached_note(-90, -180),
            &[a_release_on("plain")]
        ),
        Ok(())
    );
    // Stored at 10, 20, the note lies at 110, 220.
    assert_eq!(
        check_board_files(
            &snapshot_with_an_attached_note(10, 20),
            &[a_release_on("plain")]
        ),
        Err(Refused::ReleaseElsewhere {
            position: 1,
            board: "plain".parse().unwrap(),
        })
    );
}

#[test]
fn lets_a_move_of_an_attached_note_and_of_its_box_pass_on_a_mirrored_board() {
    let a_move = |element: &str| {
        operation(json!({
            "op": "move_element",
            "element": {"existing": element},
            "board": {"existing": "mirrored"},
            "x": 10,
            "y": 20,
            "pinned": true
        }))
    };
    let snapshot = snapshot_with_an_attached_note(5, 6);

    assert_eq!(check_board_files(&snapshot, &[a_move("payments")]), Ok(()));
    assert_eq!(check_board_files(&snapshot, &[a_move("checkout")]), Ok(()));
}

fn an_attachment(note: &str, to: &str, board: &str) -> Operation {
    operation(json!({
        "op": "attach_note",
        "note": {"existing": note},
        "to": {"existing": to},
        "board": {"existing": board},
        "pinned": true
    }))
}

fn a_detachment(note: &str, board: &str) -> Operation {
    operation(json!({
        "op": "detach_note",
        "note": {"existing": note},
        "board": {"existing": board}
    }))
}

fn a_note_move(note: &str, board: &str) -> Operation {
    operation(json!({
        "op": "move_element",
        "element": {"existing": note},
        "board": {"existing": board},
        "x": 10,
        "y": 20,
        "pinned": true
    }))
}

#[test]
fn admits_an_attachment_with_pinned_and_a_detachment_of_existing_elements() {
    assert_eq!(
        admit(&[
            move_existing(),
            an_attachment("payments", "checkout", "context")
        ]),
        Ok(())
    );
    assert_eq!(admit(&[a_detachment("payments", "context")]), Ok(()));
}

#[test]
fn a_request_that_creates_a_note_and_attaches_another_is_no_creation_of_a_note() {
    let mut operations = a_new_note_on("free");
    operations.push(an_attachment("payments", "checkout", "free"));

    assert_eq!(
        admit(&operations),
        Err(Refused::Operation {
            position: 1,
            kind: "create_element"
        })
    );
}

/// The attachment of the note the request `a_new_note_on` creates to `to` on
/// `board`, given as JSON so that each test can change one field.
fn the_new_note_hung(to: Value, board: Value, pinned: Option<bool>) -> Operation {
    let mut value = json!({"op": "attach_note", "note": {"new": "note"}});
    value["to"] = to;
    value["board"] = board;
    if let Some(pinned) = pinned {
        value["pinned"] = json!(pinned);
    }
    operation(value)
}

/// `a_new_note_on("free")` hung on `checkout` on that board.
fn a_new_note_hung_on_a_box() -> Vec<Operation> {
    let mut operations = a_new_note_on("free");
    operations.push(the_new_note_hung(
        json!({"existing": "checkout"}),
        json!({"existing": "free"}),
        Some(true),
    ));
    operations
}

#[test]
fn admits_a_new_note_hung_on_an_element_of_its_board() {
    assert_eq!(admit(&a_new_note_hung_on_a_box()), Ok(()));
}

#[test]
fn refuses_every_other_shape_of_a_new_note_with_an_attachment() {
    let with_last = |last: Operation| {
        let mut operations = a_new_note_on("free");
        operations.push(last);
        operations
    };
    let free = || json!({"existing": "free"});
    let checkout = || json!({"existing": "checkout"});
    let mut attached_before_the_move = a_new_note_on("free");
    attached_before_the_move.insert(2, the_new_note_hung(checkout(), free(), Some(true)));
    let mut twice = a_new_note_hung_on_a_box();
    twice.push(the_new_note_hung(checkout(), free(), Some(true)));
    let mut with_a_fifth = a_new_note_hung_on_a_box();
    with_a_fifth.push(a_move_on("free"));
    let mut placed_elsewhere = a_new_note_hung_on_a_box();
    placed_elsewhere[1] = operation(json!({"op": "place_element",
        "element": {"new": "note"}, "board": {"existing": "other"}}));
    placed_elsewhere[2] = operation(json!({"op": "move_element",
        "element": {"new": "note"}, "board": {"existing": "other"}, "x": 1, "y": 2,
        "pinned": true}));

    for (shape, request) in [
        (
            "an existing note",
            with_last(operation(
                json!({"op": "attach_note", "note": {"existing": "note"},
                "to": checkout(), "board": free(), "pinned": true}),
            )),
        ),
        (
            "another new key",
            with_last(operation(
                json!({"op": "attach_note", "note": {"new": "other"},
                "to": checkout(), "board": free(), "pinned": true}),
            )),
        ),
        (
            "a new element to hang on",
            with_last(the_new_note_hung(json!({"new": "box"}), free(), Some(true))),
        ),
        (
            "a new board",
            with_last(the_new_note_hung(
                checkout(),
                json!({"new": "board"}),
                Some(true),
            )),
        ),
        (
            "another board than the note's",
            with_last(the_new_note_hung(
                checkout(),
                json!({"existing": "other"}),
                Some(true),
            )),
        ),
        ("the note's board placed elsewhere", placed_elsewhere),
        (
            "no pinned",
            with_last(the_new_note_hung(checkout(), free(), None)),
        ),
        ("the attachment before the move", attached_before_the_move),
        ("two attachments", twice),
        (
            "a detachment instead",
            with_last(operation(
                json!({"op": "detach_note", "note": {"new": "note"},
                "board": free()}),
            )),
        ),
        ("a fifth operation", with_a_fifth),
    ] {
        assert_eq!(
            admit(&request),
            Err(Refused::Operation {
                position: 1,
                kind: "create_element"
            }),
            "{shape}"
        );
    }
}

#[test]
fn names_nothing_of_a_refused_new_note_with_an_attachment() {
    let request: Vec<Operation> = [
        json!({"op": "create_element", "key": "secret-key", "type_ref": "note",
               "name": "SECRET-NAME", "body": "SECRET-BODY", "statement": "idea"}),
        json!({"op": "place_element", "element": {"new": "secret-key"},
               "board": {"existing": "free"}}),
        json!({"op": "move_element", "element": {"new": "secret-key"},
               "board": {"existing": "free"}, "x": 1, "y": 2, "pinned": true}),
        json!({"op": "attach_note", "note": {"new": "secret-key"},
               "to": {"existing": "secret-id"}, "board": {"existing": "other"},
               "pinned": true}),
    ]
    .into_iter()
    .map(operation)
    .collect();

    let refused = admit(&request).unwrap_err().to_string();

    for secret in ["secret", "SECRET", "other"] {
        assert!(!refused.contains(secret), "{refused}");
    }
}

#[test]
fn lets_a_new_note_hung_on_a_box_pass_the_board_files_on_every_board() {
    for board in ["free", "plain", "mirrored", "nowhere"] {
        let mut request = a_new_note_on(board);
        request.push(the_new_note_hung(
            json!({"existing": "checkout"}),
            json!({"existing": board}),
            Some(true),
        ));

        assert_eq!(check_board_files(&snapshot(), &request), Ok(()), "{board}");
    }
}

#[test]
fn refuses_attaching_or_detaching_an_element_without_a_note_s_file_on_every_board() {
    for board in ["plain", "mirrored"] {
        for note in ["checkout", "nowhere"] {
            for refused in [
                an_attachment(note, "checkout", board),
                a_detachment(note, board),
            ] {
                assert_eq!(
                    check_board_files(
                        &snapshot_with_an_attached_note(5, 6),
                        &[a_note_move("payments", "plain"), refused]
                    ),
                    Err(Refused::NotANote { position: 2 }),
                    "{note} on {board}"
                );
            }
        }
    }
}

#[test]
fn lets_a_note_on_which_nothing_hangs_be_attached_and_detached_on_a_mirrored_board() {
    let snapshot = snapshot_with_an_attached_note(5, 6);

    assert_eq!(
        check_board_files(
            &snapshot,
            &[
                a_note_move("payments", "mirrored"),
                an_attachment("payments", "checkout", "mirrored"),
                a_detachment("payments", "mirrored"),
            ]
        ),
        Ok(())
    );
}

/// `snapshot_with_a_note` whose boards `plain` and `mirrored` place the box
/// `x` named as hanging on the note `payments` by `lines`, with `payments`
/// placed too.
fn snapshot_naming_the_note(lines: &str) -> Snapshot {
    let mut snapshot = snapshot_with_a_note();
    for board in ["plain", "mirrored"] {
        let id: BoardId = board.parse().unwrap();
        let zone = if board == "mirrored" {
            r#""zone": {"mode": "mirrored", "statement": "current"}, "#
        } else {
            ""
        };
        let text = format!(
            r#"{{"format": 1, "id": "{board}", "title": "{board}", {zone}"placements": [{lines}]}}"#
        );
        snapshot
            .boards
            .insert(id.clone(), Board::from_json(&id, &text).unwrap());
    }
    snapshot
}

/// The three ways a file names the note `payments`: an attachment that holds,
/// one out of range, and one in a chain.
fn files_naming_the_note() -> [&'static str; 3] {
    [
        r#"{"element": "payments", "x": 0, "y": 0, "width": 180, "height": 90},
           {"element": "x", "x": 1, "y": 2, "width": 180, "height": 90, "attached_to": "payments"}"#,
        r#"{"element": "payments", "x": 0, "y": 0, "width": 180, "height": 90},
           {"element": "x", "x": 2000000, "y": 2, "width": 180, "height": 90, "attached_to": "payments"}"#,
        r#"{"element": "b", "x": 500, "y": 0, "width": 180, "height": 90},
           {"element": "payments", "x": 0, "y": 0, "width": 180, "height": 90, "attached_to": "b"},
           {"element": "x", "x": 1, "y": 2, "width": 180, "height": 90, "attached_to": "payments"}"#,
    ]
}

#[test]
fn lets_a_note_a_file_names_be_moved_attached_and_detached_on_every_board() {
    for lines in files_naming_the_note() {
        let snapshot = snapshot_naming_the_note(lines);
        let on = |board: &str| {
            [
                a_note_move("payments", board),
                an_attachment("payments", "b", board),
                a_detachment("payments", board),
            ]
        };

        for board in ["plain", "mirrored"] {
            assert_eq!(
                check_board_files(&snapshot, &on(board)),
                Ok(()),
                "{board}: {lines}"
            );
        }
    }
}

#[test]
fn refuses_to_delete_a_note_a_file_names_on_any_board() {
    let delete = operation(json!({"op": "delete_element", "element": {"existing": "payments"}}));
    for lines in files_naming_the_note() {
        let mut snapshot = snapshot_naming_the_note(lines);
        // Only the board `plain` names the note.
        let mirrored: BoardId = "mirrored".parse().unwrap();
        snapshot
            .boards
            .insert(mirrored.clone(), board("mirrored", Some(Mode::Mirrored)));

        assert_eq!(
            check_board_files(&snapshot, std::slice::from_ref(&delete)),
            Err(Refused::HungOn {
                position: 1,
                board: "plain".parse().unwrap(),
            }),
            "{lines}"
        );
    }
    assert_eq!(
        check_board_files(&snapshot_with_a_note(), &[delete]),
        Ok(())
    );
}

#[test]
fn names_only_the_position_and_the_board_of_a_refused_deletion() {
    let refused = Refused::HungOn {
        position: 3,
        board: "plain".parse().unwrap(),
    };

    assert_eq!(
        refused.to_string(),
        "operation 3: something on board plain hangs on the note, so the canvas does not delete it"
    );
}

/// Every mode a zone can have. The match breaks the build when a mode is added,
/// so whoever adds one decides here whether admission must treat it apart.
fn every_mode() -> [Mode; 2] {
    let listed = |mode: Mode| match mode {
        Mode::Free | Mode::Mirrored => mode,
    };
    [listed(Mode::Free), listed(Mode::Mirrored)]
}

/// The board `here`, of the zone `zone`, placing the box `checkout` at 100,
/// 200, pinned, the note `ask` hanging on it, and the note `payments`, on which
/// the box `x` hangs; `checkout` is no note, `ask` and `payments` are.
fn arranged_board(zone: Option<Mode>) -> Snapshot {
    let zone = zone.map_or(String::new(), |mode| {
        format!(
            r#""zone": {{"mode": "{}", "statement": "current"}}, "#,
            mode.as_str()
        )
    });
    let text = format!(
        r#"{{"format": 1, "id": "here", "title": "here", {zone}"placements": [
            {{"element": "ask", "x": 5, "y": 6, "width": 180, "height": 90, "pinned": true, "attached_to": "checkout"}},
            {{"element": "checkout", "x": 100, "y": 200, "width": 180, "height": 90, "pinned": true}},
            {{"element": "payments", "x": 400, "y": 0, "width": 180, "height": 90}},
            {{"element": "x", "x": 1, "y": 2, "width": 180, "height": 90, "attached_to": "payments"}}
        ]}}"#
    );
    let id: BoardId = "here".parse().unwrap();
    let element = |id: &str, type_ref: &str| {
        (
            id.parse::<ElementId>().unwrap(),
            ElementFixture::new(id).type_ref(type_ref).name(id).build(),
        )
    };
    Snapshot {
        elements: [
            element("ask", "note"),
            element("checkout", "c4/container"),
            element("payments", "note"),
        ]
        .into_iter()
        .collect(),
        boards: std::iter::once((id.clone(), Board::from_json(&id, &text).unwrap())).collect(),
    }
}

/// One of each operation `check_board_files` decides on a board.
fn every_kind_of_request() -> Vec<(&'static str, Vec<Operation>)> {
    vec![
        (
            "move and pin a box",
            vec![operation(
                json!({"op": "move_element", "element": {"existing": "checkout"},
                "board": {"existing": "here"}, "x": 300, "y": 300, "pinned": true}),
            )],
        ),
        (
            "release a pin where the file has it",
            vec![operation(
                json!({"op": "move_element", "element": {"existing": "checkout"},
                "board": {"existing": "here"}, "x": 100, "y": 200, "pinned": false}),
            )],
        ),
        (
            "release a pin elsewhere",
            vec![operation(
                json!({"op": "move_element", "element": {"existing": "checkout"},
                "board": {"existing": "here"}, "x": 101, "y": 200, "pinned": false}),
            )],
        ),
        (
            "attach a note something hangs on",
            vec![an_attachment("payments", "checkout", "here")],
        ),
        (
            "detach a note something hangs on",
            vec![a_detachment("payments", "here")],
        ),
        (
            "move a note something hangs on",
            vec![a_note_move("payments", "here")],
        ),
        (
            "update a box",
            vec![operation(
                json!({"op": "update_element", "element": {"existing": "checkout"},
                "name": "Checkout"}),
            )],
        ),
        (
            "delete a box",
            vec![operation(
                json!({"op": "delete_element", "element": {"existing": "checkout"}}),
            )],
        ),
        (
            "delete a note something hangs on",
            vec![operation(
                json!({"op": "delete_element", "element": {"existing": "payments"}}),
            )],
        ),
        ("create a note", a_new_note_on("here")),
        ("create a note hung on a box", {
            let mut request = a_new_note_on("here");
            request.push(the_new_note_hung(
                json!({"existing": "checkout"}),
                json!({"existing": "here"}),
                Some(true),
            ));
            request
        }),
    ]
}

#[test]
fn decides_every_request_on_every_mode_as_on_a_board_without_a_zone() {
    for (request, operations) in every_kind_of_request() {
        let without_a_zone = check_board_files(&arranged_board(None), &operations);
        for mode in every_mode() {
            assert_eq!(
                check_board_files(&arranged_board(Some(mode)), &operations),
                without_a_zone,
                "{request} on {mode:?}"
            );
        }
    }
}

#[test]
fn decides_a_mirrored_board_by_the_rules_of_every_board() {
    let decided: Vec<(&str, Result<(), Refused>)> = every_kind_of_request()
        .into_iter()
        .map(|(request, operations)| {
            (
                request,
                check_board_files(&arranged_board(Some(Mode::Mirrored)), &operations),
            )
        })
        .collect();
    let here: BoardId = "here".parse().unwrap();

    assert_eq!(
        decided,
        [
            ("move and pin a box", Ok(())),
            ("release a pin where the file has it", Ok(())),
            (
                "release a pin elsewhere",
                Err(Refused::ReleaseElsewhere {
                    position: 1,
                    board: here.clone(),
                })
            ),
            ("attach a note something hangs on", Ok(())),
            ("detach a note something hangs on", Ok(())),
            ("move a note something hangs on", Ok(())),
            ("update a box", Err(Refused::NotANote { position: 1 })),
            ("delete a box", Err(Refused::NotANote { position: 1 })),
            (
                "delete a note something hangs on",
                Err(Refused::HungOn {
                    position: 1,
                    board: here,
                })
            ),
            ("create a note", Ok(())),
            ("create a note hung on a box", Ok(())),
        ]
    );
}
