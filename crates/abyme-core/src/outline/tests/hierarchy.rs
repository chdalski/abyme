//! Tests of a board's header lines and of a broken hierarchy.

use super::*;

#[test]
fn shows_whose_inside_a_board_is_and_which_elements_open_an_inner_board() {
    let containers = Board {
        summary: Some("What the shop runs".to_owned()),
        ..hierarchy_board("containers", Some("shop"), &["payment", "db"])
    };
    let snapshot = hierarchy(
        vec![
            hierarchy_board("context", None, &["shop"]),
            containers,
            hierarchy_board("components", Some("payment"), &["api"]),
        ],
        &["shop", "payment", "db", "api"],
    );

    assert_eq!(
        outline_of_board(&snapshot, "containers"),
        "Board: containers · containers\n\
         Inside of: shop · Above: context\n\
         Summary: What the shop runs\n\
         \n\
         db       t  DB\n\
         payment  t  PAYMENT    ▸ components\n"
    );
}

#[rstest]
#[case::a_summary_alone(None, Some("S"), "Board: b · b\nSummary: S\n\n")]
#[case::an_owner_alone(Some("x"), None, "Board: b · b\nInside of: x · Above: top\n\n")]
#[case::an_owner_and_then_the_summary(
    Some("x"),
    Some("S"),
    "Board: b · b\nInside of: x · Above: top\nSummary: S\n\n"
)]
#[case::an_empty_summary_as_none(None, Some(""), "Board: b · b\n\n")]
fn writes_the_header_lines_in_order(
    #[case] owner: Option<&str>,
    #[case] summary: Option<&str>,
    #[case] expected: &str,
) {
    let board = Board {
        summary: summary.map(str::to_owned),
        ..hierarchy_board("b", owner, &[])
    };
    let snapshot = hierarchy(vec![board, hierarchy_board("top", None, &["x"])], &["x"]);

    assert_eq!(outline_of_board(&snapshot, "b"), expected);
}

#[test]
fn escapes_the_title_and_the_summary_onto_their_lines() {
    let board = Board {
        title: "T\r\nBoard: forged\t\\".to_owned(),
        summary: Some(
            "x\nInside of: evil · Above: main\u{2028}\u{202e}\u{200b}\u{1b}[31m".to_owned(),
        ),
        ..hierarchy_board("b", Some("x"), &[])
    };
    let snapshot = hierarchy(vec![board, hierarchy_board("top", None, &["x"])], &["x"]);

    assert_eq!(
        outline_of_board(&snapshot, "b"),
        "Board: b · T\\r\\nBoard: forged\\t\\\\\n\
         Inside of: x · Above: top\n\
         Summary: x\\nInside of: evil · Above: main\\u{2028}\\u{202e}\\u{200b}\\u{1b}[31m\n\
         \n"
    );
}

proptest! {
    #[test]
    fn writes_each_header_value_on_its_own_line_without_any_escaped_character(
        title in any::<String>(),
        summary in any::<String>(),
    ) {
        let board = Board {
            title,
            summary: Some(summary.clone()),
            ..hierarchy_board("b", Some("x"), &[])
        };
        let snapshot = hierarchy(vec![board, hierarchy_board("top", None, &["x"])], &["x"]);
        let expected = 3 + usize::from(!summary.is_empty());

        let outline = outline_of_board(&snapshot, "b");

        prop_assert_eq!(outline.lines().count(), expected);
        prop_assert_eq!(outline.matches('\n').count(), expected);
        prop_assert!(
            !outline.replace('\n', "").chars().any(hides_or_breaks_text),
            "{:?}",
            outline
        );
    }
}

#[rstest]
#[case::an_owner_with_neither_file_nor_home("orphan", "Inside of: gone ⚠ missing")]
#[case::an_owner_without_a_file_that_a_board_places(
    "orphan-placed",
    "Inside of: placed-gone ⚠ missing · Above: top"
)]
#[case::an_owner_without_a_home("homeless", "Inside of: loner ⚠ no home")]
#[case::an_owner_on_two_boards_below_the_lower_id("doubled", "Inside of: twice · Above: alpha")]
#[case::a_board_whose_owner_lies_on_it("itself", "Inside of: mirror · Above: itself ⚠ cycle")]
#[case::a_board_whose_owner_without_a_file_lies_on_it(
    "ghost-loop",
    "Inside of: ghost ⚠ missing · Above: ghost-loop ⚠ cycle"
)]
#[case::the_first_board_of_a_cycle_of_three("one", "Inside of: x · Above: two ⚠ cycle")]
#[case::the_second_board_of_a_cycle_of_three("two", "Inside of: y · Above: three ⚠ cycle")]
#[case::the_third_board_of_a_cycle_of_three("three", "Inside of: z · Above: one ⚠ cycle")]
#[case::a_board_below_a_cycle("below", "Inside of: w · Above: one")]
#[case::one_board_of_a_cycle_of_two("left", "Inside of: r · Above: right ⚠ cycle")]
#[case::the_other_board_of_a_cycle_of_two("right", "Inside of: l · Above: left ⚠ cycle")]
fn marks_every_owner_and_board_that_breaks_the_hierarchy(
    #[case] board: &str,
    #[case] expected: &str,
) {
    let outline = outline_of_board(&damaged(), board);

    assert_eq!(outline.lines().nth(1), Some(expected), "{outline}");
}

