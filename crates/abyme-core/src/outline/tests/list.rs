//! Tests of the list of elements that search shows.

use super::*;

#[test]
fn lists_elements_in_the_given_order_and_marks_missing_ones() {
    let snapshot = snapshot(
        "Board",
        vec![
            element("a", "t", "A", None),
            element("b", "t", "B", Some("second")),
            element("c", "t", "C", None),
        ],
        &[],
    );
    let ids: Vec<ElementId> = ["c", "a", "gone", "b"]
        .iter()
        .map(|id| id.parse().unwrap())
        .collect();

    assert_eq!(
        element_list(&snapshot, &ids, None),
        "c     t  C\n\
         a     t  A\n\
         gone  ⚠ missing\n\
         b     t  B  second\n"
    );
}

#[test]
fn lists_an_element_given_twice_twice() {
    let snapshot = snapshot("Board", vec![element("a", "t", "A", None)], &[]);
    let id: ElementId = "a".parse().unwrap();

    assert_eq!(
        element_list(&snapshot, [&id, &id], None),
        "a  t  A\na  t  A\n"
    );
}

#[test]
fn lists_nothing_for_no_ids() {
    let snapshot = snapshot("Board", vec![element("a", "t", "A", None)], &[]);

    assert_eq!(element_list(&snapshot, [], None), "");
}

#[test]
fn escapes_element_text_in_the_list() {
    let snapshot = snapshot("Board", vec![element("a", "t", "x\ny\u{202e}z", None)], &[]);
    let id: ElementId = "a".parse().unwrap();

    assert_eq!(
        element_list(&snapshot, [&id], None),
        "a  t  x\\ny\\u{202e}z\n"
    );
}

#[test]
fn marks_search_lines_like_board_lines() {
    let (snapshot, review) = marked();
    let ids: Vec<ElementId> = ["c", "a"].iter().map(|id| id.parse().unwrap()).collect();

    assert_eq!(
        element_list(&snapshot, &ids, Some(&review)),
        "c  t  C\na  t  A    ⚠ task\n"
    );
}
