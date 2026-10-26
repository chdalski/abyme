//! Tests of a board's projection.

use super::*;
use std::collections::BTreeMap;
use std::num::NonZeroU32;

use abyme_model::{Board, ElementFields, Relations, TypeRef};
use serde_json::{Value, json};

fn element_id(value: &str) -> ElementId {
    value.parse().unwrap()
}

fn board_id(value: &str) -> BoardId {
    value.parse().unwrap()
}

fn rect(x: i32, y: i32, width: u32, height: u32) -> Rect {
    Rect {
        x,
        y,
        width: NonZeroU32::new(width).unwrap(),
        height: NonZeroU32::new(height).unwrap(),
    }
}

fn element(id: &str, name: &str, summary: Option<&str>, body: &str) -> Element {
    Element::new(
        element_id(id),
        ElementFields {
            type_ref: TypeRef::try_from("c4/container".to_owned()).unwrap(),
            name: name.to_owned(),
            summary: summary.map(str::to_owned),
            body: body.to_owned(),
            relations: Relations::new(),
        },
    )
    .unwrap()
}

/// A snapshot with the board `context` placing `payments` and `orders`,
/// whose file is missing.
fn snapshot() -> Snapshot {
    let board = Board {
        id: board_id("context"),
        title: "Context".to_owned(),
        placements: BTreeMap::from([
            (element_id("payments"), rect(40, 80, 180, 90)),
            (element_id("orders"), rect(-20, 300, 200, 100)),
        ]),
    };
    let payments = element(
        "payments",
        "Payment Service",
        Some("Payment processing"),
        "SECRET-BODY handles payments.",
    );
    Snapshot {
        elements: BTreeMap::from([(element_id("payments"), payments)]),
        boards: BTreeMap::from([(board_id("context"), board)]),
    }
}

#[test]
fn shows_a_box_per_placement_with_its_rect_and_the_element_s_fields() {
    let view = project(&snapshot(), &board_id("context")).unwrap();

    assert_eq!(
        view,
        BoardView {
            id: board_id("context"),
            title: "Context".to_owned(),
            placements: vec![
                Placement {
                    element: element_id("orders"),
                    x: -20,
                    y: 300,
                    width: 200,
                    height: 100,
                    shown: Shown::Missing,
                },
                Placement {
                    element: element_id("payments"),
                    x: 40,
                    y: 80,
                    width: 180,
                    height: 90,
                    shown: Shown::Element {
                        type_ref: "c4/container".to_owned(),
                        name: "Payment Service".to_owned(),
                        summary: Some("Payment processing".to_owned()),
                    },
                },
            ],
        }
    );
}

#[test]
fn an_unknown_board_is_an_error() {
    assert_eq!(
        project(&snapshot(), &board_id("elsewhere")),
        Err(UnknownBoard(board_id("elsewhere")))
    );
}

#[test]
fn extreme_rects_pass_unchanged() {
    let mut snapshot = snapshot();
    let board = snapshot.boards.get_mut(&board_id("context")).unwrap();
    board.placements = BTreeMap::from([(
        element_id("payments"),
        rect(i32::MIN, i32::MAX, u32::MAX, u32::MAX),
    )]);

    let view = project(&snapshot, &board_id("context")).unwrap();

    let [placement] = view.placements.as_slice() else {
        panic!("one placement expected");
    };
    assert_eq!(
        (placement.x, placement.y, placement.width, placement.height),
        (i32::MIN, i32::MAX, u32::MAX, u32::MAX)
    );
}

#[test]
fn the_projection_carries_exactly_the_named_fields_and_no_body() {
    let view = project(&snapshot(), &board_id("context")).unwrap();

    let json = serde_json::to_value(&view).unwrap();

    assert!(!json.to_string().contains("SECRET-BODY"));
    assert_eq!(keys(&json), ["id", "placements", "title"]);
    let placements = json["placements"].as_array().unwrap();
    assert_eq!(
        keys(&placements[0]),
        ["element", "height", "shown", "width", "x", "y"]
    );
    assert_eq!(placements[0]["shown"], json!({"kind": "missing"}));
    assert_eq!(
        keys(&placements[1]["shown"]),
        ["kind", "name", "summary", "type_ref"]
    );
}

fn keys(value: &Value) -> Vec<&str> {
    value
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect()
}