/// The board `top` placing `twice`, the owner of `first` and `second`, and
/// `plain`, which owns no board.
fn twice_owned() -> Snapshot {
    hierarchy(
        vec![
            hierarchy_board("top", None, &["twice", "plain"]),
            hierarchy_board("second", Some("twice"), &[]),
            hierarchy_board("first", Some("twice"), &[]),
        ],
        &["twice", "plain"],
    )
}

#[test]
fn shows_both_boards_of_an_owner_of_two_on_its_line() {
    let snapshot = twice_owned();
    let ids: Vec<ElementId> = ["twice", "plain"]
        .iter()
        .map(|id| id.parse().unwrap())
        .collect();

    assert_eq!(
        outline_of_board(&snapshot, "top"),
        "Board: top · top\n\
         \n\
         plain  t  PLAIN\n\
         twice  t  TWICE    ▸ first, second\n"
    );
    assert_eq!(
        element_list(&snapshot, &ids, None),
        "twice  t  TWICE    ▸ first, second\n\
         plain  t  PLAIN\n"
    );
    assert_eq!(
        outline_of_board(&snapshot, "first").lines().nth(1),
        Some("Inside of: twice · Above: top")
    );
    assert_eq!(
        outline_of_board(&snapshot, "second").lines().nth(1),
        Some("Inside of: twice · Above: top")
    );
}

#[rstest]
#[case::two_boards("twice", "Boards: top\nInner board: first, second\n")]
#[case::none("plain", "Boards: top\nInner board: none\n")]
fn names_every_inner_board_in_the_detail(#[case] id: &str, #[case] expected: &str) {
    let detail = detail_of(&twice_owned(), id);

    assert!(detail.contains(expected), "{detail}");
}

#[test]
fn ends_no_line_in_spaces_when_an_empty_summary_ends_it() {
    let snapshot = snapshot("Board", vec![element("a", "t", "A", Some(""))], &[]);
    let id: ElementId = "a".parse().unwrap();

    assert_eq!(outline_of(&snapshot), "Board: b · Board\n\na  t  A\n");
    assert_eq!(element_list(&snapshot, [&id], None), "a  t  A\n");
}

/// The snapshot and review of [`marked`], with `inside-a`, the inside of
/// `a`, and `inside-c`, the inside of `c`.
fn marked_with_inner_boards() -> (Snapshot, Review) {
    let (mut snapshot, review) = marked();
    for (board, owner) in [("inside-a", "a"), ("inside-c", "c")] {
        let inside = hierarchy_board(board, Some(owner), &[]);
        snapshot.boards.insert(inside.id.clone(), inside);
    }
    (snapshot, review)
}

#[test]
fn shows_the_inner_boards_before_the_marker() {
    let (snapshot, review) = marked_with_inner_boards();

    let outline = outline(
        &snapshot,
        &"b".parse().unwrap(),
        Geometry::Hidden,
        Some(&review),
    )
    .unwrap();

    assert_eq!(
        outline,
        "Board: b · Board\n\
         \n\
         a  t  A     ▸ inside-a  ⚠ task\n\
         b  t  B  S              ⚠ hint\n\
         c  t  C     ▸ inside-c\n\
         \n\
         b → a  uses\n"
    );
}

#[test]
fn reports_the_inner_boards_of_the_elements_to_review() {
    let (snapshot, review) = marked_with_inner_boards();

    assert_eq!(
        status_report(&snapshot, &review),
        "Elements to review: 1 with a task, 1 with only a hint.\n\
         a  t  A     ▸ inside-a  ⚠ task\n\
         b  t  B  S              ⚠ hint\n"
    );
}
