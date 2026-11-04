//! Tests of each element's statement kind.

use std::collections::BTreeMap;
use std::num::NonZeroU32;

use abyme_model::{Board, Element, ElementFixture, Mode, Placement, Rect, StatementKind, Zone};

use super::*;

fn element_id(value: &str) -> ElementId {
    value.parse().unwrap()
}

fn board_id(value: &str) -> BoardId {
    value.parse().unwrap()
}

fn element(id: &str) -> Element {
    ElementFixture::new(id).build()
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
            .map(|placed| {
                (
                    element_id(placed),
                    Placement {
                        rect,
                        pinned: false,
                        attached_to: None,
                    },
                )
            })
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
        kind_of(
            &lower_is_a_zone,
            &tree,
            &lower_is_a_zone.elements[&element_id("x")]
        ),
        Some((StatementKind::Current, KindSource::Home(&board_id("alpha"))))
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

/// `element(id)` holding `kind` itself.
fn holding(id: &str, kind: StatementKind) -> Element {
    let mut fields = element(id).into_fields();
    fields.statement = Some(kind);
    Element::new(element_id(id), fields).unwrap()
}

/// The kind `statement_kinds` gives `x`, an element holding `kind`, and the
/// source `kind_of` names, on `boards`.
fn own_kind_on(boards: Vec<Board>, kind: StatementKind) -> (Option<StatementKind>, bool) {
    let mut snapshot = snapshot(boards, &[]);
    snapshot
        .elements
        .insert(element_id("x"), holding("x", kind));
    let tree = BoardTree::of(&snapshot.boards);
    let from_the_element = matches!(
        kind_of(&snapshot, &tree, &snapshot.elements[&element_id("x")]),
        Some((_, KindSource::Element))
    );
    (
        statement_kinds(&snapshot).get(&element_id("x")).copied(),
        from_the_element,
    )
}

#[test]
fn an_own_kind_needs_no_home() {
    assert_eq!(
        own_kind_on(Vec::new(), StatementKind::Question),
        (Some(StatementKind::Question), true)
    );
}

#[test]
fn an_own_kind_counts_on_a_home_that_is_no_zone() {
    assert_eq!(
        own_kind_on(vec![board("b", None, None, &["x"])], StatementKind::Idea),
        (Some(StatementKind::Idea), true)
    );
}

#[test]
fn an_own_kind_wins_over_a_zone_of_another_kind() {
    assert_eq!(
        own_kind_on(
            vec![board("b", Some(StatementKind::Current), None, &["x"])],
            StatementKind::Idea
        ),
        (Some(StatementKind::Idea), true)
    );
}

#[test]
fn an_own_kind_on_a_zone_of_the_same_kind_comes_from_the_element() {
    assert_eq!(
        own_kind_on(
            vec![board("b", Some(StatementKind::Idea), None, &["x"])],
            StatementKind::Idea
        ),
        (Some(StatementKind::Idea), true)
    );
}

#[test]
fn an_own_kind_counts_on_an_element_several_boards_place() {
    assert_eq!(
        own_kind_on(
            vec![
                board("alpha", Some(StatementKind::Current), None, &["x"]),
                board("beta", Some(StatementKind::Target), None, &["x"]),
            ],
            StatementKind::Decision
        ),
        (Some(StatementKind::Decision), true)
    );
}
