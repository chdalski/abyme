//! Tests of which operations the canvas may send.

use super::*;
use std::collections::BTreeMap;
use std::num::NonZeroU32;

use abyme_core::LocalKey;
use abyme_model::{
    Element, ElementFields, Placement, Rect, Relations, StatementKind, TypeRef, Zone,
};
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
        (
            json!({"op": "attach_note", "note": {"existing": "secret-id"}, "to": {"existing": "payments"}, "board": {"existing": "context"}}),
            "attach_note",
        ),
        (
            json!({"op": "attach_note", "note": {"new": "secret-key"}, "to": {"existing": "payments"}, "board": {"existing": "mirrored"}, "pinned": true}),
            "attach_note",
        ),
        (
            json!({"op": "detach_note", "note": {"existing": "secret-id"}, "board": {"existing": "context"}}),
            "detach_note",
        ),
        (
            json!({"op": "detach_note", "note": {"existing": "secret-id"}, "board": {"existing": "mirrored"}}),
            "detach_note",
        ),
    ];

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
fn refuses_a_move_without_a_note_s_file_on_a_mirrored_board_and_names_where() {
    let requests: [(&[&str], usize); 3] = [
        (&["mirrored"], 1),
        (&["free", "plain", "mirrored"], 3),
        (&["free", "mirrored", "mirrored"], 2),
    ];

    for (boards, position) in requests {
        let operations: Vec<Operation> = boards.iter().map(|board| a_move_on(board)).collect();

        assert_eq!(
            check_board_files(&snapshot(), &operations),
            Err(Refused::Mirrored {
                position,
                board: "mirrored".parse().unwrap(),
            }),
            "{boards:?}"
        );
    }
}

#[test]
fn lets_a_move_pass_on_a_free_board_on_no_zone_and_on_a_board_it_does_not_have() {
    for board in ["free", "plain", "nowhere"] {
        assert_eq!(
            check_board_files(&snapshot(), &[a_move_on(board)]),
            Ok(()),
            "{board}"
        );
    }
}

#[test]
fn names_only_the_position_and_the_board_of_a_refused_move() {
    let refused = Refused::Mirrored {
        position: 2,
        board: "mirrored".parse().unwrap(),
    };

    assert_eq!(
        refused.to_string(),
        "operation 2: board mirrored is mirrored, so the canvas may only create a note there, or move one and keep it pinned"
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
    for board in ["free", "plain"] {
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
fn refuses_a_release_without_a_note_s_file_on_a_mirrored_board() {
    assert_eq!(
        check_board_files(&snapshot(), &[a_release_on("mirrored")]),
        Err(Refused::Mirrored {
            position: 1,
            board: "mirrored".parse().unwrap(),
        })
    );
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
    let note = Element::new(
        id.clone(),
        ElementFields {
            statement: None,
            type_ref: TypeRef::try_from("note".to_owned()).unwrap(),
            name: "Ask".to_owned(),
            summary: None,
            body: String::new(),
            relations: Relations::new(),
            anchor: None,
            verified: None,
        },
    )
    .unwrap();
    let mut snapshot = snapshot();
    snapshot.elements.insert(id, note);
    snapshot
}

#[test]
fn lets_only_a_move_that_keeps_a_note_pinned_pass_on_a_mirrored_board() {
    let with_pinned = |pinned: Value| {
        operation(json!({
            "op": "move_element",
            "element": {"existing": "payments"},
            "board": {"existing": "mirrored"},
            "x": 10,
            "y": 20,
            "pinned": pinned
        }))
    };
    let refused = Err(Refused::Mirrored {
        position: 1,
        board: "mirrored".parse().unwrap(),
    });
    let without_pinned = operation(json!({
        "op": "move_element",
        "element": {"existing": "payments"},
        "board": {"existing": "mirrored"},
        "x": 10,
        "y": 20
    }));
    let new_element = operation(json!({
        "op": "move_element",
        "element": {"new": "payments"},
        "board": {"existing": "mirrored"},
        "x": 10,
        "y": 20,
        "pinned": true
    }));

    assert_eq!(
        check_board_files(&snapshot_with_a_note(), &[with_pinned(json!(true))]),
        Ok(())
    );
    for not_keeping_a_pin in [with_pinned(json!(false)), without_pinned, new_element] {
        assert_eq!(
            check_board_files(&snapshot_with_a_note(), &[not_keeping_a_pin]),
            refused
        );
    }
    assert_eq!(
        check_board_files(&snapshot(), &[with_pinned(json!(true))]),
        refused,
        "no file says it is a note"
    );
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
fn a_new_note_with_anything_beside_it_does_not_pass_the_mirrored_check() {
    let beside = a_move_on("mirrored");
    let new_note = a_new_note_on("mirrored");
    let before: Vec<Operation> = std::iter::once(beside.clone())
        .chain(new_note.iter().cloned())
        .collect();
    let after: Vec<Operation> = new_note.iter().cloned().chain([beside]).collect();

    for request in [before, after] {
        assert!(matches!(
            check_board_files(&snapshot(), &request),
            Err(Refused::Mirrored { .. })
        ));
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
fn lets_a_move_of_an_attached_note_pass_on_a_mirrored_board_but_not_of_its_box() {
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
    assert_eq!(
        check_board_files(&snapshot, &[a_move("checkout")]),
        Err(Refused::Mirrored {
            position: 1,
            board: "mirrored".parse().unwrap(),
        })
    );
}
