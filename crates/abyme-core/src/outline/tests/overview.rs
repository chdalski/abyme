//! Tests of the overview of all boards.

use super::*;

#[test]
fn renders_the_boards_as_a_tree() {
    let context = Board {
        title: "Context".to_owned(),
        summary: Some("The shop and its users".to_owned()),
        ..hierarchy_board("context", None, &["shop"])
    };
    let containers = Board {
        title: "Containers".to_owned(),
        summary: Some("What the shop runs".to_owned()),
        ..hierarchy_board("containers", Some("shop"), &["payment", "db"])
    };
    let components = Board {
        title: "Components".to_owned(),
        ..hierarchy_board("components", Some("payment"), &["api"])
    };
    let snapshot = hierarchy(
        vec![context, containers, components],
        &["shop", "payment", "db", "api"],
    );

    assert_eq!(
        overview(&snapshot, None),
        "context         Context     The shop and its users  1 element\n  \
         containers    Containers  What the shop runs      2 elements\n    \
         components  Components                          1 element\n"
    );
}

#[test]
fn renders_no_boards_as_a_sentence() {
    assert_eq!(overview(&Snapshot::default(), None), "No boards.");
}

#[test]
fn lists_the_boards_out_of_the_tree_at_the_top_level_with_their_owner_lines() {
    let overview = overview(&damaged(), None);

    let first_columns: Vec<&str> = overview
        .lines()
        .map(|line| {
            line.split("  ")
                .find(|column| !column.is_empty())
                .unwrap_or("")
        })
        .collect();
    let line_of = |board: &str| {
        overview
            .lines()
            .find(|line| line.trim_start().starts_with(&format!("{board} ")))
            .unwrap_or("")
    };
    assert_eq!(
        first_columns,
        [
            "alpha",
            "doubled",
            "ghost-loop",
            "homeless",
            "itself",
            "left",
            "one",
            "below",
            "orphan",
            "orphan-placed",
            "right",
            "three",
            "top",
            "two"
        ]
    );
    assert!(line_of("doubled").starts_with("  doubled "), "{overview}");
    assert!(line_of("below").starts_with("  below "), "{overview}");
    for (board, owner_line) in [
        (
            "ghost-loop",
            "Inside of: ghost ⚠ missing · Above: ghost-loop ⚠ cycle",
        ),
        ("homeless", "Inside of: loner ⚠ no home"),
        ("one", "Inside of: x · Above: two ⚠ cycle"),
        ("orphan", "Inside of: gone ⚠ missing"),
        (
            "orphan-placed",
            "Inside of: placed-gone ⚠ missing · Above: top",
        ),
    ] {
        assert!(line_of(board).ends_with(owner_line), "{overview}");
    }
    assert!(!line_of("doubled").contains("Inside of"), "{overview}");
    assert!(!line_of("below").contains("Inside of"), "{overview}");
}

#[test]
fn counts_only_the_elements_with_a_file_whose_home_a_board_is() {
    let overview = overview(&damaged(), None);

    // `top` places `twice`, whose home is `alpha`, and `placed-gone`, which
    // has no file.
    let counts: Vec<(&str, &str)> = overview
        .lines()
        .filter_map(|line| {
            let columns: Vec<&str> = line
                .split("  ")
                .filter(|column| !column.is_empty())
                .collect();
            Some((*columns.first()?, *columns.get(2)?))
        })
        .filter(|(board, _)| ["alpha", "top", "one"].contains(&board.trim()))
        .collect();
    assert_eq!(
        counts,
        [
            ("alpha", "1 element"),
            ("one", "2 elements"),
            ("top", "0 elements")
        ]
    );
}

#[test]
fn counts_the_flags_of_each_board_s_own_elements_by_priority() {
    let (mut snapshot, review) = marked();
    // `a`, with a task, has its home on `aa` now, which sorts before `b`.
    let aa = hierarchy_board("aa", None, &["a"]);
    snapshot.boards.insert(aa.id.clone(), aa);

    assert_eq!(
        overview(&snapshot, Some(&review)),
        "aa  aa       1 element   ⚠ 1 task\n\
         b   Board    2 elements  ⚠ 1 hint\n"
    );
}

