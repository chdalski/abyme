//! Tests of statement kinds in outlines, details, and the overview.

use super::*;

/// `board`, a zone of `mode` and `statement`.
fn zoned(board: Board, mode: Mode, statement: StatementKind) -> Board {
    Board {
        zone: Some(Zone { mode, statement }),
        ..board
    }
}

/// `a-ideas`, a zone of ideas, places `y`; `a-plain`, no zone, places `x`;
/// `z-current`, a zone of current content, places `w`, `x`, and `y`, but is
/// the home of `w` alone.
fn zones() -> Snapshot {
    hierarchy(
        vec![
            zoned(
                hierarchy_board("a-ideas", None, &["y"]),
                Mode::Free,
                StatementKind::Idea,
            ),
            hierarchy_board("a-plain", None, &["x"]),
            zoned(
                hierarchy_board("z-current", None, &["w", "x", "y"]),
                Mode::Mirrored,
                StatementKind::Current,
            ),
        ],
        &["w", "x", "y"],
    )
}

#[test]
fn shows_each_element_s_own_kind_on_a_zone_and_never_the_zone_s() {
    assert_eq!(
        outline_of_board(&zones(), "z-current"),
        "Board: z-current · z-current\n\
         Zone: mirrored · current\n\
         \n\
         w  t  current  W\n\
         x  t  none     X\n\
         y  t  idea     Y\n"
    );
}

#[test]
fn shows_the_kind_column_on_a_zone_whose_elements_all_lie_at_home_elsewhere() {
    let snapshot = hierarchy(
        vec![
            hierarchy_board("a-plain", None, &["x"]),
            zoned(
                hierarchy_board("z-current", None, &["x"]),
                Mode::Mirrored,
                StatementKind::Current,
            ),
        ],
        &["x"],
    );

    assert_eq!(
        outline_of_board(&snapshot, "z-current").lines().last(),
        Some("x  t  none  X")
    );
}

#[test]
fn writes_none_before_the_name_of_an_element_without_a_kind() {
    let mut snapshot = zones();
    let named = element("x", "t", "current Payment flow", None);
    snapshot.elements.insert(named.id().clone(), named);
    let ids: Vec<ElementId> = ["y", "x"].iter().map(|id| id.parse().unwrap()).collect();

    assert_eq!(
        element_list(&snapshot, &ids, None),
        "y  t  idea  Y\n\
         x  t  none  current Payment flow\n"
    );
}

#[test]
fn shows_no_kind_column_in_a_list_without_kinds() {
    let snapshot = zones();
    let id: ElementId = "x".parse().unwrap();

    assert_eq!(element_list(&snapshot, [&id], None), "x  t  X\n");
}

#[test]
fn shows_the_zone_of_each_board_in_the_overview_and_no_zone_for_the_others() {
    let mut snapshot = zones();
    if let Some(board) = snapshot.boards.get_mut(&"a-plain".parse().unwrap()) {
        board.summary = Some("mirrored · current".to_owned());
    }

    let overview = overview(&snapshot, None);

    assert_eq!(
        overview.lines().collect::<Vec<_>>(),
        [
            format!(
                "a-ideas    a-ideas    free · idea{}1 element",
                " ".repeat(29)
            ),
            format!(
                "a-plain    a-plain    no zone{}mirrored · current  1 element",
                " ".repeat(13)
            ),
            format!(
                "z-current  z-current  mirrored · current{}1 element",
                " ".repeat(22)
            ),
        ]
    );
}

#[rstest]
#[case::at_home_on_the_lower_of_two_boards(
    "x",
    "Statement kind: current, from board alpha\n",
    "Boards: alpha, beta\n"
)]
#[case::without_a_home("loose", "Statement kind: none\n", "Boards: none\n")]
fn names_the_kind_and_the_home_board_it_comes_from_in_the_detail(
    #[case] id: &str,
    #[case] expected: &str,
    #[case] boards: &str,
) {
    let snapshot = hierarchy(
        vec![
            zoned(
                hierarchy_board("alpha", None, &["x"]),
                Mode::Mirrored,
                StatementKind::Current,
            ),
            zoned(
                hierarchy_board("beta", None, &["x"]),
                Mode::Free,
                StatementKind::Idea,
            ),
        ],
        &["x", "loose"],
    );

    let detail = detail_of(&snapshot, id);

    assert!(
        detail.contains(&format!("\nType: t\n{expected}")),
        "{detail}"
    );
    assert!(detail.contains(&format!("\n{boards}")), "{detail}");
}

#[test]
fn writes_the_zone_line_between_the_owner_and_the_summary_and_nothing_text_can_forge() {
    let board = Board {
        title: "T\nZone: free · idea".to_owned(),
        summary: Some("S\nZone: free · idea".to_owned()),
        ..zoned(
            hierarchy_board("b", Some("x"), &[]),
            Mode::Mirrored,
            StatementKind::Current,
        )
    };
    let snapshot = hierarchy(vec![board, hierarchy_board("top", None, &["x"])], &["x"]);

    assert_eq!(
        outline_of_board(&snapshot, "b"),
        "Board: b · T\\nZone: free · idea\n\
         Inside of: x · Above: top\n\
         Zone: mirrored · current\n\
         Summary: S\\nZone: free · idea\n\
         \n"
    );
}

