//! Properties of `apply` that hold for any snapshot and any request.

#![expect(
    clippy::disallowed_fields,
    reason = "the tests check the fields as the board file stores them"
)]
use std::collections::{BTreeMap, BTreeSet};
use std::num::NonZeroU32;

use abyme_core::{
    Applied, Created, LocalKey, Operation, PinConfirmed, Ref, Snapshot, Write, apply,
};
use abyme_model::{
    Anchor, Board, BoardId, CommitId, Element, ElementFields, ElementId, Mode, NOTE_TYPE,
    Placement, Rect, RelationKind, Relations, StatementKind, TypeRef, Verification, Verifier, Zone,
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

/// Any zone, every mode and kind among them.
fn zone() -> impl Strategy<Value = Zone> {
    (
        prop::sample::select(Mode::ALL.as_slice()),
        prop::sample::select(StatementKind::ALL.as_slice()),
    )
        .prop_map(|(mode, statement)| Zone { mode, statement })
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

/// An element with the given ID, fixed content, the anchor `crates/<id>`
/// with a stamp, and the given relations except one to itself.
fn element(id: ElementId, mut relations: Relations) -> Option<Element> {
    let type_ref = TypeRef::try_from("t".to_owned()).ok()?;
    let anchor = Anchor::try_from(format!("crates/{id}")).ok()?;
    let commit = CommitId::try_from("a".repeat(40)).ok()?;
    relations.retain(|(_, to), _| *to != id);
    Element::new(
        id,
        ElementFields {
            statement: None,
            type_ref,
            name: "name".to_owned(),
            summary: None,
            body: String::new(),
            relations,
            anchor: Some(anchor),
            verified: Some(Verification {
                commit,
                by: Verifier::Person,
            }),
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
        prop::collection::btree_map(
            element_id(),
            (rect(), any::<bool>()).prop_map(|(rect, pinned)| Placement {
                rect,
                pinned,
                attached_to: None,
            }),
            0..4,
        ),
    )
        .prop_map(|(id, title, placements)| Board {
            id,
            title,
            owner: None,
            summary: None,
            zone: None,
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

/// Anchors that equal a snapshot element's, differ from it only in case, or
/// are new, mixed with arbitrary text.
fn anchor() -> impl Strategy<Value = String> {
    prop_oneof![
        prop::sample::select(vec![
            "crates/orders-db",
            "Crates/orders-db",
            "crates/checkout",
            "docs",
            "../x",
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
            any::<String>(),
            proptest::option::of(anchor()),
            proptest::option::of(prop::sample::select(StatementKind::ALL.as_slice())),
        )
            .prop_map(|(key, type_ref, name, summary, body, anchor, statement)| {
                Operation::CreateElement {
                    key,
                    type_ref,
                    name,
                    summary,
                    body,
                    anchor,
                    statement,
                }
            }),
        (
            key(),
            name(),
            proptest::option::of(element_ref()),
            proptest::option::of(any::<String>()),
            proptest::option::of(zone()),
        )
            .prop_map(
                |(key, title, owner, summary, zone)| Operation::CreateBoard {
                    key,
                    title,
                    owner,
                    summary,
                    zone,
                }
            ),
        (
            board_ref(),
            proptest::option::of(name()),
            proptest::option::of(proptest::option::of(any::<String>())),
            proptest::option::of(proptest::option::of(element_ref())),
            proptest::option::of(proptest::option::of(zone())),
        )
            .prop_map(
                |(board, title, summary, owner, zone)| Operation::UpdateBoard {
                    board,
                    title,
                    summary,
                    owner,
                    zone,
                },
            ),
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
            proptest::option::of(proptest::option::of(anchor())),
            proptest::option::of(proptest::option::of(prop::sample::select(
                StatementKind::ALL.as_slice(),
            ))),
        )
            .prop_map(
                |(element, name, type_ref, summary, body, anchor, statement)| {
                    Operation::UpdateElement {
                        element,
                        name,
                        type_ref,
                        summary,
                        body,
                        anchor,
                        statement,
                    }
                }
            ),
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
            | Operation::UpdateBoard { .. }
            | Operation::PlaceElement { .. }
            | Operation::MoveElement { .. }
            | Operation::LayoutBoard { .. }
            | Operation::AttachNote { .. }
            | Operation::DetachNote { .. }
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
            }
            | Operation::AttachNote {
                note: Ref::Existing(id),
                ..
            } => Some(id),
            Operation::CreateElement { .. }
            | Operation::CreateBoard { .. }
            | Operation::UpdateBoard { .. }
            | Operation::PlaceElement { .. }
            | Operation::MoveElement { .. }
            | Operation::LayoutBoard { .. }
            | Operation::AttachNote { .. }
            | Operation::DetachNote { .. }
            | Operation::UpdateElement { .. }
            | Operation::DeleteElement { .. }
            | Operation::CreateRelation { .. }
            | Operation::DeleteRelation { .. } => None,
        })
        .collect()
}

/// The existing boards that a request's operations update.
fn updated_existing_boards(operations: &[Operation]) -> BTreeSet<&BoardId> {
    operations
        .iter()
        .filter_map(|operation| match operation {
            Operation::UpdateBoard {
                board: Ref::Existing(id),
                ..
            } => Some(id),
            Operation::CreateElement { .. }
            | Operation::CreateBoard { .. }
            | Operation::UpdateBoard { .. }
            | Operation::PlaceElement { .. }
            | Operation::MoveElement { .. }
            | Operation::LayoutBoard { .. }
            | Operation::AttachNote { .. }
            | Operation::DetachNote { .. }
            | Operation::UpdateElement { .. }
            | Operation::DeleteElement { .. }
            | Operation::CreateRelation { .. }
            | Operation::DeleteRelation { .. } => None,
        })
        .collect()
}

/// Checks that, after a successful request, no element owns two boards,
/// every owner is an element, and no board lies inside itself. The
/// snapshot's boards have no owner, so every breach would be one the
/// request brought in.
fn check_board_hierarchy(snapshot: &Snapshot, applied: &Applied) -> Result<(), TestCaseError> {
    let mut boards: BTreeMap<&BoardId, &Board> = snapshot.boards.iter().collect();
    let mut written = BTreeSet::new();
    let mut removed = BTreeSet::new();
    for write in &applied.writes {
        match write {
            Write::Board(board) => {
                boards.insert(&board.id, board);
            }
            Write::Element(element) => {
                written.insert(element.id());
            }
            Write::RemoveElement(id) => {
                removed.insert(id);
            }
        }
    }
    let mut owned = BTreeSet::new();
    for (id, board) in &boards {
        if let Some(owner) = &board.owner {
            prop_assert!(owned.insert(owner), "{owner} owns two boards");
            prop_assert!(
                written.contains(owner)
                    || (snapshot.elements.contains_key(owner) && !removed.contains(owner)),
                "owner {owner} of board {id} is no element"
            );
        }
    }
    let home = |element: &ElementId| {
        boards
            .iter()
            .filter(|(_, board)| board.placements.contains_key(element))
            .map(|(id, _)| *id)
            .min()
    };
    for start in boards.keys() {
        let mut seen = BTreeSet::new();
        let mut current = *start;
        while let Some(parent) = boards
            .get(current)
            .and_then(|board| board.owner.as_ref())
            .and_then(home)
        {
            prop_assert!(parent != *start, "board {start} lies inside itself");
            if !seen.insert(parent) {
                break;
            }
            current = parent;
        }
    }
    Ok(())
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
                owner: None,
                summary: None,
                zone: None,
            }];
            for (index, (name, summary, body)) in elements.into_iter().enumerate() {
                let key = LocalKey::try_from(format!("e{index}")).ok()?;
                operations.push(Operation::CreateElement {
                    statement: None,
                    key: key.clone(),
                    type_ref: "c4/container".to_owned(),
                    name,
                    summary,
                    body,
                    anchor: None,
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
            | Operation::UpdateBoard { .. }
            | Operation::PlaceElement { .. }
            | Operation::MoveElement { .. }
            | Operation::LayoutBoard { .. }
            | Operation::AttachNote { .. }
            | Operation::DetachNote { .. }
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
/// file changes only through an operation that names that element, and no
/// element file it writes carries a stamp.
fn check(
    snapshot: &Snapshot,
    operations: &[Operation],
    applied: &Applied,
) -> Result<(), TestCaseError> {
    let named = named_existing_elements(operations);
    let deleted = deleted_existing_elements(operations);
    let updated_boards = updated_existing_boards(operations);
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
    check_board_hierarchy(snapshot, applied)?;
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
                prop_assert_eq!(
                    element.verified(),
                    None,
                    "{} is written with a stamp",
                    element.id()
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
                    if !updated_boards.contains(&board.id) {
                        prop_assert_eq!(&board.title, &before.title);
                        prop_assert_eq!(&board.owner, &before.owner);
                        prop_assert_eq!(&board.summary, &before.summary);
                        prop_assert_eq!(&board.zone, &before.zone);
                    }
                    for (id, placement) in &before.placements {
                        // A placement disappears only through delete_element.
                        let expected = (!deleted.contains(id)).then_some(placement);
                        prop_assert_eq!(board.placements.get(id), expected);
                    }
                }
                let old: BTreeMap<&ElementId, &Placement> = before
                    .map(|before| before.placements.iter().collect())
                    .unwrap_or_default();
                for (id, placement) in &board.placements {
                    if old.contains_key(id) {
                        continue;
                    }
                    for (other_id, other) in &board.placements {
                        prop_assert!(
                            other_id == id || !placement.rect.overlaps(&other.rect),
                            "{id} overlaps {other_id}"
                        );
                    }
                }
            }
        }
    }
    Ok(())
}

/// A board `main` placing `first` and `second`, pinned as given, and their
/// element files.
fn board_with_pins(
    main: BoardId,
    first: (ElementId, bool),
    second: (ElementId, bool),
) -> Option<Snapshot> {
    let elements = [&first.0, &second.0]
        .into_iter()
        .map(|id| Some((id.clone(), element(id.clone(), Relations::new())?)))
        .collect::<Option<BTreeMap<_, _>>>()?;
    let placement = |pinned| Placement {
        rect: Rect {
            x: 0,
            y: 0,
            width: NonZeroU32::MIN,
            height: NonZeroU32::MIN,
        },
        pinned,
        attached_to: None,
    };
    let board = Board {
        id: main,
        title: "Main".to_owned(),
        owner: None,
        summary: None,
        zone: None,
        placements: BTreeMap::from([
            (first.0, placement(first.1)),
            (second.0, placement(second.1)),
        ]),
    };
    Some(Snapshot {
        elements,
        boards: BTreeMap::from([(board.id.clone(), board)]),
    })
}

proptest! {
    #[test]
    fn a_layout_keeps_pins_overlaps_nothing_and_changes_no_byte_when_repeated(
        snapshot in snapshot(),
        operations in prop::collection::vec(operation(), 0..6),
        board in prop::sample::select(BOARD_IDS.to_vec()),
    ) {
        let id: BoardId = board.parse().unwrap();
        let layout = Operation::LayoutBoard {
            board: Ref::Existing(id.clone()),
        };
        let once: Vec<Operation> = operations.into_iter().chain([layout.clone()]).collect();
        let twice: Vec<Operation> = once.iter().cloned().chain([layout]).collect();

        if let (Ok(first), Ok(second)) = (apply(&snapshot, &once), apply(&snapshot, &twice)) {
            let laid_out = |applied: &Applied| {
                applied.writes.iter().find_map(|write| match write {
                    Write::Board(board) if board.id == id => Some(board.clone()),
                    Write::Board(_) | Write::Element(_) | Write::RemoveElement(_) => None,
                })
            };
            let (board, repeated) = (laid_out(&first).unwrap(), laid_out(&second).unwrap());
            prop_assert_eq!(board.to_json(), repeated.to_json());
            let before = &snapshot.boards[&id].placements;
            for (element, placement) in &board.placements {
                if placement.pinned {
                    prop_assert_eq!(before.get(element), Some(placement));
                    continue;
                }
                for (other_id, other) in &board.placements {
                    prop_assert!(
                        other_id == element || !placement.rect.overlaps(&other.rect),
                        "{element} overlaps {other_id}"
                    );
                }
            }
        }
    }

    #[test]
    fn a_move_changes_a_pin_only_by_saying_so_and_a_pinned_box_only_with_a_pin(
        pins in (any::<bool>(), any::<bool>()),
        moves in prop::collection::vec(
            (
                prop::sample::select(vec!["orders-db", "checkout"]),
                -1000..1000_i32,
                proptest::option::of(any::<bool>()),
            ),
            0..8,
        ),
    ) {
        let main: BoardId = "main".parse().unwrap();
        let before = board_with_pins(
            main.clone(),
            ("orders-db".parse().unwrap(), pins.0),
            ("checkout".parse().unwrap(), pins.1),
        )
        .unwrap();
        let operations: Vec<Operation> = moves
            .iter()
            .map(|(element, x, pinned)| Operation::MoveElement {
                element: Ref::Existing(element.parse().unwrap()),
                board: Ref::Existing(main.clone()),
                x: *x,
                y: 0,
                pinned: *pinned,
            })
            .collect();

        // Replays the rule on the placements: the request is refused at the
        // first plain move of an element pinned at that point.
        let mut expected = before.boards[&main].placements.clone();
        let mut refused = false;
        for (element, x, pinned) in &moves {
            let id: ElementId = element.parse().unwrap();
            let placement = expected.get_mut(&id).unwrap();
            if placement.pinned && pinned.is_none() {
                refused = true;
                break;
            }
            placement.rect.x = *x;
            placement.pinned = pinned.unwrap_or(placement.pinned);
        }

        let applied = apply(&before, &operations);

        prop_assert_eq!(applied.is_err(), refused);
        if let Ok(applied) = applied {
            let written = applied.writes.iter().find_map(|write| match write {
                Write::Board(board) => Some(&board.placements),
                Write::Element(_) | Write::RemoveElement(_) => None,
            });
            prop_assert_eq!(written.unwrap_or_else(|| &before.boards[&main].placements), &expected);
        }
    }

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

/// The elements of the board with attached notes: boxes and notes alike, all
/// with files, and `nowhere`, which no board places.
const ATTACHED_IDS: [&str; 5] = ["a", "b", "c", "d", "nowhere"];

/// A position near the origin, at the edge of the range of a move, or any.
fn position() -> impl Strategy<Value = i32> {
    prop_oneof![
        4 => -500..2_000_i32,
        1 => prop::sample::select(vec![
            -1_000_001, -1_000_000, 999_999, 1_000_000, 1_000_001
        ]),
        1 => any::<i32>(),
    ]
}

/// One placement of the board with attached notes: mostly hanging on another
/// element, sometimes on itself, on `nowhere`, or on nothing.
fn attached_placement() -> impl Strategy<Value = Placement> {
    let named = proptest::option::weighted(
        0.7,
        prop::sample::select(ATTACHED_IDS.to_vec())
            .prop_filter_map("valid ID", |id| id.parse::<ElementId>().ok()),
    );
    (position(), position(), any::<bool>(), named).prop_map(|(x, y, pinned, attached_to)| {
        Placement {
            rect: Rect {
                x,
                y,
                width: NonZeroU32::new(180).unwrap_or(NonZeroU32::MIN),
                height: NonZeroU32::new(90).unwrap_or(NonZeroU32::MIN),
            },
            pinned,
            attached_to,
        }
    })
}

/// A snapshot whose board `main` places some of `a` to `d`, many of them
/// hanging on another, in chains and cycles among them, and every one of them
/// with a file.
fn attached_snapshot() -> impl Strategy<Value = Snapshot> {
    prop::collection::btree_map(
        prop::sample::select(ATTACHED_IDS[..4].to_vec())
            .prop_filter_map("valid ID", |id| id.parse::<ElementId>().ok()),
        attached_placement(),
        1..=4,
    )
    .prop_map(|placements| {
        // `c` and `d` are notes, `a` and `b` boxes.
        let elements = ATTACHED_IDS[..4]
            .iter()
            .filter_map(|id| {
                let id: ElementId = id.parse().ok()?;
                let mut fields = element(id.clone(), Relations::new())?.into_fields();
                if matches!(id.as_str(), "c" | "d") {
                    fields.type_ref = TypeRef::try_from(NOTE_TYPE.to_owned()).ok()?;
                }
                Some((id.clone(), Element::new(id, fields).ok()?))
            })
            .collect();
        let main: BoardId = "main".parse().unwrap_or_else(|_| unreachable!());
        let board = Board {
            id: main.clone(),
            title: "Main".to_owned(),
            owner: None,
            summary: None,
            zone: None,
            placements,
        };
        Snapshot {
            elements,
            boards: BTreeMap::from([(main, board)]),
        }
    })
}

/// A move, a layout, or a deletion on the board with attached notes.
fn attached_operation() -> impl Strategy<Value = Operation> {
    let id = prop::sample::select(ATTACHED_IDS.to_vec())
        .prop_filter_map("valid ID", |id| id.parse::<ElementId>().ok());
    let main = || Ref::Existing("main".parse().unwrap_or_else(|_| unreachable!()));
    prop_oneof![
        3 => (
            id.clone(),
            position(),
            position(),
            proptest::option::of(any::<bool>()),
        )
            .prop_map(move |(element, x, y, pinned)| Operation::MoveElement {
                element: Ref::Existing(element),
                board: main(),
                x,
                y,
                pinned,
            }),
        1 => Just(Operation::LayoutBoard { board: main() }),
        1 => id.clone().prop_map(|element| Operation::DeleteElement {
            element: Ref::Existing(element),
        }),
        2 => (id.clone(), id.clone(), any::<bool>()).prop_map(move |(note, to, pinned)| {
            Operation::AttachNote {
                note: Ref::Existing(note),
                to: Ref::Existing(to),
                board: main(),
                pinned: pinned.then_some(PinConfirmed),
            }
        }),
        1 => id.prop_map(move |note| Operation::DetachNote {
            note: Ref::Existing(note),
            board: main(),
        }),
    ]
}

/// The notes a request attaches or detaches.
fn notes_named(operations: &[Operation]) -> BTreeSet<ElementId> {
    operations
        .iter()
        .filter_map(|operation| match operation {
            Operation::AttachNote {
                note: Ref::Existing(note),
                ..
            }
            | Operation::DetachNote {
                note: Ref::Existing(note),
                ..
            } => Some(note.clone()),
            Operation::CreateElement { .. }
            | Operation::CreateBoard { .. }
            | Operation::UpdateBoard { .. }
            | Operation::PlaceElement { .. }
            | Operation::MoveElement { .. }
            | Operation::AttachNote { .. }
            | Operation::DetachNote { .. }
            | Operation::LayoutBoard { .. }
            | Operation::UpdateElement { .. }
            | Operation::DeleteElement { .. }
            | Operation::CreateRelation { .. }
            | Operation::DeleteRelation { .. } => None,
        })
        .collect()
}

/// The element `id` as `applied` writes it, or as `snapshot` has it.
fn element_after<'a>(
    snapshot: &'a Snapshot,
    applied: &'a Applied,
    id: &ElementId,
) -> Option<&'a Element> {
    applied
        .writes
        .iter()
        .find_map(|write| match write {
            Write::Element(element) if element.id() == id => Some(element),
            Write::Element(_) | Write::Board(_) | Write::RemoveElement(_) => None,
        })
        .or_else(|| snapshot.elements.get(id))
}

/// The board `main` as `applied` writes it, or as `snapshot` has it.
fn main_after(snapshot: &Snapshot, applied: &Applied) -> Board {
    applied
        .writes
        .iter()
        .find_map(|write| match write {
            Write::Board(board) if board.id.as_str() == "main" => Some(board.clone()),
            Write::Board(_) | Write::Element(_) | Write::RemoveElement(_) => None,
        })
        .unwrap_or_else(|| {
            snapshot
                .boards
                .values()
                .next()
                .cloned()
                .unwrap_or_else(|| unreachable!())
        })
}

/// Each element of `board` with the box it hangs on.
fn holding(board: &Board) -> Vec<(ElementId, ElementId)> {
    board
        .placements
        .keys()
        .filter_map(|id| board.attachment(id).map(|host| (id.clone(), host.clone())))
        .collect()
}

/// Checks that each attachment of `board` that holds puts its element at the
/// box's position plus the stored offset, within the range of a move.
fn check_attachments(board: &Board) -> Result<(), TestCaseError> {
    for (element, host) in holding(board) {
        let (rect, host_rect) = (board.rect_of(&element), board.rect_of(&host));
        let (Some(rect), Some(host_rect)) = (rect, host_rect) else {
            return Err(TestCaseError::fail(format!(
                "{element} or {host} is not placed"
            )));
        };
        let Some(stored) = board
            .placements
            .get(&element)
            .map(|placement| placement.rect)
        else {
            return Err(TestCaseError::fail(format!("{element} has no placement")));
        };
        prop_assert_eq!(
            (i64::from(rect.x), i64::from(rect.y)),
            (
                i64::from(host_rect.x) + i64::from(stored.x),
                i64::from(host_rect.y) + i64::from(stored.y)
            ),
            "{} on {}",
            element,
            host
        );
        prop_assert_eq!((rect.width, rect.height), (stored.width, stored.height));
        for value in [rect.x, rect.y] {
            prop_assert!(
                (-1_000_000..=1_000_000).contains(&value),
                "{element}: {value}"
            );
        }
    }
    Ok(())
}

proptest! {
    #[test]
    fn attached_notes_stay_on_their_boxes_and_within_range_after_any_request(
        snapshot in attached_snapshot(),
        operations in prop::collection::vec(attached_operation(), 0..6),
        last in attached_operation(),
    ) {
        let request: Vec<Operation> = operations.into_iter().chain([last.clone()]).collect();

        if let Ok(applied) = apply(&snapshot, &request) {
            let before = &snapshot.boards.values().next().cloned().unwrap_or_else(|| unreachable!());
            let after = main_after(&snapshot, &applied);
            check_attachments(&after)?;
            let named = notes_named(&request);
            for (element, host) in holding(before) {
                if named.contains(&element) {
                    continue;
                }
                if after.placements.contains_key(&element) && after.placements.contains_key(&host) {
                    prop_assert_eq!(after.attachment(&element), Some(&host), "{} came off", element);
                }
            }
            let prepared = apply(&snapshot, &request[..request.len() - 1])
                .map_or_else(|_| unreachable!(), |prepared| main_after(&snapshot, &prepared));
            match last {
                Operation::MoveElement { element: Ref::Existing(element), x, y, .. } => {
                    let rect = after.rect_of(&element).unwrap_or_else(|| unreachable!());
                    prop_assert_eq!((rect.x, rect.y), (x, y));
                }
                Operation::AttachNote { note: Ref::Existing(note), to: Ref::Existing(to), .. } => {
                    prop_assert_eq!(after.attachment(&note), Some(&to));
                    prop_assert_eq!(after.rect_of(&note), prepared.rect_of(&note));
                    prop_assert_eq!(after.hanging_on(&note), Vec::<ElementId>::new());
                    let relations = element_after(&snapshot, &applied, &note)
                        .map(|element| element.relations().clone())
                        .unwrap_or_default();
                    prop_assert!(relations.contains_key(&(RelationKind::Concerns, to)));
                }
                Operation::DetachNote { note: Ref::Existing(note), .. } => {
                    prop_assert_eq!(after.attachment(&note), None);
                    prop_assert_eq!(after.rect_of(&note), prepared.rect_of(&note));
                }
                Operation::CreateElement { .. }
                | Operation::CreateBoard { .. }
                | Operation::UpdateBoard { .. }
                | Operation::PlaceElement { .. }
                | Operation::MoveElement { .. }
                | Operation::AttachNote { .. }
                | Operation::DetachNote { .. }
                | Operation::LayoutBoard { .. }
                | Operation::UpdateElement { .. }
                | Operation::DeleteElement { .. }
                | Operation::CreateRelation { .. }
                | Operation::DeleteRelation { .. } => {}
            }
        }
    }

    #[test]
    fn a_layout_with_attached_notes_keeps_pins_overlaps_nothing_and_repeats_byte_for_byte(
        snapshot in attached_snapshot(),
        operations in prop::collection::vec(attached_operation(), 0..4),
    ) {
        let layout = Operation::LayoutBoard {
            board: Ref::Existing("main".parse().unwrap_or_else(|_| unreachable!())),
        };
        let once: Vec<Operation> = operations.iter().cloned().chain([layout.clone()]).collect();
        let twice: Vec<Operation> = once.iter().cloned().chain([layout]).collect();

        if let (Ok(prepared), Ok(first), Ok(second)) =
            (apply(&snapshot, &operations), apply(&snapshot, &once), apply(&snapshot, &twice))
        {
            let (before, board, repeated) = (
                main_after(&snapshot, &prepared),
                main_after(&snapshot, &first),
                main_after(&snapshot, &second),
            );
            prop_assert_eq!(board.to_json(), repeated.to_json());
            check_attachments(&board)?;
            let line_of = |board: &Board, id: &ElementId| {
                let needle = format!(r#""element": "{id}","#);
                board.to_json().lines().find(|line| line.contains(&needle)).map(str::to_owned)
            };
            for id in before.placements.keys().filter(|id| before.named_box(id).is_some()) {
                prop_assert_eq!(line_of(&board, id), line_of(&before, id), "{} was laid out", id);
            }
            let free: Vec<(ElementId, Rect, bool)> = board
                .placed()
                .filter(|(id, _, _)| board.named_box(id).is_none())
                .map(|(id, placement, rect)| (id.clone(), rect, placement.pinned))
                .collect();
            for (element, rect, pinned) in &free {
                if *pinned {
                    prop_assert_eq!(Some(*rect), before.rect_of(element), "{} moved", element);
                    continue;
                }
                for (other, other_rect, _) in &free {
                    prop_assert!(
                        other == element || !rect.overlaps(other_rect),
                        "{element} overlaps {other}"
                    );
                }
            }
        }
    }
}
