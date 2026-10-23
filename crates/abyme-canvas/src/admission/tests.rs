//! Tests of which operations the canvas may send.

use super::*;

use abyme_core::LocalKey;
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
        "y": 20
    }))
}

#[test]
fn a_move_of_an_existing_element_on_an_existing_board_is_admitted() {
    assert_eq!(admit(&[move_existing()]), Ok(()));
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
