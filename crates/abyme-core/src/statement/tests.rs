//! Tests of each element's statement kind.

use std::collections::BTreeMap;
use std::num::NonZeroU32;

use abyme_model::{
    Board, Element, ElementFields, Mode, Rect, Relations, StatementKind, TypeRef, Zone,
};

use super::*;

fn element_id(value: &str) -> ElementId {
    value.parse().unwrap()
}

fn board_id(value: &str) -> BoardId {
    value.parse().unwrap()
}

fn element(id: &str) -> Element {
    Element::new(
        element_id(id),
        ElementFields {
            type_ref: TypeRef::try_from("t".to_owned()).unwrap(),
            name: id.to_owned(),
            summary: None,
            body: String::new(),
            relations: Relations::new(),
            anchor: None,
            verified: None,
        },
    )
    .unwrap()
}

/// The board `id`, a zone of `kind` if one is given, the inside of `owner`,
/// placing `placed`.
fn board(id: &str, kind: Option<StatementKind>, owner: Option<&str>, placed: &[&str]) -> Board {
    let rect = Rect {
        x: 0,
        y: 0,
        width: NonZeroU32::MIN,
        height: NonZeroU32::MIN,
    };
    Board {
        id: board_id(id),
        title: id.to_owned(),
        owner: owner.map(element_id),
        summary: None,
        zone: kind.map(|statement| Zone {
            mode: Mode::Free,
            statement,
        }),
        placements: placed
            .iter()
            .map(|placed| (element_id(placed), rect))
            .collect(),
    }
}

/// `boards` and a file for each element in `elements`.
fn snapshot(boards: Vec<Board>, elements: &[&str]) -> Snapshot {
    Snapshot {
        elements: elements
            .iter()
            .map(|id| (element_id(id), element(id)))
            .collect(),
        boards: boards
            .into_iter()
            .map(|board| (board.id.clone(), board))
            .collect(),
    }
}

#[test]
fn gives_each_element_the_kind_of_its_home_board_s_zone() {
    let snapshot = snapshot(
        vec![
            board("ideas", Some(StatementKind::Idea), None, &["spark"]),
            board("plain", None, None, &["thing"]),
        ],
        &["spark", "thing", "loose"],
    );

    assert_eq!(
        statement_kinds(&snapshot),
        BTreeMap::from([(element_id("spark"), StatementKind::Idea)])
    );
}

#[test]
fn takes_the_kind_of_the_lower_board_when_two_place_an_element() {
    let lower_is_a_zone = snapshot(
        vec![
            board("alpha", Some(StatementKind::Current), None, &["x"]),
            board("beta", Some(StatementKind::Idea), None, &["x"]),
        ],
        &["x"],
    );
    let lower_is_none = snapshot(
        vec![
            board("alpha", None, None, &["x"]),
            board("beta", Some(StatementKind::Idea), None, &["x"]),
        ],
        &["x"],
    );
    let tree = BoardTree::of(&lower_is_a_zone.boards);

    assert_eq!(
        kind_of(&lower_is_a_zone, &tree, &element_id("x")),
        Some((StatementKind::Current, &board_id("alpha")))
    );
    assert_eq!(statement_kinds(&lower_is_none), BTreeMap::new());
}

#[test]
fn leaves_out_a_placement_whose_element_file_is_missing() {
    let snapshot = snapshot(
        vec![board("ideas", Some(StatementKind::Idea), None, &["ghost"])],
        &[],
    );

    assert_eq!(statement_kinds(&snapshot), BTreeMap::new());
}

#[test]
fn takes_no_kind_from_the_board_above() {
    // `inside`, no zone, is the inside of `shop`, which lies on the zone
    // `context`.
    let snapshot = snapshot(
        vec![
            board("context", Some(StatementKind::Current), None, &["shop"]),
            board("inside", None, Some("shop"), &["part"]),
        ],
        &["shop", "part"],
    );

    assert_eq!(
        statement_kinds(&snapshot),
        BTreeMap::from([(element_id("shop"), StatementKind::Current)])
    );
}
