//! Tests of review markers, the review section of a detail, and the status report.

use super::*;

fn reviewed_detail(snapshot: &Snapshot, facts: &GitFacts) -> String {
    let review = review(snapshot, facts);
    element_detail(snapshot, &"x".parse().unwrap(), Some(&review)).unwrap()
}

#[test]
fn renders_review_none_for_an_element_without_flags() {
    let snapshot = stamped_snapshot();

    let detail = reviewed_detail(&snapshot, &facts(&[b"crates/x/lib.rs"], &[]));

    assert!(
        detail.contains(&format!(
            "\nVerified: {STAMP} by person\nReview: none\nBoards: none\n"
        )),
        "{detail}"
    );
}

#[test]
fn renders_each_flag_on_its_own_line_after_the_highest_priority() {
    let mut snapshot = snapshot("Board", Vec::new(), &[]);
    let element = anchored(element("x", "t", "X", Some("S")), "crates/gone", None);
    snapshot.elements.insert(element.id().clone(), element);

    let detail = reviewed_detail(&snapshot, &facts(&[], &[]));

    assert!(
        detail.contains(
            "\nVerified: none\n\
             Review: task\n  \
             never verified\n  \
             no file that Git knows lies under the anchor\n\
             Summary: S\n"
        ),
        "{detail}"
    );
}

#[test]
fn names_at_most_ten_changed_files_escaped_and_counts_the_rest() {
    let snapshot = stamped_snapshot();
    let mut since: Vec<Vec<u8>> = (0..11)
        .map(|index| format!("crates/x/{index:02}.rs").into_bytes())
        .collect();
    since.extend([
        b"crates/x/a\nb".to_vec(),
        b"crates/x/\xe2\x80\xaeevil".to_vec(),
        b"crates/x/\xff".to_vec(),
    ]);
    let since: Vec<&[u8]> = since.iter().map(Vec::as_slice).collect();

    let detail = reviewed_detail(&snapshot, &facts(&[b"crates/x/00.rs"], &since));

    let line = detail
        .lines()
        .find(|line| line.starts_with("  changed since the stamp: "))
        .unwrap();
    assert_eq!(
        line,
        "  changed since the stamp: crates/x/00.rs, crates/x/01.rs, crates/x/02.rs, \
         crates/x/03.rs, crates/x/04.rs, crates/x/05.rs, crates/x/06.rs, crates/x/07.rs, \
         crates/x/08.rs, crates/x/09.rs and 4 more"
    );
}

#[test]
fn escapes_hostile_file_names_in_a_review() {
    let snapshot = stamped_snapshot();
    let since: [&[u8]; 3] = [
        b"crates/x/a\nb",
        b"crates/x/\xe2\x80\xaeevil",
        b"crates/x/\xff",
    ];

    let detail = reviewed_detail(&snapshot, &facts(&[b"crates/x/a\nb"], &since));

    assert!(
        detail.contains(
            "  changed since the stamp: crates/x/a\\nb, crates/x/\\u{202e}evil, crates/x/\u{fffd}\n"
        ),
        "{detail}"
    );
}

#[test]
fn marks_the_highest_priority_last_on_each_element_line() {
    let (snapshot, review) = marked();

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
         a  t  A     ⚠ task\n\
         b  t  B  S  ⚠ hint\n\
         c  t  C\n\
         \n\
         b → a  uses\n"
    );
}

#[test]
fn marks_nothing_without_a_review() {
    let (snapshot, _) = marked();

    let outline = outline(&snapshot, &"b".parse().unwrap(), Geometry::Hidden, None).unwrap();

    assert!(!outline.contains('⚠'), "{outline}");
}

#[test]
fn keeps_the_marker_last_when_lines_show_coordinates() {
    let (snapshot, review) = marked();

    let outline = outline(
        &snapshot,
        &"b".parse().unwrap(),
        Geometry::Shown,
        Some(&review),
    )
    .unwrap();

    let line = outline.lines().find(|line| line.starts_with("a ")).unwrap();
    assert_eq!(line, "a  @0,0 1x1  t  A     ⚠ task");
}

#[test]
fn reports_no_open_review_flags() {
    let snapshot = snapshot("Board", vec![element("c", "t", "C", None)], &[]);
    let review = review(&snapshot, &facts(&[], &[]));

    assert_eq!(status_report(&snapshot, &review), "No open review flags.");
}

#[test]
fn reports_tasks_before_hints_after_a_count() {
    let (snapshot, review) = marked();

    assert_eq!(
        status_report(&snapshot, &review),
        "Elements to review: 1 with a task, 1 with only a hint.\n\
         a  t  A     ⚠ task\n\
         b  t  B  S  ⚠ hint\n"
    );
}

#[test]
fn reports_hostile_names_on_one_line_each() {
    let mut snapshot = snapshot("Board", Vec::new(), &[]);
    let hostile = anchored(
        element("x", "t", "a\nb\tc", Some("c\rd\u{202e}")),
        "crates/x",
        None,
    );
    snapshot.elements.insert(hostile.id().clone(), hostile);
    let review = review(&snapshot, &facts(&[b"crates/x/lib.rs"], &[]));

    let report = status_report(&snapshot, &review);

    assert_eq!(report.lines().count(), 2, "{report:?}");
    assert!(
        report.ends_with("x  t  a\\nb\\tc  c\\rd\\u{202e}  ⚠ task\n"),
        "{report:?}"
    );
}

#[test]
fn reports_at_most_fifty_elements_and_counts_all() {
    let tasks = (0..55).map(|number| {
        anchored(
            element(&format!("e{number:02}"), "t", "E", None),
            &format!("crates/e{number:02}"),
            None,
        )
    });
    let hints = (0..10).map(|number| {
        anchored(
            related(
                element(&format!("h{number}"), "t", "H", None),
                &[(RelationKind::Uses, "e00", None)],
            ),
            &format!("docs/h{number}"),
            Some((STAMP, Verifier::Ai)),
        )
    });
    let snapshot = snapshot("Board", tasks.chain(hints).collect(), &[]);
    let known: Vec<Vec<u8>> = (0..55)
        .map(|number| format!("crates/e{number:02}/lib.rs").into_bytes())
        .chain((0..10).map(|number| format!("docs/h{number}/a.md").into_bytes()))
        .collect();
    let known: Vec<&[u8]> = known.iter().map(Vec::as_slice).collect();
    let review = review(&snapshot, &facts(&known, &[b"crates/e00/lib.rs"]));

    let report = status_report(&snapshot, &review);

    let lines: Vec<&str> = report.lines().collect();
    assert_eq!(
        lines.first(),
        Some(&"Elements to review: 55 with a task, 10 with only a hint.")
    );
    assert_eq!(lines.len(), 1 + 50 + 1, "{report}");
    assert!(
        lines.get(1).is_some_and(|line| line.starts_with("e00 ")),
        "{report}"
    );
    assert!(
        lines.get(50).is_some_and(|line| line.starts_with("e49 ")),
        "{report}"
    );
    assert_eq!(lines.last(), Some(&"… and 15 more"));
}
