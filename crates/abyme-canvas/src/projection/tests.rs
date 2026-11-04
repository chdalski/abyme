//! Tests of a board's projection.

use super::*;
use std::collections::BTreeMap;
use std::num::NonZeroU32;

use abyme_model::{Board, ElementFixture, Rect};
use serde_json::{Value, json};

fn element_id(value: &str) -> ElementId {
    value.parse().unwrap()
}

fn board_id(value: &str) -> BoardId {
    value.parse().unwrap()
}

fn placed(x: i32, y: i32, width: u32, height: u32) -> abyme_model::Placement {
    abyme_model::Placement {
        rect: Rect {
            x,
            y,
            width: NonZeroU32::new(width).unwrap(),
            height: NonZeroU32::new(height).unwrap(),
        },
        pinned: false,
        attached_to: None,
    }
}

fn element(id: &str, name: &str, summary: Option<&str>, body: &str) -> Element {
    ElementFixture::new(id)
        .type_ref("c4/container")
        .name(name)
        .summary(summary)
        .body(body)
        .build()
}

/// A snapshot with the board `context` placing `payments` and `orders`,
/// whose file is missing.
fn snapshot() -> Snapshot {
    let board = Board {
        id: board_id("context"),
        title: "Context".to_owned(),
        owner: None,
        summary: None,
        zone: None,
        placements: BTreeMap::from([
            (element_id("payments"), placed(40, 80, 180, 90)),
            (element_id("orders"), placed(-20, 300, 200, 100)),
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
            mode: None,
            placements: vec![
                Placement {
                    element: element_id("orders"),
                    x: -20,
                    y: 300,
                    width: 200,
                    height: 100,
                    shown: Shown::Missing,
                    inner_boards: Vec::new(),
                    pinned: false,
                    attached_to: None,
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
                    inner_boards: Vec::new(),
                    pinned: false,
                    attached_to: None,
                },
            ],
            relations: Vec::new(),
            trail: Vec::new(),
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
        placed(i32::MIN, i32::MAX, u32::MAX, u32::MAX),
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
    assert_eq!(
        keys(&json),
        ["id", "mode", "placements", "relations", "title", "trail"]
    );
    let placements = json["placements"].as_array().unwrap();
    assert_eq!(
        keys(&placements[0]),
        [
            "attached_to",
            "element",
            "height",
            "inner_boards",
            "pinned",
            "shown",
            "width",
            "x",
            "y"
        ]
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

/// `snapshot()` with `payments` relating to `to` as given, with `SECRET-BODY`
/// in its description.
fn with_relation_to(to: &str, label: Option<&str>) -> Snapshot {
    let mut snapshot = snapshot();
    let mut fields = snapshot.elements[&element_id("payments")]
        .clone()
        .into_fields();
    fields.relations.insert(
        (RelationKind::Uses, element_id(to)),
        label.map(str::to_owned),
    );
    snapshot.elements.insert(
        element_id("payments"),
        Element::new(element_id("payments"), fields).unwrap(),
    );
    snapshot
}

#[test]
fn carries_a_relation_to_a_box_whose_file_is_missing_with_its_full_label() {
    let label = "x".repeat(100);
    let snapshot = with_relation_to("orders", Some(&label));

    let view = project(&snapshot, &board_id("context")).unwrap();

    assert_eq!(
        view.relations,
        [Relation {
            from: element_id("payments"),
            kind: RelationKind::Uses,
            to: element_id("orders"),
            label: Some(label),
        }]
    );
    assert!(
        !serde_json::to_string(&view)
            .unwrap()
            .contains("SECRET-BODY"),
        "{view:?}"
    );
}

#[test]
fn a_relation_carries_exactly_the_named_fields() {
    let view = project(
        &with_relation_to("orders", Some("REST")),
        &board_id("context"),
    )
    .unwrap();

    let json = serde_json::to_value(&view).unwrap();

    assert_eq!(
        json["relations"],
        json!([{"from": "payments", "kind": "uses", "to": "orders", "label": "REST"}])
    );
}

#[test]
fn leaves_out_a_relation_to_an_element_not_on_the_board() {
    let snapshot = with_relation_to("elsewhere", None);

    let view = project(&snapshot, &board_id("context")).unwrap();

    assert_eq!(view.relations, []);
}

#[test]
fn leaves_out_a_relation_of_a_source_whose_file_is_missing() {
    let mut snapshot = with_relation_to("orders", None);
    snapshot.elements.remove(&element_id("payments"));

    let view = project(&snapshot, &board_id("context")).unwrap();

    assert_eq!(view.relations, []);
}

/// A board `id`, the inside of `owner`, placing `placed`, titled `<id> title`.
fn tree_board(id: &str, owner: Option<&str>, placed: &[&str]) -> Board {
    Board {
        id: board_id(id),
        title: format!("{id} title"),
        owner: owner.map(element_id),
        summary: Some("SECRET-SUMMARY".to_owned()),
        zone: None,
        placements: placed
            .iter()
            .map(|element| (element_id(element), placed_at_origin()))
            .collect(),
    }
}

fn placed_at_origin() -> abyme_model::Placement {
    placed(0, 0, 1, 1)
}

/// A snapshot of `boards` and a file for each of `elements`.
fn tree_snapshot(boards: Vec<Board>, elements: &[&str]) -> Snapshot {
    Snapshot {
        elements: elements
            .iter()
            .map(|id| (element_id(id), element(id, id, None, "SECRET-BODY")))
            .collect(),
        boards: boards
            .into_iter()
            .map(|board| (board.id.clone(), board))
            .collect(),
    }
}

/// The IDs of the trail of `board`.
fn trail_of(snapshot: &Snapshot, board: &str) -> Vec<String> {
    project(snapshot, &board_id(board))
        .unwrap()
        .trail
        .iter()
        .map(|entry| entry.id.as_str().to_owned())
        .collect()
}

#[test]
fn the_trail_names_the_boards_above_top_first() {
    let snapshot = tree_snapshot(
        vec![
            tree_board("top", None, &["shop"]),
            tree_board("middle", Some("shop"), &["orders"]),
            tree_board("low", Some("orders"), &[]),
        ],
        &["shop", "orders"],
    );

    assert_eq!(trail_of(&snapshot, "low"), ["top", "middle"]);
    assert_eq!(trail_of(&snapshot, "middle"), ["top"]);
    assert_eq!(trail_of(&snapshot, "top"), Vec::<String>::new());
}

#[test]
fn the_trail_is_empty_on_a_cycle_and_on_a_board_that_owns_itself() {
    let snapshot = tree_snapshot(
        vec![
            tree_board("loop-a", Some("in-b"), &["in-a"]),
            tree_board("loop-b", Some("in-a"), &["in-b"]),
            tree_board("own", Some("in-own"), &["in-own"]),
        ],
        &["in-a", "in-b", "in-own"],
    );

    assert_eq!(trail_of(&snapshot, "loop-a"), Vec::<String>::new());
    assert_eq!(trail_of(&snapshot, "loop-b"), Vec::<String>::new());
    assert_eq!(trail_of(&snapshot, "own"), Vec::<String>::new());
}

#[test]
fn the_trail_of_a_board_below_a_cycle_ends_at_the_first_board_of_the_cycle() {
    let snapshot = tree_snapshot(
        vec![
            tree_board("loop-a", Some("in-b"), &["in-a", "hang"]),
            tree_board("loop-b", Some("in-a"), &["in-b"]),
            tree_board("below", Some("hang"), &[]),
        ],
        &["in-a", "in-b", "hang"],
    );

    assert_eq!(trail_of(&snapshot, "below"), ["loop-a"]);
}

#[test]
fn the_trail_ends_at_an_owner_without_a_file_or_a_home() {
    let snapshot = tree_snapshot(
        vec![
            tree_board("top", None, &["no-file"]),
            tree_board("unfiled", Some("no-file"), &[]),
            tree_board("homeless", Some("nowhere"), &[]),
        ],
        &["nowhere"],
    );

    assert_eq!(trail_of(&snapshot, "unfiled"), Vec::<String>::new());
    assert_eq!(trail_of(&snapshot, "homeless"), Vec::<String>::new());
}

#[test]
fn a_placement_names_the_boards_its_element_owns_with_only_id_and_title() {
    let snapshot = tree_snapshot(
        vec![
            tree_board("top", None, &["shop", "plain"]),
            tree_board("first", Some("shop"), &[]),
            tree_board("second", Some("shop"), &[]),
        ],
        &["shop", "plain"],
    );

    let view = project(&snapshot, &board_id("top")).unwrap();
    let json = serde_json::to_value(&view).unwrap();

    assert_eq!(
        json["placements"][1]["inner_boards"],
        json!([{"id": "first", "title": "first title"},
               {"id": "second", "title": "second title"}])
    );
    assert_eq!(json["placements"][0]["inner_boards"], json!([]));
    assert!(!json.to_string().contains("SECRET"), "{json}");
}

#[test]
fn says_a_placement_is_pinned_also_when_its_element_file_is_missing() {
    let mut snapshot = snapshot();
    snapshot
        .boards
        .get_mut(&board_id("context"))
        .unwrap()
        .placements
        .get_mut(&element_id("orders"))
        .unwrap()
        .pinned = true;

    let view = project(&snapshot, &board_id("context")).unwrap();

    let [orders, payments] = view.placements.as_slice() else {
        panic!("two placements expected");
    };
    assert!(orders.pinned);
    assert_eq!(orders.shown, Shown::Missing);
    assert!(!payments.pinned);
}

#[test]
fn an_attached_note_lies_at_its_position_on_the_board_with_its_own_size() {
    let text = r#"{"format": 1, "id": "context", "title": "Context", "placements": [
        {"element": "b", "x": 100, "y": 200, "width": 180, "height": 90},
        {"element": "lost", "x": 7, "y": 8, "width": 30, "height": 40, "attached_to": "gone"},
        {"element": "n", "x": 5, "y": 6, "width": 50, "height": 60, "attached_to": "b"}
    ]}"#;
    let board = Board::from_json(&board_id("context"), text).unwrap();
    let snapshot = Snapshot {
        elements: BTreeMap::new(),
        boards: BTreeMap::from([(board.id.clone(), board)]),
    };

    let view = project(&snapshot, &board_id("context")).unwrap();

    let rect_of = |id: &str| {
        view.placements
            .iter()
            .find(|placement| placement.element == element_id(id))
            .map(|placement| (placement.x, placement.y, placement.width, placement.height))
    };
    assert_eq!(rect_of("n"), Some((105, 206, 50, 60)));
    assert_eq!(rect_of("lost"), Some((7, 8, 30, 40)));
    assert_eq!(rect_of("b"), Some((100, 200, 180, 90)));
    let attached_to = |id: &str| {
        view.placements
            .iter()
            .find(|placement| placement.element == element_id(id))
            .and_then(|placement| placement.attached_to.clone())
    };
    assert_eq!(attached_to("n"), Some(element_id("b")));
    assert_eq!(attached_to("lost"), None);
    assert_eq!(attached_to("b"), None);
}
