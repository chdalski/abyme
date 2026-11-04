//! Tests of turning operations into file writes.

use super::*;
use std::num::NonZeroU32;

use abyme_model::{ElementFixture, Mode, Rect, StatementKind, hides_or_breaks_text};
use rstest::rstest;

use crate::operation::Operations;

mod attachments;
mod deletion;
mod hierarchy;
mod ids;
mod layout;
mod moves;
mod notes;
mod placement;
mod relations;
mod request;
mod stamps;
mod statements;
mod updates;

fn key(value: &str) -> LocalKey {
    LocalKey::try_from(value.to_owned()).unwrap()
}

fn element_id(value: &str) -> ElementId {
    value.parse().unwrap()
}

fn board_id(value: &str) -> BoardId {
    value.parse().unwrap()
}

fn placed(x: i32, y: i32) -> Placement {
    sized(x, y, 180, 90)
}

fn element(id: &str) -> Element {
    ElementFixture::new(id).build()
}

/// A snapshot with the elements `orders-db` (placed on `main`) and `loose`
/// (on no board), the board `main`, and an orphan placement `ghost` whose
/// element file is missing.
fn snapshot() -> Snapshot {
    let main = Board {
        id: board_id("main"),
        title: "Main".to_owned(),
        owner: None,
        summary: None,
        zone: None,
        placements: BTreeMap::from([
            (element_id("orders-db"), placed(0, 0)),
            (element_id("ghost"), placed(220, 0)),
        ]),
    };
    Snapshot {
        elements: BTreeMap::from([
            (element_id("orders-db"), element("orders-db")),
            (element_id("loose"), element("loose")),
        ]),
        boards: BTreeMap::from([(board_id("main"), main)]),
    }
}

/// The placements of the board `board` in `snapshot`, for a test to change.
fn placements_on<'a>(
    snapshot: &'a mut Snapshot,
    board: &str,
) -> &'a mut BTreeMap<ElementId, Placement> {
    &mut snapshot
        .boards
        .get_mut(&board_id(board))
        .unwrap()
        .placements
}

fn operations(json: &str) -> Vec<Operation> {
    serde_json::from_str::<Operations>(json).unwrap().into_vec()
}

fn problem(snapshot: &Snapshot, json: &str) -> OperationProblem {
    match apply(snapshot, &operations(json)) {
        Err(ApplyError::Operation { problem, .. }) => problem,
        other => panic!("expected an operation error, got {other:?}"),
    }
}

fn written_board<'a>(applied: &'a Applied, id: &str) -> &'a Board {
    applied
        .writes
        .iter()
        .find_map(|write| match write {
            Write::Board(board) if board.id.as_str() == id => Some(board),
            Write::Board(_) | Write::Element(_) | Write::RemoveElement(_) => None,
        })
        .unwrap()
}

fn sized(x: i32, y: i32, width: u32, height: u32) -> Placement {
    Placement {
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

/// The element files a request writes, by ID, in the order of the writes.
fn written_elements(applied: &Applied) -> Vec<&Element> {
    applied
        .writes
        .iter()
        .filter_map(|write| match write {
            Write::Element(element) => Some(element),
            Write::Board(_) | Write::RemoveElement(_) => None,
        })
        .collect()
}

/// The element `id` as the request wrote it, if it wrote one.
fn written_element<'a>(applied: &'a Applied, id: &str) -> Option<&'a Element> {
    written_elements(applied)
        .into_iter()
        .find(|element| element.id().as_str() == id)
}

fn relations_of(element: &Element) -> Vec<(RelationKind, &str, Option<&str>)> {
    element
        .relations()
        .iter()
        .map(|((kind, to), label)| (*kind, to.as_str(), label.as_deref()))
        .collect()
}

/// The snapshot with a second board `side` that places `orders-db` too.
fn snapshot_for_deletion() -> Snapshot {
    let mut snapshot = snapshot();
    let side = Board {
        id: board_id("side"),
        title: "Side".to_owned(),
        owner: None,
        summary: None,
        zone: None,
        placements: BTreeMap::from([(element_id("orders-db"), placed(0, 0))]),
    };
    snapshot.boards.insert(side.id.clone(), side);
    snapshot
}

fn removals(applied: &Applied) -> Vec<&str> {
    applied
        .writes
        .iter()
        .filter_map(|write| match write {
            Write::RemoveElement(id) => Some(id.as_str()),
            Write::Element(_) | Write::Board(_) => None,
        })
        .collect()
}

const COMMIT: &str = "0123456789abcdef0123456789abcdef01234567";

/// The snapshot with `anchored`, anchored on `crates/anchored` and related
/// to `ghost`, which has no file, and `native`, without an anchor; both
/// carry a stamp.
fn stamped_snapshot() -> Snapshot {
    let mut snapshot = snapshot();
    for (id, anchor, relations) in [
        (
            "anchored",
            "anchor: crates/anchored\n",
            "relations:\n  - {kind: uses, to: ghost}\n",
        ),
        ("native", "", ""),
    ] {
        let element = Element::from_markdown(
            &element_id(id),
            &format!(
                "---\nid: {id}\ntype: t\nname: N\n{anchor}verified: \"{COMMIT}\"\nverified_by: person\n{relations}---\n"
            ),
        )
        .unwrap();
        snapshot.elements.insert(element_id(id), element);
    }
    snapshot
}

/// `snapshot()` with `orders-db` pinned on `main`.
fn with_a_pin() -> Snapshot {
    let mut snapshot = snapshot();
    placements_on(&mut snapshot, "main")
        .get_mut(&element_id("orders-db"))
        .unwrap()
        .pinned = true;
    snapshot
}

/// The placement line of `element` at `x` and `y`, 180 by 90, with `rest`
/// after the height.
fn placement_line(element: &str, x: i32, y: i32, rest: &str) -> String {
    format!(r#"{{"element": "{element}", "x": {x}, "y": {y}, "width": 180, "height": 90{rest}}}"#)
}

/// The board `id` read from a file with these placement lines.
fn board_file(id: &str, lines: &[String]) -> Board {
    let text = format!(
        r#"{{"format": 1, "id": "{id}", "title": "t", "placements": [{}]}}"#,
        lines.join(", ")
    );
    Board::from_json(&board_id(id), &text).unwrap()
}

/// A snapshot with the boards given and a file for each element they place.
fn with_files_for(boards: Vec<Board>) -> Snapshot {
    Snapshot {
        elements: boards
            .iter()
            .flat_map(|board| board.placements.keys())
            .map(|id| (id.clone(), element(id.as_str())))
            .collect(),
        boards: boards
            .into_iter()
            .map(|board| (board.id.clone(), board))
            .collect(),
    }
}

fn a_move(element: &str, x: i32, y: i32) -> String {
    format!(
        r#"{{"op": "move_element", "element": {{"existing": "{element}"}}, "board": {{"existing": "main"}}, "x": {x}, "y": {y}}}"#
    )
}

fn rect_at(x: i32, y: i32) -> Rect {
    Rect {
        x,
        y,
        width: NonZeroU32::new(180).unwrap(),
        height: NonZeroU32::new(90).unwrap(),
    }
}

/// The line of `element` in the file of `board`.
fn line_of(board: &Board, element: &str) -> String {
    let needle = format!(r#""element": "{element}""#);
    board
        .to_json()
        .lines()
        .find(|line| line.contains(&needle))
        .unwrap()
        .to_owned()
}