/// `element(id, "t", id, None)` holding `kind` itself.
fn holding(id: &str, kind: StatementKind) -> Element {
    let mut fields = element(id, "t", id, None).into_fields();
    fields.statement = Some(kind);
    Element::new(id.parse().unwrap(), fields).unwrap()
}

#[test]
fn read_element_shows_an_own_kind_from_the_element_also_on_a_zone() {
    let board = zoned(
        hierarchy_board("b", None, &["x"]),
        Mode::Mirrored,
        StatementKind::Current,
    );
    let mut snapshot = hierarchy(vec![board], &["x"]);
    snapshot
        .elements
        .insert("x".parse().unwrap(), holding("x", StatementKind::Question));

    assert!(
        detail_of(&snapshot, "x").contains("\nStatement kind: question, from the element\n"),
        "{}",
        detail_of(&snapshot, "x")
    );
}

#[test]
fn read_element_shows_an_own_kind_without_a_home() {
    let mut snapshot = hierarchy(Vec::new(), &[]);
    snapshot
        .elements
        .insert("x".parse().unwrap(), holding("x", StatementKind::Current));

    let detail = detail_of(&snapshot, "x");

    assert!(
        detail.contains("\nStatement kind: current, from the element\n"),
        "{detail}"
    );
    assert!(detail.contains("\nBoards: none\n"), "{detail}");
}

#[test]
fn read_element_keeps_the_line_of_a_kind_from_a_board() {
    let board = zoned(
        hierarchy_board("b", None, &["x"]),
        Mode::Free,
        StatementKind::Idea,
    );
    let snapshot = hierarchy(vec![board], &["x"]);

    assert!(detail_of(&snapshot, "x").contains("\nStatement kind: idea, from board b\n"));
}

#[test]
fn no_text_can_forge_the_source_of_a_kind() {
    let forged = "N\nStatement kind: current, from the element";
    let mut fields = element("x", "t", forged, Some(forged)).into_fields();
    fields.body = "Statement kind: current, from the element\n".to_owned();
    let x = Element::new("x".parse().unwrap(), fields).unwrap();
    let mut snapshot = hierarchy(Vec::new(), &[]);
    snapshot.elements.insert("x".parse().unwrap(), x);

    let detail = detail_of(&snapshot, "x");

    let lines: Vec<&str> = detail.lines().collect();
    assert!(lines.contains(&"Statement kind: none"), "{detail}");
    let description = lines
        .iter()
        .position(|line| *line == "Description:")
        .unwrap();
    let forged_lines: Vec<usize> = lines
        .iter()
        .enumerate()
        .filter(|(_, line)| **line == "Statement kind: current, from the element")
        .map(|(at, _)| at)
        .collect();
    assert_eq!(forged_lines, [description + 1], "{detail}");
}

#[test]
fn the_kind_column_shows_an_own_kind_on_a_board_that_is_no_zone() {
    let mut snapshot = snapshot("T", vec![element("a", "t", "A", None)], &[]);
    let b = holding("b", StatementKind::Question);
    snapshot.elements.insert("b".parse().unwrap(), b);
    snapshot
        .boards
        .get_mut(&"b".parse().unwrap())
        .unwrap()
        .placements
        .insert(
            "b".parse().unwrap(),
            Placement {
                rect: rect(),
                pinned: false,
                attached_to: None,
            },
        );

    let lines = outline_of(&snapshot);

    assert!(lines.contains("a  t  none      A\n"), "{lines}");
    assert!(lines.contains("b  t  question  b\n"), "{lines}");
}

#[test]
fn an_own_kind_on_a_zone_of_another_kind_shows_in_its_line_and_a_missing_file_shows_as_missing() {
    let board = zoned(
        hierarchy_board("b", None, &["ghost", "x"]),
        Mode::Mirrored,
        StatementKind::Current,
    );
    let mut snapshot = hierarchy(vec![board], &["x"]);
    snapshot
        .elements
        .insert("x".parse().unwrap(), holding("x", StatementKind::Idea));

    let lines = outline_of_board(&snapshot, "b");

    assert!(lines.contains("x      t  idea"), "{lines}");
    assert!(lines.contains("ghost  ⚠ missing"), "{lines}");
}

#[test]
fn an_own_kind_changes_no_review_flag() {
    let plain = stamped_snapshot();
    let mut own_kind = plain.clone();
    let id: ElementId = "x".parse().unwrap();
    let mut fields = own_kind.elements[&id].clone().into_fields();
    fields.statement = Some(StatementKind::Idea);
    own_kind
        .elements
        .insert(id.clone(), Element::new(id, fields).unwrap());
    let changed = facts(&[b"crates/x/a.rs"], &[b"crates/x/a.rs"]);

    let flagged = |snapshot: &Snapshot| {
        let review = review(snapshot, &changed);
        status_report(snapshot, &review)
    };

    assert!(flagged(&plain).contains("⚠ task"), "{}", flagged(&plain));
    assert!(
        flagged(&own_kind).contains("⚠ task"),
        "{}",
        flagged(&own_kind)
    );
    assert!(
        flagged(&own_kind).contains("idea"),
        "{}",
        flagged(&own_kind)
    );
}