#[test]
fn escapes_the_title_and_the_summary_in_the_overview() {
    let board = Board {
        title: "T\r\nx\t\\".to_owned(),
        summary: Some("s\u{2028}\u{202e}\n".to_owned()),
        ..hierarchy_board("b", None, &[])
    };
    let snapshot = hierarchy(vec![board], &[]);

    assert_eq!(
        overview(&snapshot, None),
        "b  T\\r\\nx\\t\\\\  s\\u{2028}\\u{202e}\\n  0 elements\n"
    );
}

#[test]
fn lists_at_most_fifty_boards_and_pads_only_by_those() {
    let boards: Vec<Board> = (0..51)
        .map(|index| Board {
            title: if index == 50 {
                "A title far longer than any other on these boards".to_owned()
            } else {
                "T".to_owned()
            },
            ..hierarchy_board(&format!("b{index:02}"), None, &[])
        })
        .collect();

    let overview = overview(&hierarchy(boards, &[]), None);

    let lines: Vec<&str> = overview.lines().collect();
    assert_eq!(lines.len(), 51, "{overview}");
    assert_eq!(lines.first(), Some(&"b00  T    0 elements"));
    assert_eq!(lines.get(49), Some(&"b49  T    0 elements"));
    assert_eq!(lines.last(), Some(&"… and 1 more board"));
}

#[test]
fn renders_the_overview_of_a_cycle_of_ten_thousand_boards() {
    // Board `b{n}` is the inside of `e{n}`, which lies on `b{n+1}`, and
    // `e9999` lies on `b0`.
    let boards: Vec<Board> = (0..10_000_usize)
        .map(|index| {
            let below = format!("e{}", (index + 9_999) % 10_000);
            hierarchy_board(&format!("b{index}"), Some(&format!("e{index}")), &[&below])
        })
        .collect();
    let elements: Vec<String> = (0..10_000).map(|index| format!("e{index}")).collect();
    let elements: Vec<&str> = elements.iter().map(String::as_str).collect();

    let overview = overview(&hierarchy(boards, &elements), None);

    let lines: Vec<&str> = overview.lines().collect();
    assert_eq!(lines.len(), 51);
    assert!(
        lines.first().is_some_and(|line| line.ends_with("⚠ cycle")),
        "{overview}"
    );
    assert_eq!(lines.last(), Some(&"… and 9950 more boards"));
}

#[rstest]
#[case::a_title_at_the_limit(&"t".repeat(80), None, &"t".repeat(80), "")]
#[case::a_title_past_the_limit(&"t".repeat(81), None, &format!("{}…", "t".repeat(80)), "")]
#[case::a_summary_of_wide_characters_past_the_limit(
    "T",
    Some("é".repeat(161)),
    "T",
    &format!("{}…", "é".repeat(160))
)]
#[case::a_summary_cut_after_a_line_feed(
    "T",
    Some(format!("{}\ny", "s".repeat(159))),
    "T",
    &format!("{}\\n…", "s".repeat(159))
)]
fn cuts_long_titles_and_summaries_in_the_overview_before_escaping(
    #[case] title: &str,
    #[case] summary: Option<String>,
    #[case] expected_title: &str,
    #[case] expected_summary: &str,
) {
    let board = Board {
        title: title.to_owned(),
        summary,
        ..hierarchy_board("b", None, &[])
    };

    let overview = overview(&hierarchy(vec![board], &[]), None);

    let columns: Vec<&str> = overview
        .trim_end()
        .split("  ")
        .filter(|column| !column.is_empty())
        .collect();
    let expected: Vec<&str> = ["b", expected_title, expected_summary, "0 elements"]
        .into_iter()
        .filter(|column| !column.is_empty())
        .collect();
    assert_eq!(columns, expected);
}

#[test]
fn keeps_the_overview_small_whatever_the_size_of_titles_and_summaries() {
    let boards: Vec<Board> = (0..50)
        .map(|index| Board {
            title: "\u{202e}".repeat(10_000),
            summary: Some("\u{1b}".repeat(10_000)),
            ..hierarchy_board(&format!("b{index:02}"), None, &[])
        })
        .collect();

    let overview = overview(&hierarchy(boards, &[]), None);

    // Escaped in full, the texts would take 50 × 140,000 bytes.
    assert_eq!(overview.lines().count(), 50);
    assert!(overview.len() < 100_000, "{} bytes", overview.len());
}
