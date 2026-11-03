//! Tests of the layered layout.

#![expect(
    clippy::disallowed_fields,
    reason = "the tests check the fields as the board file stores them"
)]
use super::*;
use std::collections::BTreeSet;
use std::num::NonZeroU32;

use abyme_model::{ElementFields, Placement, Relations, TypeRef};
use proptest::prelude::*;
use rstest::rstest;

use RelationKind::{Concerns, DependsOn, Describes, SeeAlso, Uses};

fn id(value: &str) -> ElementId {
    value.parse().unwrap()
}

fn element(name: &str, summary: Option<&str>, relations: &[(RelationKind, &str)]) -> Element {
    Element::new(
        id(name),
        ElementFields {
            statement: None,
            type_ref: TypeRef::try_from("c4/container".to_owned()).unwrap(),
            name: name.to_owned(),
            summary: summary.map(str::to_owned),
            body: String::new(),
            relations: relations
                .iter()
                .map(|(kind, to)| ((*kind, id(to)), None))
                .collect::<Relations>(),
            anchor: None,
            verified: None,
        },
    )
    .unwrap()
}

fn rect(x: i32, y: i32, width: u32, height: u32) -> Rect {
    Rect {
        x,
        y,
        width: NonZeroU32::new(width).unwrap(),
        height: NonZeroU32::new(height).unwrap(),
    }
}

/// A board placing `free` unpinned, far from where a layout puts them, and
/// `pinned` where it says.
fn board(free: &[&str], pinned: &[(&str, Rect)]) -> Board {
    let unpinned = free.iter().map(|name| {
        (
            id(name),
            Placement {
                rect: rect(-999, -999, 7, 7),
                pinned: false,
                attached_to: None,
            },
        )
    });
    let pins = pinned.iter().map(|(name, rect)| {
        (
            id(name),
            Placement {
                rect: *rect,
                pinned: true,
                attached_to: None,
            },
        )
    });
    Board {
        id: "main".parse().unwrap(),
        title: String::new(),
        owner: None,
        summary: None,
        zone: None,
        placements: unpinned.chain(pins).collect(),
    }
}

fn elements_of(elements: Vec<Element>) -> BTreeMap<ElementId, Element> {
    elements
        .into_iter()
        .map(|element| (element.id().clone(), element))
        .collect()
}

/// The layout of `board` with `left` work, by the names of the free boxes.
fn lay_with(
    board: &Board,
    elements: &BTreeMap<ElementId, Element>,
    left: u64,
) -> Result<BTreeMap<String, Rect>, Problem> {
    let rects = layered(board, |id| elements.get(id), &mut LayoutWork { left })?;
    Ok(board
        .placements
        .keys()
        .zip(rects)
        .filter_map(|(id, rect)| rect.map(|rect| (id.as_str().to_owned(), rect)))
        .collect())
}

fn lay(board: &Board, elements: &BTreeMap<ElementId, Element>) -> BTreeMap<String, Rect> {
    lay_with(board, elements, MAX_LAYOUT_WORK).unwrap()
}

/// Boxes `names` without files' text beyond their name, each relating as given.
fn related(
    relations: &[(&str, RelationKind, &str)],
    names: &[&str],
) -> BTreeMap<ElementId, Element> {
    elements_of(
        names
            .iter()
            .map(|name| {
                let own: Vec<(RelationKind, &str)> = relations
                    .iter()
                    .filter(|(from, _, _)| from == name)
                    .map(|(_, kind, to)| (*kind, *to))
                    .collect();
                element(name, None, &own)
            })
            .collect(),
    )
}

fn entirely_above(upper: &Rect, lower: &Rect) -> bool {
    upper.bottom() <= i64::from(lower.y)
}

#[rstest]
#[case::uses(Uses)]
#[case::depends_on(DependsOn)]
#[case::describes(Describes)]
#[case::concerns(Concerns)]
fn a_relation_puts_its_source_above_its_target(#[case] kind: RelationKind) {
    // Against ID order, so the ID cannot be what decides it.
    let elements = related(&[("b", kind, "a")], &["a", "b"]);

    let rects = lay(&board(&["a", "b"], &[]), &elements);

    assert!(entirely_above(&rects["b"], &rects["a"]), "{rects:?}");
}

