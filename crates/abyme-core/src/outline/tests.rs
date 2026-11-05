use std::collections::{BTreeMap, BTreeSet};
use std::num::NonZeroU32;

use abyme_model::{
    Anchor, CommitId, ElementFields, ElementFixture, FORMAT_AND_SEPARATORS, Mode, Placement, Rect,
    Relations, TypeRef, Verification, Verifier,
};
use proptest::prelude::*;
use rstest::rstest;

use super::*;
use crate::review::{GitFacts, RepositoryFacts, Since, review};

mod board;
mod detail;
mod escaping;
mod geometry;
mod hierarchy;
mod kinds;
mod list;
mod overview;
mod relations;
mod review;

fn rect() -> Rect {
    Rect {
        x: 0,
        y: 0,
        width: NonZeroU32::MIN,
        height: NonZeroU32::MIN,
    }
}

fn element(id: &str, type_ref: &str, name: &str, summary: Option<&str>) -> Element {
    ElementFixture::new(id)
        .type_ref(type_ref)
        .name(name)
        .summary(summary)
        .build()
}

/// A snapshot with the board `b` titled `title` that places `elements` and
/// the IDs in `missing`, whose files do not exist.
fn snapshot(title: &str, elements: Vec<Element>, missing: &[&str]) -> Snapshot {
    let placed = elements
        .iter()
        .map(|element| element.id().clone())
        .chain(missing.iter().map(|id| id.parse().unwrap()));
    let board = Board {
        id: "b".parse().unwrap(),
        title: title.to_owned(),
        owner: None,
        summary: None,
        zone: None,
        placements: placed
            .map(|id| {
                (
                    id,
                    Placement {
                        rect: rect(),
                        pinned: false,
                        attached_to: None,
                    },
                )
            })
            .collect(),
    };
    Snapshot {
        elements: elements
            .into_iter()
            .map(|element| (element.id().clone(), element))
            .collect(),
        boards: BTreeMap::from([(board.id.clone(), board)]),
    }
}

fn outline_of(snapshot: &Snapshot) -> String {
    outline(snapshot, &"b".parse().unwrap(), Geometry::Hidden, None).unwrap()
}

fn outline_with_geometry(snapshot: &Snapshot) -> String {
    outline(snapshot, &"b".parse().unwrap(), Geometry::Shown, None).unwrap()
}

/// `element` with the given relations.
fn related(element: Element, relations: &[(RelationKind, &str, Option<&str>)]) -> Element {
    let id = element.id().clone();
    let relations = relations
        .iter()
        .map(|(kind, to, label)| ((*kind, to.parse().unwrap()), label.map(str::to_owned)))
        .collect();
    Element::new(
        id,
        ElementFields {
            relations,
            ..element.into_fields()
        },
    )
    .unwrap()
}

fn detail_of(snapshot: &Snapshot, id: &str) -> String {
    element_detail(snapshot, &id.parse().unwrap(), None).unwrap()
}

/// `element` with the given anchor and stamp.
fn anchored(element: Element, anchor: &str, stamp: Option<(&str, Verifier)>) -> Element {
    let id = element.id().clone();
    Element::new(
        id,
        ElementFields {
            anchor: Some(Anchor::try_from(anchor.to_owned()).unwrap()),
            verified: stamp.map(|(commit, by)| Verification {
                commit: CommitId::try_from(commit.to_owned()).unwrap(),
                by,
                content: None,
            }),
            ..element.into_fields()
        },
    )
    .unwrap()
}

const STAMP: &str = "0123456789abcdef0123456789abcdef01234567";

/// The snapshot with only the element `x`, anchored on `crates/x` and
/// stamped with `STAMP`.
fn stamped_snapshot() -> Snapshot {
    let mut snapshot = snapshot("Board", Vec::new(), &[]);
    let element = anchored(
        element("x", "t", "X", None),
        "crates/x",
        Some((STAMP, Verifier::Person)),
    );
    snapshot.elements.insert(element.id().clone(), element);
    snapshot
}

