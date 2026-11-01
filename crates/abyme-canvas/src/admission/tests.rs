//! Tests of which operations the canvas may send.

use super::*;
use std::collections::BTreeMap;

use abyme_core::LocalKey;
use abyme_model::{Board, StatementKind, Zone};
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
fn a_move_that_does_not_pin_is_refused_with_its_position() {
    for without_a_pin in [
        json!({"op": "move_element", "element": {"existing": "payments"}, "board": {"existing": "context"}, "x": 1, "y": 2}),
        json!({"op": "move_element", "element": {"existing": "payments"}, "board": {"existing": "context"}, "x": 1, "y": 2, "pinned": false}),
    ] {
        let refusal = admit(&[move_existing(), operation(without_a_pin)]).unwrap_err();

        assert_eq!(
            refusal,
            Refused::Operation {
                position: 2,
                kind: "move_element without pinned: true",
            }
        );
    }
}

#[test]
fn a_pin_does_not_admit_a_move_with_a_new_element_or_board() {
    for with_a_new_ref in [
        json!({"op": "move_element", "element": {"new": "e"}, "board": {"existing": "context"}, "x": 1, "y": 2, "pinned": true}),
        json!({"op": "move_element", "element": {"existing": "payments"}, "board": {"new": "b"}, "x": 1, "y": 2, "pinned": true}),
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
            json!({"op": "delete_element", "element": {"existing": "secret-id"}}),
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
fn refuses_a_move_on_a_mirrored_board_and_names_where() {
    let requests: [(&[&str], usize); 3] = [
        (&["mirrored"], 1),
        (&["free", "plain", "mirrored"], 3),
        (&["free", "mirrored", "mirrored"], 2),
    ];

    for (boards, position) in requests {
        let operations: Vec<Operation> = boards.iter().map(|board| a_move_on(board)).collect();

        assert_eq!(
            check_modes(&snapshot(), &operations),
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
            check_modes(&snapshot(), &[a_move_on(board)]),
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
        "operation 2: board mirrored is mirrored, so the canvas may not move its elements"
    );
}