#[test]
fn see_also_stands_on_one_rank() {
    let elements = related(&[("a", SeeAlso, "b")], &["a", "b"]);

    let rects = lay(&board(&["a", "b"], &[]), &elements);

    assert_eq!(rects["a"].y, rects["b"].y);
    assert!(!rects["a"].overlaps(&rects["b"]));
}

#[test]
fn see_also_influences_the_order_within_a_rank() {
    // Top `t` relates to a, b, and c; the see_also of c to `x` under `a` pulls c
    // left past b.
    let elements = related(
        &[
            ("t", Uses, "a"),
            ("t", Uses, "b"),
            ("t", Uses, "c"),
            ("a", Uses, "x"),
            ("c", SeeAlso, "x"),
        ],
        &["a", "b", "c", "t", "x"],
    );

    let rects = lay(&board(&["a", "b", "c", "t", "x"], &[]), &elements);

    assert_eq!(rects["a"].y, rects["c"].y);
    assert!(rects["c"].x < rects["b"].x, "{rects:?}");
}

#[test]
fn a_relation_to_an_element_off_the_board_counts_for_nothing() {
    let elements = related(&[("a", Uses, "off")], &["a", "b", "off"]);

    let rects = lay(&board(&["a", "b"], &[]), &elements);

    assert_eq!(rects["a"], rect(0, 0, 180, 90));
    assert_eq!(rects["b"], rect(220, 0, 180, 90));
}

#[test]
fn a_walk_leaves_out_only_the_relation_that_closes_a_cycle() {
    // a → b → c → a is a cycle, and the walk from a leaves out c → a. The tree
    // below c stays ranked.
    let elements = related(
        &[
            ("a", Uses, "b"),
            ("b", Uses, "c"),
            ("c", Uses, "a"),
            ("c", Uses, "d"),
            ("d", Uses, "e"),
        ],
        &["a", "b", "c", "d", "e"],
    );

    let rects = lay(&board(&["a", "b", "c", "d", "e"], &[]), &elements);

    for (upper, lower) in [("a", "b"), ("b", "c"), ("c", "d"), ("d", "e")] {
        assert!(
            entirely_above(&rects[upper], &rects[lower]),
            "{upper} {lower}"
        );
    }
}

#[test]
fn two_relations_between_the_same_boxes_count_once_and_their_opposite_is_left_out() {
    let elements = related(
        &[("a", Uses, "b"), ("a", DependsOn, "b"), ("b", Uses, "a")],
        &["a", "b"],
    );

    let rects = lay(&board(&["a", "b"], &[]), &elements);

    assert!(entirely_above(&rects["a"], &rects["b"]));
}

#[test]
fn connected_parts_stand_side_by_side_by_their_smallest_id() {
    let elements = related(&[("a", Uses, "z"), ("c", Uses, "d")], &["a", "c", "d", "z"]);

    let rects = lay(&board(&["a", "c", "d", "z"], &[]), &elements);

    assert_eq!(rects["a"].y, 0);
    assert_eq!(rects["c"].y, 0);
    assert!(rects["a"].right() < i64::from(rects["c"].x), "{rects:?}");
}

#[test]
fn boxes_without_relations_stand_in_a_grid_below_the_parts() {
    let names = ["a", "b", "l1", "l2", "l3", "l4", "l5"];
    let elements = related(&[("a", Uses, "b")], &names);

    let rects = lay(&board(&names, &[]), &elements);

    let bottom_of_parts = rects["b"].bottom();
    assert_eq!(rects["l1"], rect(0, 260, 180, 90));
    assert!(i64::from(rects["l1"].y) > bottom_of_parts);
    assert_eq!(rects["l2"].y, rects["l1"].y);
    assert_eq!(rects["l4"], rect(660, rects["l1"].y, 180, 90));
    assert_eq!(rects["l5"], rect(0, rects["l1"].y + 130, 180, 90));
}

#[test]
fn each_row_is_centered_in_the_widest() {
    let elements = related(
        &[("top", Uses, "a"), ("top", Uses, "b"), ("top", Uses, "c")],
        &["a", "b", "c", "top"],
    );

    let rects = lay(&board(&["a", "b", "c", "top"], &[]), &elements);

    assert_eq!(rects["top"].x, rects["b"].x);
}

