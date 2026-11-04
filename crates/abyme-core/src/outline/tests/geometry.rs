//! Tests of positions, sizes, pins, and attached notes in an outline.

use super::*;

fn sized(x: i32, y: i32, width: u32, height: u32) -> Rect {
    Rect {
        x,
        y,
        width: NonZeroU32::new(width).unwrap(),
        height: NonZeroU32::new(height).unwrap(),
    }
}

/// `snapshot` with the placement of `id` on board `b` at `rect`.
fn with_rect(mut snapshot: Snapshot, id: &str, rect: Rect) -> Snapshot {
    snapshot
        .boards
        .get_mut(&"b".parse().unwrap())
        .unwrap()
        .placements
        .insert(
            id.parse().unwrap(),
            Placement {
                rect,
                pinned: false,
                attached_to: None,
            },
        );
    snapshot
}

#[test]
fn shows_position_and_size_after_the_id_on_request() {
    let snapshot = snapshot(
        "T",
        vec![
            element("a", "t", "A", Some("first")),
            element("b", "t", "B", None),
        ],
        &["c"],
    );
    let snapshot = with_rect(snapshot, "a", sized(-40, 120, 180, 90));
    let snapshot = with_rect(snapshot, "b", sized(400, -7, 300, 50));
    let snapshot = with_rect(snapshot, "c", sized(10, 20, 1, 1));

    assert_eq!(
        outline_with_geometry(&snapshot),
        "Board: b · T\n\
         \n\
         a  @-40,120 180x90  t  A  first\n\
         b  @400,-7 300x50   t  B\n\
         c  @10,20 1x1       ⚠ missing\n"
    );
}

#[test]
fn shows_the_extreme_positions_and_sizes_a_board_file_holds() {
    let snapshot = snapshot("T", vec![element("a", "t", "A", None)], &[]);
    let snapshot = with_rect(snapshot, "a", sized(i32::MIN, i32::MAX, u32::MAX, u32::MAX));

    assert_eq!(
        outline_with_geometry(&snapshot),
        "Board: b · T\n\na  @-2147483648,2147483647 4294967295x4294967295  t  A\n"
    );
}

#[test]
fn shows_the_geometry_before_any_text_from_the_repository() {
    let snapshot = snapshot(
        "T",
        vec![element("a", "t", "@1,2 3x4\nZ\u{202e}", None)],
        &[],
    );
    let snapshot = with_rect(snapshot, "a", sized(5, 6, 7, 8));

    assert_eq!(
        outline_with_geometry(&snapshot),
        "Board: b · T\n\na  @5,6 7x8  t  @1,2 3x4\\nZ\\u{202e}\n"
    );
}

#[test]
fn shows_a_pin_after_the_size_only_with_the_geometry() {
    let snapshot = snapshot("T", vec![element("a", "t", "n", None)], &[]);
    let mut snapshot = with_rect(snapshot, "a", sized(5, 6, 7, 8));
    let unpinned_without_geometry = outline_of(&snapshot);
    snapshot
        .boards
        .get_mut(&"b".parse().unwrap())
        .unwrap()
        .placements
        .get_mut(&"a".parse().unwrap())
        .unwrap()
        .pinned = true;

    assert_eq!(
        outline_with_geometry(&snapshot),
        "Board: b · T\n\na  @5,6 7x8 pinned  t  n\n"
    );
    assert_eq!(outline_of(&snapshot), unpinned_without_geometry);
}

/// `snapshot` with `a` placed at 100, 200 and `n` named as hanging on
/// `host` at the offset 5, 6, pinned.
fn with_a_note_on(snapshot: Snapshot, host: &str) -> Snapshot {
    let mut snapshot = with_rect(snapshot, "a", sized(100, 200, 7, 8));
    snapshot
        .boards
        .get_mut(&"b".parse().unwrap())
        .unwrap()
        .placements
        .insert(
            "n".parse().unwrap(),
            Placement {
                rect: sized(5, 6, 3, 4),
                pinned: true,
                attached_to: Some(host.parse().unwrap()),
            },
        );
    snapshot
}

#[test]
fn shows_where_an_attached_note_lies_on_the_board_and_its_box_only_with_the_geometry() {
    let elements = vec![
        element("a", "t", "A", None),
        element("n", "note", "Ask", None),
    ];
    let attached = with_a_note_on(snapshot("T", elements.clone(), &[]), "a");
    let loose = with_rect(snapshot("T", elements, &[]), "a", sized(100, 200, 7, 8));

    assert_eq!(
        outline_with_geometry(&attached),
        "Board: b · T\n\n\
         a  @100,200 7x8              t     A\n\
         n  @105,206 3x4 pinned on a  note  Ask\n"
    );
    assert_eq!(outline_of(&attached), outline_of(&loose));
}

#[test]
fn says_so_when_an_attachment_does_not_hold() {
    let elements = vec![
        element("a", "t", "A", None),
        element("n", "note", "Ask", None),
    ];
    let snapshot = with_a_note_on(snapshot("T", elements, &[]), "gone");

    assert_eq!(
        outline_with_geometry(&snapshot),
        "Board: b · T\n\n\
         a  @100,200 7x8                              t     A\n\
         n  @5,6 3x4 pinned not on gone (attachment does not hold)  note  Ask\n"
    );
}

#[test]
fn a_name_that_says_pinned_marks_no_position() {
    let snapshot = snapshot("T", vec![element("a", "t", "pinned", None)], &[]);
    let snapshot = with_rect(snapshot, "a", sized(5, 6, 7, 8));

    assert_eq!(
        outline_with_geometry(&snapshot),
        "Board: b · T\n\na  @5,6 7x8  t  pinned\n"
    );
}
