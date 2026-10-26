//! Properties of `apply` that hold for any snapshot and any request.

use std::collections::{BTreeMap, BTreeSet};
use std::num::NonZeroU32;

use abyme_core::{Applied, Created, LocalKey, Operation, Ref, Snapshot, Write, apply};
use abyme_model::{
    Board, BoardId, Element, ElementFields, ElementId, Rect, RelationKind, Relations, TypeRef,
};
use proptest::prelude::*;

const ELEMENT_IDS: [&str; 4] = ["orders-db", "checkout", "ghost", "payment-service"];
const BOARD_IDS: [&str; 2] = ["main", "context"];
const KEYS: [&str; 4] = ["a", "b", "c", "d"];

fn rect() -> impl Strategy<Value = Rect> {
    let size = (1..=u32::MAX).prop_filter_map("zero size", NonZeroU32::new);
    let small = (1..=400_u32).prop_filter_map("zero size", NonZeroU32::new);
    let width = prop_oneof![3 => small.clone(), 1 => size.clone()];
    let height = prop_oneof![3 => small, 1 => size];
    let position = prop_oneof![3 => -500..2_000_i32, 1 => any::<i32>()];
    (position.clone(), position, width, height).prop_map(|(x, y, width, height)| Rect {
        x,
        y,
        width,
        height,
    })
}

/// One of the known element IDs.
fn element_id() -> impl Strategy<Value = ElementId> {
    prop::sample::select(ELEMENT_IDS.to_vec()).prop_filter_map("valid ID", |id| id.parse().ok())
}

fn board_id() -> impl Strategy<Value = BoardId> {
    prop::sample::select(BOARD_IDS.to_vec()).prop_filter_map("valid ID", |id| id.parse().ok())
}

fn relation_kind() -> impl Strategy<Value = RelationKind> {
    prop::sample::select(vec![
        RelationKind::Concerns,
        RelationKind::DependsOn,
        RelationKind::Describes,
        RelationKind::SeeAlso,
        RelationKind::Uses,
    ])
}

/// Relations to the known element IDs, some of which have no file.
fn relations() -> impl Strategy<Value = Relations> {
    prop::collection::btree_map(
        (relation_kind(), element_id()),
        proptest::option::of(Just("label".to_owned())),
        0..3,
    )
}

/// An element with the given ID, fixed content, and the given relations
/// except one to itself.
fn element(id: ElementId, mut relations: Relations) -> Option<Element> {
    let type_ref = TypeRef::try_from("t".to_owned()).ok()?;
    relations.retain(|(_, to), _| *to != id);
    Element::new(
        id,
        ElementFields {
            type_ref,
            name: "name".to_owned(),
            summary: None,
            body: String::new(),
            relations,
            anchor: None,
            verified: None,
        },
    )
    .ok()
}

/// A snapshot with some of the known elements and boards; `ghost` never has
/// a file, so placing it leaves an orphan placement.
fn snapshot() -> impl Strategy<Value = Snapshot> {
    let elements = prop::collection::btree_map(element_id(), relations(), 0..=3).prop_map(|ids| {
        ids.into_iter()
            .filter(|(id, _)| id.as_str() != "ghost")
            .filter_map(|(id, relations)| {
                element(id.clone(), relations).map(|element| (id, element))
            })
            .collect::<BTreeMap<_, _>>()
    });
    let board = (
        board_id(),
        any::<String>(),
        prop::collection::btree_map(element_id(), rect(), 0..4),
    )
        .prop_map(|(id, title, placements)| Board {
            id,
            title,
            placements,
        });
    let boards = prop::collection::vec(board, 0..=2).prop_map(|boards| {
        boards
            .into_iter()
            .map(|board| (board.id.clone(), board))
            .collect()
    });
    (elements, boards).prop_map(|(elements, boards)| Snapshot { elements, boards })
}

fn key() -> impl Strategy<Value = LocalKey> {
    prop::sample::select(KEYS.to_vec())
        .prop_filter_map("valid key", |key| LocalKey::try_from(key.to_owned()).ok())
}

fn element_ref() -> impl Strategy<Value = Ref<ElementId>> {
    prop_oneof![
        key().prop_map(Ref::New),
        element_id().prop_map(Ref::Existing)
    ]
}

fn board_ref() -> impl Strategy<Value = Ref<BoardId>> {
    prop_oneof![key().prop_map(Ref::New), board_id().prop_map(Ref::Existing)]
}