#[test]
fn a_box_gets_the_size_of_its_text_and_without_a_file_the_default() {
    let long = "word ".repeat(40);
    let elements = elements_of(vec![element("a", Some(&long), &[])]);

    let rects = lay(&board(&["a", "ghost"], &[]), &elements);

    assert!(rects["a"].height.get() > 90, "{rects:?}");
    assert_eq!(rects["ghost"].height.get(), 90);
    assert_eq!(rects["ghost"].width.get(), 180);
}

#[test]
fn a_row_is_as_tall_as_its_tallest_box() {
    let long = "word ".repeat(40);
    let elements = elements_of(vec![
        element("a", Some(&long), &[(Uses, "c"), (SeeAlso, "b")]),
        element("b", None, &[]),
        element("c", None, &[]),
    ]);

    let rects = lay(&board(&["a", "b", "c"], &[]), &elements);

    assert!(entirely_above(&rects["a"], &rects["c"]));
    assert_eq!(rects["b"].y, rects["a"].y);
}

#[test]
fn a_chain_through_a_pinned_box_stays_connected_and_the_pin_stays() {
    let elements = related(
        &[("a", Uses, "pin"), ("pin", Uses, "b")],
        &["a", "b", "pin"],
    );
    let far = rect(5_000, 5_000, 50, 50);

    let rects = lay(&board(&["a", "b"], &[("pin", far)]), &elements);

    assert!(entirely_above(&rects["a"], &rects["b"]), "{rects:?}");
    assert!(!rects.contains_key("pin"));
}

#[test]
fn a_box_jumps_past_a_pin_it_would_overlap() {
    let elements = related(&[], &["a", "pin"]);
    let pin = rect(0, 0, 500, 500);

    let rects = lay(&board(&["a"], &[("pin", pin)]), &elements);

    assert_eq!(rects["a"], rect(540, 0, 180, 90));
}

#[test]
fn a_box_jumps_past_overlapping_pins_as_one() {
    let elements = related(&[], &["a", "p1", "p2", "p3"]);
    let pins = [
        ("p1", rect(0, 0, 500, 500)),
        ("p2", rect(100, 0, 100, 100)),
        ("p3", rect(400, 0, 300, 90)),
    ];

    let rects = lay(&board(&["a"], &pins), &elements);

    assert_eq!(rects["a"], rect(740, 0, 180, 90));
}

#[test]
fn a_pin_far_from_the_rows_blocks_nothing() {
    let elements = related(&[], &["a", "pin"]);
    let pin = rect(i32::MAX, i32::MAX, u32::MAX, u32::MAX);

    let rects = lay(&board(&["a"], &[("pin", pin)]), &elements);

    assert_eq!(rects["a"], rect(0, 0, 180, 90));
}

#[test]
fn a_pin_across_the_whole_plane_leaves_no_room() {
    let elements = related(&[], &["a", "pin"]);
    let pin = rect(i32::MIN, i32::MIN, u32::MAX, u32::MAX);

    assert_eq!(
        lay_with(&board(&["a"], &[("pin", pin)]), &elements, MAX_LAYOUT_WORK),
        Err(Problem::OutOfRange)
    );
}

#[test]
fn a_pin_near_the_end_of_the_range_does_not_keep_a_grid_from_the_top() {
    let elements = related(&[], &["a", "b", "pin"]);
    let pin = rect(0, 999_990, 10, 10);

    let rects = lay(&board(&["a", "b"], &[("pin", pin)]), &elements);

    assert_eq!(rects["a"], rect(0, 0, 180, 90));
}

#[test]
fn a_chain_beyond_the_range_is_refused_without_overflowing_the_stack() {
    let names: Vec<String> = (0..20_000).map(|index| format!("e{index}")).collect();
    let elements: BTreeMap<ElementId, Element> = names
        .iter()
        .enumerate()
        .map(|(index, name)| {
            let next = format!("e{}", index + 1);
            let own: Vec<(RelationKind, &str)> = if index + 1 < names.len() {
                vec![(Uses, next.as_str())]
            } else {
                Vec::new()
            };
            (id(name), element(name, None, &own))
        })
        .collect();
    let borrowed: Vec<&str> = names.iter().map(String::as_str).collect();

    assert_eq!(
        lay_with(&board(&borrowed, &[]), &elements, MAX_LAYOUT_WORK),
        Err(Problem::OutOfRange)
    );
}