/// Facts in which the stamp `STAMP` is known and the index changed
/// `since` it.
fn facts(known: &[&[u8]], since: &[&[u8]]) -> GitFacts {
    let paths = |values: &[&[u8]]| values.iter().map(|value| value.to_vec()).collect();
    GitFacts {
        project: RepositoryFacts {
            known: paths(known),
            uncommitted: BTreeSet::new(),
            since: BTreeMap::from([(
                CommitId::try_from(STAMP.to_owned()).unwrap(),
                Since::Changed(paths(since)),
            )]),
            fallback: None,
        },
        nested: BTreeMap::new(),
    }
}

/// A snapshot whose board `b` places `a`, anchored on `crates/a` and
/// stamped, `b`, stamped and using `a`, and `c`, without flags, with the
/// facts in which `crates/a/lib.rs` changed since the stamp; and its review.
fn marked() -> (Snapshot, Review) {
    let a = anchored(
        element("a", "t", "A", None),
        "crates/a",
        Some((STAMP, Verifier::Ai)),
    );
    let b = anchored(
        related(
            element("b", "t", "B", Some("S")),
            &[(RelationKind::Uses, "a", None)],
        ),
        "crates/b",
        Some((STAMP, Verifier::Ai)),
    );
    let snapshot = snapshot("Board", vec![a, b, element("c", "t", "C", None)], &[]);
    let review = review(
        &snapshot,
        &facts(
            &[b"crates/a/lib.rs", b"crates/b/lib.rs"],
            &[b"crates/a/lib.rs"],
        ),
    );
    (snapshot, review)
}

/// The board `id`, titled `id` too, the inside of `owner`, that places the
/// elements `placed`.
fn hierarchy_board(id: &str, owner: Option<&str>, placed: &[&str]) -> Board {
    Board {
        id: id.parse().unwrap(),
        title: id.to_owned(),
        owner: owner.map(|owner| owner.parse().unwrap()),
        summary: None,
        zone: None,
        placements: placed
            .iter()
            .map(|element| {
                (
                    element.parse().unwrap(),
                    Placement {
                        rect: rect(),
                        pinned: false,
                        attached_to: None,
                    },
                )
            })
            .collect(),
    }
}

/// A snapshot of `boards` and of a file for each element in `elements`,
/// typed `t` and named after its ID in capitals.
fn hierarchy(boards: Vec<Board>, elements: &[&str]) -> Snapshot {
    Snapshot {
        elements: elements
            .iter()
            .map(|id| {
                let element = element(id, "t", &id.to_uppercase(), None);
                (element.id().clone(), element)
            })
            .collect(),
        boards: boards
            .into_iter()
            .map(|board| (board.id.clone(), board))
            .collect(),
    }
}

fn outline_of_board(snapshot: &Snapshot, board: &str) -> String {
    outline(snapshot, &board.parse().unwrap(), Geometry::Hidden, None).unwrap()
}

/// Boards that hand edits and merges left in every state the owner line
/// marks: owners without a file, with and without a home; an owner without
/// a home; an owner placed on two boards; a board whose owner lies on it,
/// also an owner without a file; cycles of three and of two boards; and a
/// board below a cycle.
fn damaged() -> Snapshot {
    hierarchy(
        vec![
            hierarchy_board("top", None, &["placed-gone", "twice"]),
            hierarchy_board("alpha", None, &["twice"]),
            hierarchy_board("orphan", Some("gone"), &[]),
            hierarchy_board("orphan-placed", Some("placed-gone"), &[]),
            hierarchy_board("homeless", Some("loner"), &[]),
            hierarchy_board("doubled", Some("twice"), &[]),
            hierarchy_board("itself", Some("mirror"), &["mirror"]),
            hierarchy_board("ghost-loop", Some("ghost"), &["ghost"]),
            hierarchy_board("one", Some("x"), &["z", "w"]),
            hierarchy_board("two", Some("y"), &["x"]),
            hierarchy_board("three", Some("z"), &["y"]),
            hierarchy_board("below", Some("w"), &[]),
            hierarchy_board("left", Some("r"), &["l"]),
            hierarchy_board("right", Some("l"), &["r"]),
        ],
        &["twice", "loner", "mirror", "x", "y", "z", "w", "l", "r"],
    )
}