/// Names that often collide with the snapshot's IDs, mixed with arbitrary
/// text.
fn name() -> impl Strategy<Value = String> {
    prop_oneof![
        prop::sample::select(vec![
            "Orders DB",
            "Ghost",
            "Main",
            "con",
            "!!!",
            "Payment Service"
        ])
        .prop_map(str::to_owned),
        any::<String>(),
    ]
}

fn operation() -> impl Strategy<Value = Operation> {
    prop_oneof![
        (
            key(),
            prop_oneof![Just("t".to_owned()), any::<String>()],
            name(),
            proptest::option::of(any::<String>()),
            any::<String>()
        )
            .prop_map(
                |(key, type_ref, name, summary, body)| Operation::CreateElement {
                    key,
                    type_ref,
                    name,
                    summary,
                    body,
                }
            ),
        (key(), name()).prop_map(|(key, title)| Operation::CreateBoard { key, title }),
        (element_ref(), board_ref())
            .prop_map(|(element, board)| Operation::PlaceElement { element, board }),
        (
            element_ref(),
            relation_kind(),
            element_ref(),
            proptest::option::of(prop_oneof![Just("REST".to_owned()), any::<String>()]),
        )
            .prop_map(|(from, kind, to, label)| Operation::CreateRelation {
                from,
                kind,
                to,
                label,
            }),
        (element_ref(), relation_kind(), element_ref())
            .prop_map(|(from, kind, to)| Operation::DeleteRelation { from, kind, to }),
        (
            element_ref(),
            proptest::option::of(name()),
            proptest::option::of(prop_oneof![Just("t".to_owned()), any::<String>()]),
            proptest::option::of(proptest::option::of(any::<String>())),
            proptest::option::of(any::<String>()),
        )
            .prop_map(|(element, name, type_ref, summary, body)| {
                Operation::UpdateElement {
                    element,
                    name,
                    type_ref,
                    summary,
                    body,
                }
            }),
        element_ref().prop_map(|element| Operation::DeleteElement { element }),
    ]
}

/// The existing IDs a request's operations delete: elements with a file,
/// or IDs a board places without one.
fn deleted_existing_elements(operations: &[Operation]) -> BTreeSet<&ElementId> {
    operations
        .iter()
        .filter_map(|operation| match operation {
            Operation::DeleteElement {
                element: Ref::Existing(id),
            } => Some(id),
            Operation::CreateElement { .. }
            | Operation::CreateBoard { .. }
            | Operation::PlaceElement { .. }
            | Operation::MoveElement { .. }
            | Operation::UpdateElement { .. }
            | Operation::DeleteElement { .. }
            | Operation::CreateRelation { .. }
            | Operation::DeleteRelation { .. } => None,
        })
        .collect()
}

/// The existing elements that a request's operations name as the element
/// they change: the source of a relation operation, or the element an
/// update names.
fn named_existing_elements(operations: &[Operation]) -> BTreeSet<&ElementId> {
    operations
        .iter()
        .filter_map(|operation| match operation {
            Operation::CreateRelation {
                from: Ref::Existing(id),
                ..
            }
            | Operation::DeleteRelation {
                from: Ref::Existing(id),
                ..
            }
            | Operation::UpdateElement {
                element: Ref::Existing(id),
                ..
            } => Some(id),
            Operation::CreateElement { .. }
            | Operation::CreateBoard { .. }
            | Operation::PlaceElement { .. }
            | Operation::MoveElement { .. }
            | Operation::UpdateElement { .. }
            | Operation::DeleteElement { .. }
            | Operation::CreateRelation { .. }
            | Operation::DeleteRelation { .. } => None,
        })
        .collect()
}

/// Arbitrary text without the characters an element file cannot hold.
fn printable_text() -> impl Strategy<Value = String> {
    any::<String>().prop_map(|text| {
        text.chars()
            .filter(|character| {
                !matches!(
                    character,
                    '\u{0}'..='\u{8}'
                        | '\u{b}'
                        | '\u{c}'
                        | '\u{e}'..='\u{1f}'
                        | '\u{7f}'..='\u{84}'
                        | '\u{86}'..='\u{9f}'
                        | '\u{fffe}'
                        | '\u{ffff}'
                )
            })
            .collect()
    })
}