#[test]
fn a_chain_of_diamonds_ends_quickly() {
    // Paths through 60 diamonds double 60 times; walking them would not end.
    let names: Vec<String> = (0..60)
        .flat_map(|index| {
            [
                format!("a{index}"),
                format!("b{index}"),
                format!("c{index}"),
            ]
        })
        .collect();
    let elements: BTreeMap<ElementId, Element> = (0..60)
        .flat_map(|index| {
            let next = format!("a{}", index + 1);
            let joined: Vec<(RelationKind, &str)> = if index + 1 < 60 {
                vec![(Uses, next.as_str())]
            } else {
                Vec::new()
            };
            let (top, left, right) = (
                format!("a{index}"),
                format!("b{index}"),
                format!("c{index}"),
            );
            [
                (
                    id(&top),
                    element(&top, None, &[(Uses, &left), (Uses, &right)]),
                ),
                (id(&left), element(&left, None, &joined)),
                (id(&right), element(&right, None, &joined)),
            ]
        })
        .collect();
    let borrowed: Vec<&str> = names.iter().map(String::as_str).collect();

    let rects = lay(&board(&borrowed, &[]), &elements);

    assert!(entirely_above(&rects["a0"], &rects["a59"]));
}

#[test]
fn work_pays_for_each_box_and_each_relation_also_off_the_board() {
    let elements = related(
        &[("a", Uses, "b"), ("a", Uses, "off"), ("b", SeeAlso, "off")],
        &["a", "b", "off"],
    );
    let board = board(&["a", "b"], &[]);
    // Two boxes, their free space, and three relations.
    let cost = 2 + 2 + 3;

    assert!(lay_with(&board, &elements, cost).is_ok());
    assert_eq!(
        lay_with(&board, &elements, cost - 1),
        Err(Problem::TooLarge)
    );
}

#[test]
fn work_pays_for_the_text_it_sizes() {
    let text = " ".repeat(TEXT_BYTES_PER_WORK * 100);
    let elements = elements_of(vec![element("a", Some(&text), &[])]);
    let board = board(&["a"], &[]);
    // The box, its free space, and its text; the type and the name are shorter
    // than one unit.
    let cost = 1 + 1 + 100;

    assert!(lay_with(&board, &elements, cost).is_ok());
    assert_eq!(
        lay_with(&board, &elements, cost - 1),
        Err(Problem::TooLarge)
    );
}

#[test]
fn work_pays_for_rows_times_pins_before_the_rows_are_placed() {
    let elements = related(
        &[("a", Uses, "b"), ("b", Uses, "c")],
        &["a", "b", "c", "p1", "p2"],
    );
    let pins = [("p1", rect(5_000, 0, 1, 1)), ("p2", rect(5_000, 9, 1, 1))];
    let board = board(&["a", "b", "c"], &pins);
    // Five boxes, their free space, two relations, three rows times two pins.
    let cost = 5 + 5 + 2 + 3 * 2;

    assert!(lay_with(&board, &elements, cost).is_ok());
    assert_eq!(
        lay_with(&board, &elements, cost - 1),
        Err(Problem::TooLarge)
    );
}

#[test]
fn a_layout_of_the_result_gives_the_same_result() {
    let names = ["a", "b", "c", "d", "l"];
    let elements = related(
        &[
            ("a", Uses, "b"),
            ("b", Uses, "c"),
            ("c", Uses, "a"),
            ("a", SeeAlso, "d"),
        ],
        &names,
    );
    let mut board = board(&names, &[]);

    let first = lay(&board, &elements);
    for (name, rect) in &first {
        if let Some(placement) = board.placements.get_mut(&id(name)) {
            placement.rect = *rect;
        }
    }

    assert_eq!(lay(&board, &elements), first);
}

