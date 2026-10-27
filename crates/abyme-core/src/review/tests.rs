//! Tests of computing review flags from hand-built facts.

use rstest::rstest;

use super::*;

const STAMP: &str = "0123456789abcdef0123456789abcdef01234567";
const OTHER: &str = "fedcba9876543210fedcba9876543210fedcba98";

fn element_id(value: &str) -> ElementId {
    value.parse().unwrap()
}

fn commit(value: &str) -> CommitId {
    CommitId::try_from(value.to_owned()).unwrap()
}

/// The element `x` with the given anchor and stamp commit.
fn element(anchor: Option<&str>, stamp: Option<&str>) -> Element {
    let anchor = anchor.map_or(String::new(), |anchor| format!("anchor: \"{anchor}\"\n"));
    let stamp = stamp.map_or(String::new(), |stamp| {
        format!("verified: \"{stamp}\"\nverified_by: person\n")
    });
    Element::from_markdown(
        &element_id("x"),
        &format!("---\nid: x\ntype: t\nname: X\n{anchor}{stamp}---\n"),
    )
    .unwrap()
}

fn snapshot(element: Element) -> Snapshot {
    Snapshot {
        elements: BTreeMap::from([(element.id().clone(), element)]),
        boards: BTreeMap::new(),
    }
}

fn paths(values: &[&str]) -> BTreeSet<Vec<u8>> {
    values
        .iter()
        .map(|value| value.as_bytes().to_vec())
        .collect()
}

/// Facts in which Git knows `crates/x/lib.rs` and `docs/a.md`, and the
/// stamp `STAMP` is known with the given changes since.
fn facts(since: &[&str], uncommitted: &[&str]) -> GitFacts {
    GitFacts {
        known: paths(&["crates/x/lib.rs", "docs/a.md"]),
        uncommitted: paths(uncommitted),
        since: BTreeMap::from([(commit(STAMP), Since::Changed(paths(since)))]),
    }
}

fn flags_of(element: Element, facts: &GitFacts) -> Vec<Flag> {
    review(&snapshot(element), facts)
        .flags(&element_id("x"))
        .to_vec()
}

fn changed(values: &[&str]) -> Flag {
    Flag::Changed(
        values
            .iter()
            .map(|value| value.as_bytes().to_vec())
            .collect(),
    )
}

#[test]
fn flags_nothing_on_a_verified_anchor_without_changes() {
    let flags = flags_of(element(Some("crates/x"), Some(STAMP)), &facts(&[], &[]));

    assert_eq!(flags, []);
}

#[rstest]
#[case::committed_or_staged(&["crates/x/lib.rs"], &[], &["crates/x/lib.rs"])]
#[case::in_the_working_tree_or_untracked(&[], &["crates/x/new.rs"], &["crates/x/new.rs"])]
#[case::both_merged_and_sorted(
    &["crates/x/lib.rs"],
    &["crates/x/a.rs", "crates/x/lib.rs"],
    &["crates/x/a.rs", "crates/x/lib.rs"]
)]
fn flags_files_under_the_anchor_changed_since_the_stamp(
    #[case] since: &[&str],
    #[case] uncommitted: &[&str],
    #[case] expected: &[&str],
) {
    let flags = flags_of(
        element(Some("crates/x"), Some(STAMP)),
        &facts(since, uncommitted),
    );

    assert_eq!(flags, [changed(expected)]);
}

#[test]
fn ignores_changes_outside_the_anchor() {
    let flags = flags_of(
        element(Some("crates/x"), Some(STAMP)),
        &facts(&["crates/xy/lib.rs", "docs/a.md"], &["crates/other.rs"]),
    );

    assert_eq!(flags, []);
}

#[test]
fn flags_an_anchored_element_without_a_stamp() {
    let flags = flags_of(element(Some("crates/x"), None), &facts(&[], &[]));

    assert_eq!(flags, [Flag::NeverVerified]);
}

#[rstest]
#[case::anchored(Some("crates/x"))]
#[case::native(None)]
fn flags_a_stamp_whose_commit_git_does_not_know(#[case] anchor: Option<&str>) {
    let mut facts = facts(&[], &[]);
    facts.since.insert(commit(OTHER), Since::Unknown);

    let flags = flags_of(element(anchor, Some(OTHER)), &facts);

    assert_eq!(flags, [Flag::UnknownStamp]);
}

#[test]
fn flags_a_stamp_the_facts_do_not_name_as_unknown() {
    let flags = flags_of(element(Some("crates/x"), Some(OTHER)), &facts(&[], &[]));

    assert_eq!(flags, [Flag::UnknownStamp]);
}

#[rstest]
#[case::verified(Some(STAMP), vec![Flag::AnchorMissing])]
#[case::never_verified(None, vec![Flag::NeverVerified, Flag::AnchorMissing])]
fn flags_an_anchor_that_matches_no_file(#[case] stamp: Option<&str>, #[case] expected: Vec<Flag>) {
    let flags = flags_of(element(Some("crates/gone"), stamp), &facts(&[], &[]));

    assert_eq!(flags, expected);
}

#[test]
fn flags_files_removed_under_an_anchor_that_now_matches_nothing() {
    let flags = flags_of(
        element(Some("crates/gone"), Some(STAMP)),
        &facts(&["crates/gone/lib.rs"], &[]),
    );

    assert_eq!(
        flags,
        [changed(&["crates/gone/lib.rs"]), Flag::AnchorMissing]
    );
}

#[test]
fn finds_an_anchor_that_is_a_file() {
    let flags = flags_of(element(Some("docs/a.md"), Some(STAMP)), &facts(&[], &[]));

    assert_eq!(flags, []);
}

#[rstest]
#[case::without_a_stamp(None)]
#[case::with_a_known_stamp(Some(STAMP))]
fn flags_nothing_on_an_element_without_an_anchor(#[case] stamp: Option<&str>) {
    let flags = flags_of(element(None, stamp), &facts(&["docs/a.md"], &["x"]));

    assert_eq!(flags, []);
}

#[test]
fn rates_every_own_flag_a_task() {
    let review = review(
        &snapshot(element(Some("crates/gone"), None)),
        &facts(&[], &[]),
    );

    assert_eq!(review.priority(&element_id("x")), Some(Priority::Task));
    assert_eq!(review.priority(&element_id("y")), None);
}