/// A request that creates a board and several elements and places each on
/// the board, with arbitrary titles, names, summaries, and bodies.
fn well_formed_request() -> impl Strategy<Value = Vec<Operation>> {
    let element = (
        printable_text(),
        proptest::option::of(printable_text()),
        any::<String>(),
    );
    (any::<String>(), prop::collection::vec(element, 1..6)).prop_filter_map(
        "invalid key",
        |(title, elements)| {
            let board = LocalKey::try_from("board".to_owned()).ok()?;
            let mut operations = vec![Operation::CreateBoard {
                key: board.clone(),
                title,
            }];
            for (index, (name, summary, body)) in elements.into_iter().enumerate() {
                let key = LocalKey::try_from(format!("e{index}")).ok()?;
                operations.push(Operation::CreateElement {
                    key: key.clone(),
                    type_ref: "c4/container".to_owned(),
                    name,
                    summary,
                    body,
                });
                operations.push(Operation::PlaceElement {
                    element: Ref::New(key),
                    board: Ref::New(board.clone()),
                });
            }
            Some(operations)
        },
    )
}

fn overlaps(a: &Rect, b: &Rect) -> bool {
    let right = |rect: &Rect| i64::from(rect.x) + i64::from(rect.width.get());
    let bottom = |rect: &Rect| i64::from(rect.y) + i64::from(rect.height.get());
    i64::from(a.x) < right(b)
        && i64::from(b.x) < right(a)
        && i64::from(a.y) < bottom(b)
        && i64::from(b.y) < bottom(a)
}

/// The keys whose elements a request's operations delete again.
fn deleted_keys(operations: &[Operation]) -> BTreeSet<&LocalKey> {
    operations
        .iter()
        .filter_map(|operation| match operation {
            Operation::DeleteElement {
                element: Ref::New(key),
            } => Some(key),
            Operation::CreateElement { .. }
            | Operation::CreateBoard { .. }
            | Operation::PlaceElement { .. }
            | Operation::MoveElement { .. }
            | Operation::UpdateElement { .. }
            | Operation::DeleteElement { .. }
            | Operation::CreateRelation { .. }
            | Operation::DeleteRelation { .. } => None,
        })
        .collect()
}

/// Checks that the element files written start with exactly the elements
/// the request created and did not delete again, in the order of their
/// creation, followed only by existing elements, and that no element is
/// written twice. A request that succeeds deleted exactly the element each
/// deleted key created.
fn check_elements_written_as_created(
    snapshot: &Snapshot,
    operations: &[Operation],
    applied: &Applied,
) -> Result<(), TestCaseError> {
    let deleted = deleted_keys(operations);
    let created: Vec<&ElementId> = applied
        .created
        .iter()
        .filter(|(key, _)| !deleted.contains(key))
        .filter_map(|(_, created)| match created {
            Created::Element(id) => Some(id),
            Created::Board(_) => None,
        })
        .collect();
    let written: Vec<&ElementId> = applied
        .writes
        .iter()
        .filter_map(|write| match write {
            Write::Element(element) => Some(element.id()),
            Write::Board(_) | Write::RemoveElement(_) => None,
        })
        .collect();
    let (new, changed) = written.split_at(created.len().min(written.len()));
    prop_assert_eq!(new, created.as_slice());
    for id in changed {
        prop_assert!(
            snapshot.elements.contains_key(*id),
            "{id} written after the new elements is no existing element"
        );
    }
    let unique: BTreeSet<&&ElementId> = written.iter().collect();
    prop_assert_eq!(unique.len(), written.len(), "an element is written twice");
    Ok(())
}

/// Checks that files are removed last, only for an element whose file
/// exists before the request and that a deletion names, that no removed
/// element is also written, and that no board keeps a deleted ID.
fn check_removals(
    snapshot: &Snapshot,
    deleted: &BTreeSet<&ElementId>,
    applied: &Applied,
) -> Result<(), TestCaseError> {
    let first_removal = applied
        .writes
        .iter()
        .position(|write| matches!(write, Write::RemoveElement(_)))
        .unwrap_or(applied.writes.len());
    let (writes, removals) = applied.writes.split_at(first_removal);
    let mut removed = BTreeSet::new();
    for removal in removals {
        let Write::RemoveElement(id) = removal else {
            return Err(TestCaseError::fail("a file is written after a removal"));
        };
        prop_assert!(
            snapshot.elements.contains_key(id),
            "{id} has no file to remove"
        );
        prop_assert!(
            deleted.contains(id),
            "{id} removed though no deletion names it"
        );
        prop_assert!(removed.insert(id), "{id} removed twice");
    }
    let mut written_boards = BTreeSet::new();
    for write in writes {
        match write {
            Write::Element(element) => {
                prop_assert!(
                    !removed.contains(element.id()),
                    "{} written and removed",
                    element.id()
                );
            }
            Write::Board(board) => {
                written_boards.insert(&board.id);
                for id in deleted {
                    prop_assert!(
                        !board.placements.contains_key(*id),
                        "board {} keeps {id}",
                        board.id
                    );
                }
            }
            Write::RemoveElement(_) => {}
        }
    }
    for (board_id, board) in &snapshot.boards {
        if !written_boards.contains(board_id) {
            for id in deleted {
                prop_assert!(
                    !board.placements.contains_key(*id),
                    "board {board_id} keeps {id}"
                );
            }
        }
    }
    Ok(())
}

