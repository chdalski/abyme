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

/// The element `id` with the given anchor, stamp, and relations, each a
/// kind and a target.
fn node(
    id: &str,
    anchor: Option<&str>,
    stamp: Option<&str>,
    relations: &[(&str, &str)],
) -> Element {
    let anchor = anchor.map_or(String::new(), |anchor| format!("anchor: \"{anchor}\"\n"));
    let stamp = stamp.map_or(String::new(), |stamp| {
        format!("verified: \"{stamp}\"\nverified_by: ai\n")
    });
    let relations = if relations.is_empty() {
        String::new()
    } else {
        let lines: Vec<String> = relations
            .iter()
            .map(|(kind, to)| format!("  - {{kind: {kind}, to: {to}, label: \"secret label\"}}\n"))
            .collect();
        format!("relations:\n{}", lines.concat())
    };
    Element::from_markdown(
        &element_id(id),
        &format!("---\nid: {id}\ntype: t\nname: N\n{anchor}{stamp}{relations}---\n"),
    )
    .unwrap()
}

fn snapshot_of(elements: Vec<Element>) -> Snapshot {
    Snapshot {
        elements: elements
            .into_iter()
            .map(|element| (element.id().clone(), element))
            .collect(),
        boards: BTreeMap::new(),
    }
}

fn upstream(kind: RelationKind, target: &str, priority: Priority) -> Flag {
    Flag::Upstream {
        kind,
        target: element_id(target),
        priority,
    }
}

/// Facts in which `STAMP` is known and the index changed `crates/t/lib.rs`
/// since; Git knows the files under every anchor these tests use.
fn target_changed() -> GitFacts {
    GitFacts {
        known: paths(&["crates/t/lib.rs", "crates/u/lib.rs", "crates/v/lib.rs"]),
        uncommitted: BTreeSet::new(),
        since: BTreeMap::from([(commit(STAMP), Since::Changed(paths(&["crates/t/lib.rs"])))]),
    }
}

#[rstest]
#[case::depends_on("depends_on", Some(Priority::Task))]
#[case::describes("describes", Some(Priority::Task))]
#[case::uses("uses", Some(Priority::Hint))]
#[case::concerns("concerns", Some(Priority::Hint))]
#[case::see_also("see_also", None)]
fn passes_on_what_the_relation_kind_says(#[case] kind: &str, #[case] priority: Option<Priority>) {
    let snapshot = snapshot_of(vec![
        node("s", None, Some(STAMP), &[(kind, "t")]),
        node("t", Some("crates/t"), Some(STAMP), &[]),
    ]);

    let review = review(&snapshot, &target_changed());

    let expected: Vec<Flag> = priority
        .into_iter()
        .map(|priority| upstream(RelationKind::from_name(kind).unwrap(), "t", priority))
        .collect();
    assert_eq!(review.flags(&element_id("s")), expected.as_slice());
    assert_eq!(review.priority(&element_id("s")), priority);
}

#[rstest]
#[case::source_without_a_stamp(None, "t")]
#[case::source_with_an_unknown_stamp(Some(OTHER), "t")]
#[case::target_without_an_anchor(Some(STAMP), "native")]
#[case::target_without_a_file(Some(STAMP), "gone")]
#[case::target_whose_anchor_did_not_change(Some(STAMP), "u")]
fn passes_nothing_on_without_a_stamp_to_measure_or_a_changed_target_anchor(
    #[case] stamp: Option<&str>,
    #[case] target: &str,
) {
    let snapshot = snapshot_of(vec![
        node("s", None, stamp, &[("depends_on", target)]),
        node("t", Some("crates/t"), Some(STAMP), &[]),
        node("u", Some("crates/u"), Some(STAMP), &[]),
        node("native", None, Some(STAMP), &[]),
    ]);
    let mut facts = target_changed();
    facts.since.insert(commit(OTHER), Since::Unknown);

    let review = review(&snapshot, &facts);

    let upstream = review
        .flags(&element_id("s"))
        .iter()
        .filter(|flag| matches!(flag, Flag::Upstream { .. }))
        .count();
    assert_eq!(upstream, 0);
}

#[test]
fn passes_nothing_on_from_a_stamp_the_facts_do_not_name() {
    let snapshot = snapshot_of(vec![
        node("s", None, Some(OTHER), &[("depends_on", "t")]),
        node("t", Some("crates/t"), Some(STAMP), &[]),
    ]);

    let review = review(&snapshot, &target_changed());

    assert_eq!(review.flags(&element_id("s")), [Flag::UnknownStamp]);
}

#[test]
fn passes_on_a_change_in_the_working_tree() {
    let snapshot = snapshot_of(vec![
        node("s", None, Some(STAMP), &[("uses", "u")]),
        node("u", Some("crates/u"), Some(STAMP), &[]),
    ]);
    let mut facts = target_changed();
    facts.uncommitted = paths(&["crates/u/new.rs"]);

    let review = review(&snapshot, &facts);

    assert_eq!(
        review.flags(&element_id("s")),
        [upstream(RelationKind::Uses, "u", Priority::Hint)]
    );
}

#[test]
fn passes_flags_on_one_hop_only() {
    let snapshot = snapshot_of(vec![
        node("a", None, Some(STAMP), &[("depends_on", "b")]),
        node("b", Some("crates/u"), Some(STAMP), &[("depends_on", "c")]),
        node("c", Some("crates/t"), Some(STAMP), &[]),
    ]);

    let review = review(&snapshot, &target_changed());

    assert_eq!(review.flags(&element_id("a")), []);
    assert_eq!(
        review.flags(&element_id("b")),
        [upstream(RelationKind::DependsOn, "c", Priority::Task)]
    );
}

#[test]
fn passes_flags_both_ways_in_a_cycle() {
    let snapshot = snapshot_of(vec![
        node("a", Some("crates/t"), Some(STAMP), &[("depends_on", "b")]),
        node("b", Some("crates/t"), Some(STAMP), &[("depends_on", "a")]),
    ]);

    let review = review(&snapshot, &target_changed());

    assert_eq!(
        review.flags(&element_id("a")),
        [
            changed(&["crates/t/lib.rs"]),
            upstream(RelationKind::DependsOn, "b", Priority::Task)
        ]
    );
    assert_eq!(
        review.flags(&element_id("b")),
        [
            changed(&["crates/t/lib.rs"]),
            upstream(RelationKind::DependsOn, "a", Priority::Task)
        ]
    );
}

#[test]
fn passes_on_one_flag_per_relation_and_rates_the_highest() {
    let snapshot = snapshot_of(vec![
        node(
            "s",
            None,
            Some(STAMP),
            &[("uses", "t"), ("depends_on", "t")],
        ),
        node("t", Some("crates/t"), Some(STAMP), &[]),
    ]);

    let review = review(&snapshot, &target_changed());

    assert_eq!(
        review.flags(&element_id("s")),
        [
            upstream(RelationKind::DependsOn, "t", Priority::Task),
            upstream(RelationKind::Uses, "t", Priority::Hint)
        ]
    );
    assert_eq!(review.priority(&element_id("s")), Some(Priority::Task));
}