/// A relation between two of the boxes, as positions among them.
fn edge() -> impl Strategy<Value = (usize, usize, RelationKind)> {
    (
        0..12_usize,
        0..12_usize,
        prop::sample::select(vec![Uses, DependsOn, Describes, Concerns, SeeAlso]),
    )
}

/// A pinned rectangle, at times one of the extremes.
fn pin_rect() -> impl Strategy<Value = Rect> {
    prop_oneof![
        8 => (-300..3_000_i32, -300..3_000_i32, 1..900_u32, 1..900_u32)
            .prop_map(|(x, y, width, height)| rect(x, y, width, height)),
        1 => (any::<i32>(), any::<i32>(), any::<u32>(), any::<u32>()).prop_map(
            |(x, y, width, height)| rect(x, y, width.max(1), height.max(1))
        ),
    ]
}

proptest! {
    #[test]
    fn no_box_overlaps_or_leaves_the_range_and_a_relation_holds_unless_left_out(
        edges in prop::collection::vec(edge(), 0..24),
        pin_of in prop::collection::vec(proptest::option::of(pin_rect()), 12),
    ) {
        let names: Vec<String> = (0..12).map(|index| format!("e{index:02}")).collect();
        let elements: BTreeMap<ElementId, Element> = names
            .iter()
            .enumerate()
            .map(|(index, name)| {
                let own: Vec<(RelationKind, &str)> = edges
                    .iter()
                    .filter(|(from, to, _)| *from == index && from != to)
                    .filter_map(|(_, to, kind)| {
                        names.get(*to).map(|to| (*kind, to.as_str()))
                    })
                    .collect();
                (id(name), element(name, None, &own))
            })
            .collect();
        let free: Vec<&str> = names
            .iter()
            .zip(&pin_of)
            .filter(|(_, pin)| pin.is_none())
            .map(|(name, _)| name.as_str())
            .collect();
        let pins: Vec<(&str, Rect)> = names
            .iter()
            .zip(&pin_of)
            .filter_map(|(name, pin)| pin.map(|pin| (name.as_str(), pin)))
            .collect();
        let board = board(&free, &pins);

        let Ok(rects) = lay_with(&board, &elements, MAX_LAYOUT_WORK) else {
            // A huge pin can leave a box no room, which is a refusal, no panic.
            return Ok(());
        };

        let placed: Vec<&Rect> = rects.values().collect();
        for (index, rect) in placed.iter().enumerate() {
            prop_assert!(i64::from(rect.x) >= -1_000_000);
            prop_assert!(i64::from(rect.y) >= -1_000_000);
            prop_assert!(rect.right() <= 1_000_000);
            prop_assert!(rect.bottom() <= 1_000_000);
            for (_, pin) in &pins {
                prop_assert!(!rect.overlaps(pin), "{rect:?} overlaps the pin {pin:?}");
            }
            for other in placed.iter().skip(index + 1) {
                prop_assert!(!rect.overlaps(other), "{rect:?} overlaps {other:?}");
            }
        }
        prop_assert_eq!(rects.len(), free.len());

        // Every relation the walk keeps holds.
        let above: Vec<Vec<usize>> = (0..12)
            .map(|from| {
                let targets: BTreeSet<usize> = edges
                    .iter()
                    .filter(|(source, to, kind)| {
                        *source == from && source != to && *kind != SeeAlso
                    })
                    .map(|(_, to, _)| *to)
                    .collect();
                targets.into_iter().collect()
            })
            .collect();
        let finished = finishing_order(&above);
        let finished_at = |node: usize| finished.iter().position(|other| *other == node);
        for (from, targets) in above.iter().enumerate() {
            for to in targets {
                let kept = finished_at(*to) < finished_at(from);
                let (Some(upper), Some(lower)) = (
                    names.get(from).and_then(|name| rects.get(name)),
                    names.get(*to).and_then(|name| rects.get(name)),
                ) else {
                    continue;
                };
                if kept {
                    prop_assert!(entirely_above(upper, lower), "{from} above {to}");
                }
            }
        }

        // Repeating the layout from the result changes nothing.
        let mut again = board;
        for (name, rect) in &rects {
            if let Some(placement) = again.placements.get_mut(&id(name)) {
                placement.rect = *rect;
            }
        }
        prop_assert_eq!(lay(&again, &elements), rects);
    }
}