/// Checks every rule a successful result must keep. An existing element
/// file changes only through an operation that names that element.
fn check(
    snapshot: &Snapshot,
    operations: &[Operation],
    applied: &Applied,
) -> Result<(), TestCaseError> {
    let named = named_existing_elements(operations);
    let deleted = deleted_existing_elements(operations);
    let existing_element_ids: BTreeSet<&ElementId> = snapshot
        .elements
        .keys()
        .chain(
            snapshot
                .boards
                .values()
                .flat_map(|board| board.placements.keys()),
        )
        .collect();
    let existing_board_ids: BTreeSet<&BoardId> = snapshot.boards.keys().collect();
    let mut created_elements = BTreeSet::new();
    let mut created_boards = BTreeSet::new();
    for (key, created) in &applied.created {
        match created {
            Created::Element(id) => {
                prop_assert!(
                    !existing_element_ids.contains(id),
                    "{key} reused element {id}"
                );
                prop_assert!(created_elements.insert(id), "{key} got element {id} twice");
            }
            Created::Board(id) => {
                prop_assert!(!existing_board_ids.contains(id), "{key} reused board {id}");
                prop_assert!(created_boards.insert(id), "{key} got board {id} twice");
            }
        }
    }
    check_elements_written_as_created(snapshot, operations, applied)?;
    check_removals(snapshot, &deleted, applied)?;
    let mut seen_board = false;
    let mut written_boards = BTreeSet::new();
    for write in &applied.writes {
        match write {
            Write::RemoveElement(_) => {}
            Write::Element(element) => {
                prop_assert!(!seen_board, "an element follows a board");
                prop_assert!(
                    !existing_element_ids.contains(element.id()) || named.contains(element.id()),
                    "existing element {} changed though no operation names it",
                    element.id()
                );
                prop_assert!(element.id().as_str().len() <= 64);
                prop_assert_eq!(
                    Element::from_markdown(element.id(), &element.to_markdown()),
                    Ok(element.clone())
                );
            }
            Write::Board(board) => {
                seen_board = true;
                prop_assert!(
                    written_boards.insert(board.id.clone()),
                    "board {} written twice",
                    board.id
                );
                prop_assert_eq!(
                    Board::from_json(&board.id, &board.to_json()),
                    Ok(board.clone())
                );
                let before = snapshot.boards.get(&board.id);
                if let Some(before) = before {
                    prop_assert_eq!(&board.title, &before.title);
                    for (id, rect) in &before.placements {
                        // A placement disappears only through delete_element.
                        let expected = (!deleted.contains(id)).then_some(rect);
                        prop_assert_eq!(board.placements.get(id), expected);
                    }
                }
                let old: BTreeMap<&ElementId, &Rect> = before
                    .map(|before| before.placements.iter().collect())
                    .unwrap_or_default();
                for (id, rect) in &board.placements {
                    if old.contains_key(id) {
                        continue;
                    }
                    for (other_id, other) in &board.placements {
                        prop_assert!(
                            other_id == id || !overlaps(rect, other),
                            "{id} overlaps {other_id}"
                        );
                    }
                }
            }
        }
    }
    Ok(())
}

proptest! {
    #[test]
    fn writes_only_named_elements_and_readable_files(
        snapshot in snapshot(),
        operations in prop::collection::vec(operation(), 0..12),
    ) {
        if let Ok(applied) = apply(&snapshot, &operations) {
            check(&snapshot, &operations, &applied)?;
        }
    }

    #[test]
    fn writes_readable_files_for_arbitrary_titles_names_and_bodies(
        snapshot in snapshot(),
        operations in well_formed_request(),
    ) {
        let applied = apply(&snapshot, &operations);

        prop_assert!(applied.is_ok(), "{applied:?}");
        if let Ok(applied) = applied {
            prop_assert_eq!(applied.created.len(), operations.len().div_ceil(2));
            check(&snapshot, &operations, &applied)?;
        }
    }
}
