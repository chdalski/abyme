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
    project_only(RepositoryFacts {
        known: paths(&["crates/x/lib.rs", "docs/a.md"]),
        uncommitted: paths(uncommitted),
        since: BTreeMap::from([(commit(STAMP), Since::Changed(paths(since)))]),
        fallback: None,
    })
}

/// Facts of the project's repository alone.
fn project_only(project: RepositoryFacts) -> GitFacts {
    with_nested(project, Vec::new())
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
    facts.project.since.insert(commit(OTHER), Since::Unknown);

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
    project_only(RepositoryFacts {
        known: paths(&["crates/t/lib.rs", "crates/u/lib.rs", "crates/v/lib.rs"]),
        uncommitted: BTreeSet::new(),
        since: BTreeMap::from([(commit(STAMP), Since::Changed(paths(&["crates/t/lib.rs"])))]),
        fallback: None,
    })
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
    facts.project.since.insert(commit(OTHER), Since::Unknown);

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
    facts.project.uncommitted = paths(&["crates/u/new.rs"]);

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

#[test]
fn passes_nothing_on_for_a_changed_path_above_the_target_that_is_no_repository_root() {
    let snapshot = snapshot_of(vec![
        node("s", None, Some(STAMP), &[("depends_on", "t")]),
        node("t", Some("a/b/c"), Some(STAMP), &[]),
    ]);
    let facts = project_only(repository(&["a/b/c"], &["a/b"]));

    let review = review(&snapshot, &facts);

    assert_eq!(review.flags(&element_id("s")), []);
}

/// The facts of one repository in which Git knows `known` and the stamp
/// `STAMP` is known with `changed` since.
fn repository(known: &[&str], changed: &[&str]) -> RepositoryFacts {
    RepositoryFacts {
        known: paths(known),
        uncommitted: BTreeSet::new(),
        since: BTreeMap::from([(commit(STAMP), Since::Changed(paths(changed)))]),
        fallback: None,
    }
}

/// The facts of the project's repository and of the nested ones, by root.
fn with_nested(project: RepositoryFacts, nested: Vec<(&str, RepositoryFacts)>) -> GitFacts {
    GitFacts {
        project,
        nested: nested
            .into_iter()
            .map(|(root, facts)| (root.as_bytes().to_vec(), facts))
            .collect(),
    }
}

/// Facts with the nested repositories `sub`, `sub/inner`, and
/// `vendor/lib`, each knowing `STAMP`. Since it, in the project the root
/// `sub` moved, `crates/t/lib.rs` changed, and the file `a/b` was removed;
/// in `sub` the root `sub/inner` moved and `sub/a.rs` changed; in
/// `vendor/lib` the file `vendor/lib/x.rs` changed.
fn changes_in_every_repository() -> GitFacts {
    with_nested(
        repository(
            &["crates/t/lib.rs", "sub", "vendor/lib"],
            &["sub", "crates/t/lib.rs", "a/b"],
        ),
        vec![
            (
                "sub",
                repository(
                    &["sub/a.rs", "sub/s.rs", "sub/inner"],
                    &["sub/a.rs", "sub/inner"],
                ),
            ),
            ("sub/inner", repository(&["sub/inner/c.rs"], &[])),
            (
                "vendor/lib",
                repository(
                    &["vendor/lib/s.rs", "vendor/lib/x.rs"],
                    &["vendor/lib/x.rs"],
                ),
            ),
        ],
    )
}

#[rstest]
#[case::the_deepest_root("sub/inner/c.rs", Some("sub/inner"))]
#[case::a_root_itself("sub/inner", Some("sub/inner"))]
#[case::the_outer_root("sub/a.rs", Some("sub"))]
#[case::a_sibling_with_the_same_start("sub2/x", None)]
#[case::an_anchor_that_contains_a_root("vendor", None)]
#[case::outside_every_root("crates/t", None)]
fn belongs_to_the_deepest_repository_its_anchor_lies_in(
    #[case] anchor: &str,
    #[case] root: Option<&str>,
) {
    let facts = changes_in_every_repository();
    let anchor = Anchor::try_from(anchor.to_owned()).unwrap();

    assert_eq!(
        repository_root(facts.nested.keys(), Some(&anchor)),
        root.map(str::as_bytes)
    );
}

#[test]
fn belongs_to_the_project_repository_without_an_anchor() {
    assert_eq!(
        repository_root(changes_in_every_repository().nested.keys(), None),
        None
    );
}

#[test]
fn belongs_to_no_repository_whose_root_is_empty_or_ends_with_a_slash() {
    let facts = with_nested(
        repository(&[], &[]),
        vec![("", repository(&[], &[])), ("sub/", repository(&[], &[]))],
    );
    let anchor = Anchor::try_from("sub/x".to_owned()).unwrap();

    assert_eq!(repository_root(facts.nested.keys(), Some(&anchor)), None);
}

#[test]
fn judges_each_element_by_the_facts_of_its_own_repository() {
    let snapshot = snapshot_of(vec![
        node("in-sub", Some("sub"), Some(STAMP), &[]),
        node("in-inner", Some("sub/inner"), Some(STAMP), &[]),
        node("in-project", Some("crates/t"), Some(STAMP), &[]),
        node("sibling", Some("sub2"), Some(STAMP), &[]),
    ]);
    let mut facts = changes_in_every_repository();
    facts.project.known.insert(b"sub2/b.rs".to_vec());
    facts.project.uncommitted.insert(b"sub2/b.rs".to_vec());

    let review = review(&snapshot, &facts);

    assert_eq!(
        review.flags(&element_id("in-sub")),
        [changed(&["sub/a.rs", "sub/inner"])]
    );
    assert_eq!(review.flags(&element_id("in-inner")), []);
    assert_eq!(
        review.flags(&element_id("in-project")),
        [changed(&["crates/t/lib.rs"])]
    );
    assert_eq!(
        review.flags(&element_id("sibling")),
        [changed(&["sub2/b.rs"])]
    );
}

#[rstest]
#[case::nothing_changed_in_its_repository(&[], vec![])]
#[case::the_nested_root_moved(&["vendor/lib"], vec![changed(&["vendor/lib"])])]
fn judges_an_anchor_that_contains_a_nested_repository_only_by_its_root(
    #[case] changed_in_project: &[&str],
    #[case] expected: Vec<Flag>,
) {
    let snapshot = snapshot_of(vec![node("vendor", Some("vendor"), Some(STAMP), &[])]);
    let facts = with_nested(
        repository(&["vendor/README.md", "vendor/lib"], changed_in_project),
        vec![(
            "vendor/lib",
            RepositoryFacts {
                uncommitted: paths(&["vendor/lib/src/b.rs"]),
                ..repository(&["vendor/lib/src/a.rs"], &["vendor/lib/src/a.rs"])
            },
        )],
    );

    let review = review(&snapshot, &facts);

    assert_eq!(review.flags(&element_id("vendor")), expected.as_slice());
}

#[test]
fn reads_a_stamp_only_in_the_elements_repository() {
    let snapshot = snapshot_of(vec![
        node("in-sub", Some("sub/a.rs"), Some(STAMP), &[]),
        node("in-project", Some("docs/a.md"), Some(OTHER), &[]),
    ]);
    let facts = with_nested(
        repository(&["docs/a.md", "sub"], &[]),
        vec![(
            "sub",
            RepositoryFacts {
                since: BTreeMap::from([(commit(OTHER), Since::Changed(BTreeSet::new()))]),
                ..repository(&["sub/a.rs"], &[])
            },
        )],
    );

    let review = review(&snapshot, &facts);

    assert_eq!(review.flags(&element_id("in-sub")), [Flag::UnknownStamp]);
    assert_eq!(
        review.flags(&element_id("in-project")),
        [Flag::UnknownStamp]
    );
}

#[rstest]
#[case::within_the_project(None, "crates/t", true)]
#[case::to_a_nested_repository_whose_root_moved(None, "sub/a.rs", true)]
#[case::to_a_repository_nested_below_a_moved_root(None, "sub/inner/c.rs", true)]
#[case::to_a_nested_repository_whose_root_did_not_move(None, "vendor/lib/x.rs", false)]
#[case::to_a_path_below_a_removed_file(None, "a/b/c", false)]
#[case::within_a_nested_repository(Some("vendor/lib/s.rs"), "vendor/lib/x.rs", true)]
#[case::to_a_repository_nested_in_the_sources_whose_root_moved(
    Some("sub/s.rs"),
    "sub/inner/c.rs",
    true
)]
#[case::from_a_nested_repository_to_a_target_outside(Some("vendor/lib/s.rs"), "crates/t", false)]
#[case::from_a_nested_repository_to_a_target_whose_anchor_contains_it(
    Some("vendor/lib/s.rs"),
    "vendor",
    false
)]
fn measures_a_passed_on_flag_in_the_sources_repository(
    #[case] source: Option<&str>,
    #[case] target: &str,
    #[case] passes: bool,
) {
    let snapshot = snapshot_of(vec![
        node("s", source, Some(STAMP), &[("depends_on", "t")]),
        node("t", Some(target), Some(STAMP), &[]),
    ]);

    let review = review(&snapshot, &changes_in_every_repository());

    let passed_on: Vec<Flag> = review
        .flags(&element_id("s"))
        .iter()
        .filter(|flag| matches!(flag, Flag::Upstream { .. }))
        .cloned()
        .collect();
    let expected: Vec<Flag> = passes
        .then(|| upstream(RelationKind::DependsOn, "t", Priority::Task))
        .into_iter()
        .collect();
    assert_eq!(passed_on, expected);
}

/// HEAD at `OTHER` with the given paths changed since.
fn head(changed: &[&str]) -> HeadFacts {
    HeadFacts {
        commit: Some(commit(OTHER)),
        changed: paths(changed),
        root: None,
        roots: BTreeSet::new(),
        objects: BTreeMap::new(),
    }
}

#[rstest]
#[case::no_change(&[])]
#[case::change_elsewhere(&["crates/other/lib.rs"])]
#[case::sibling_with_the_same_start(&["crates/xy/lib.rs"])]
fn stamps_an_element_whose_anchor_has_no_change(#[case] changed: &[&str]) {
    let snapshot = snapshot(element(Some("crates/x"), Some(STAMP)));

    let (stamped, commit) =
        stamped(&snapshot, &element_id("x"), &head(changed), Verifier::Ai).unwrap();

    assert_eq!(commit, self::commit(OTHER));
    let before = snapshot.elements.get(&element_id("x")).unwrap();
    assert_eq!(
        stamped.verified(),
        Some(&Verification {
            commit: self::commit(OTHER),
            by: Verifier::Ai,
            content: Some(RecordedContent::from([(
                Anchor::try_from("crates/x".to_owned()).unwrap(),
                None
            )])),
        })
    );
    assert_eq!(
        Element::new(
            element_id("x"),
            abyme_model::ElementFields {
                verified: before.verified().cloned(),
                ..stamped.into_fields()
            }
        )
        .unwrap(),
        before.clone()
    );
}

#[rstest]
#[case::changed_file(&["crates/x/lib.rs"])]
#[case::the_anchor_itself(&["crates/x"])]
#[case::unmerged_or_untracked_file_below(&["crates/x/deep/new.rs"])]
fn refuses_an_element_with_a_change_under_its_anchor(#[case] changed: &[&str]) {
    let snapshot = snapshot(element(Some("crates/x"), None));

    let refused = stamped(&snapshot, &element_id("x"), &head(changed), Verifier::Ai);

    assert_eq!(refused, Err(VerifyProblem::Uncommitted(element_id("x"))));
}

#[rstest]
#[case::without_an_anchor(None)]
#[case::with_an_anchor_that_matches_no_file(Some("crates/gone"))]
fn stamps_an_element_without_anything_under_its_anchor(#[case] anchor: Option<&str>) {
    let snapshot = snapshot(element(anchor, None));

    let stamped = stamped(
        &snapshot,
        &element_id("x"),
        &head(&["docs/a.md"]),
        Verifier::Ai,
    );

    assert!(stamped.is_ok(), "{stamped:?}");
}

#[test]
fn refuses_an_element_without_a_file_and_a_repository_without_a_commit() {
    let snapshot = snapshot(element(None, None));

    assert_eq!(
        stamped(&snapshot, &element_id("y"), &head(&[]), Verifier::Ai),
        Err(VerifyProblem::UnknownElement(element_id("y")))
    );
    assert_eq!(
        stamped(
            &snapshot,
            &element_id("x"),
            &HeadFacts::default(),
            Verifier::Ai
        ),
        Err(VerifyProblem::NoCommit(element_id("x")))
    );
}

#[test]
fn names_only_the_element_when_it_refuses() {
    let problems = [
        VerifyProblem::UnknownElement(element_id("x")),
        VerifyProblem::NoCommit(element_id("x")),
        VerifyProblem::Uncommitted(element_id("x")),
        VerifyProblem::Unavailable {
            element: element_id("x"),
            reason: Unavailable::GitFailed,
        },
        VerifyProblem::Rejected {
            element: element_id("x"),
            problem: ElementProblem::FrontmatterTooLarge { size: 65_600 },
        },
    ];

    let messages: Vec<String> = problems.iter().map(ToString::to_string).collect();

    assert_eq!(
        messages,
        [
            "no element has the ID x",
            "element x cannot be verified: the repository has no commit yet",
            "element x cannot be verified: files under its anchor differ from HEAD or are untracked; they must be committed first",
            "element x cannot be verified: git failed",
            "element x cannot be verified: the frontmatter has 65600 bytes, more than 65536",
        ]
    );
}

#[test]
fn refuses_a_stamp_that_would_make_the_frontmatter_too_large() {
    let with_summary = |length: usize| {
        abyme_model::ElementFixture::new("x")
            .name("X")
            .summary(Some(&"a".repeat(length)))
            .try_build()
    };
    let largest = (abyme_model::MAX_FRONTMATTER_BYTES - 200..=abyme_model::MAX_FRONTMATTER_BYTES)
        .rev()
        .find(|length| with_summary(*length).is_ok())
        .unwrap();
    let snapshot = snapshot(with_summary(largest).unwrap());

    let refused = stamped(&snapshot, &element_id("x"), &head(&[]), Verifier::Ai);

    assert!(
        matches!(
            refused,
            Err(VerifyProblem::Rejected {
                ref element,
                problem: ElementProblem::FrontmatterTooLarge { .. },
            }) if *element == element_id("x")
        ),
        "{refused:?}"
    );
}

#[test]
fn a_stamp_keeps_the_statement() {
    let text = "---\nid: x\ntype: t\nname: X\nstatement: question\n---\n";
    let element = Element::from_markdown(&element_id("x"), text).unwrap();
    let snapshot = snapshot(element);

    let (stamped, _) = stamped(&snapshot, &element_id("x"), &head(&[]), Verifier::Ai).unwrap();

    assert_eq!(
        stamped.statement(),
        Some(abyme_model::StatementKind::Question)
    );
}

const TREE_X: &str = "1111111111111111111111111111111111111111";
const TREE_Y: &str = "2222222222222222222222222222222222222222";

/// The element `id`, anchored on `anchor` or on nothing, with a relation of
/// each kind to each target of `targets`, and a stamp `STAMP` by the AI
/// that recorded `content` lines.
fn recorded_element(
    id: &str,
    anchor: Option<&str>,
    content: &str,
    targets: &[(&str, &str)],
) -> Element {
    let anchor = anchor.map_or(String::new(), |anchor| format!("anchor: \"{anchor}\"\n"));
    let relations: String = targets
        .iter()
        .map(|(kind, target)| format!("  - {{kind: {kind}, to: {target}}}\n"))
        .collect::<Vec<_>>()
        .concat();
    let relations = if relations.is_empty() {
        relations
    } else {
        format!("relations:\n{relations}")
    };
    Element::from_markdown(
        &element_id(id),
        &format!(
            "---\nid: {id}\ntype: t\nname: X\n{anchor}verified: \"{STAMP}\"\nverified_by: ai\n\
             {content}{relations}---\n"
        ),
    )
    .unwrap()
}

/// The element `y`, anchored on `crates/y`, without a stamp.
fn target_y() -> Element {
    Element::from_markdown(
        &element_id("y"),
        "---\nid: y\ntype: t\nname: Y\nanchor: crates/y\n---\n",
    )
    .unwrap()
}

/// Facts in which `STAMP` tells nothing and HEAD holds `crates/x` as
/// `TREE_X` and `crates/y` as `TREE_Y`, with `staged` and `uncommitted`
/// paths; without fallback facts when `fallback` is false.
fn rewritten(staged: &[&str], uncommitted: &[&str], fallback: bool) -> GitFacts {
    project_only(RepositoryFacts {
        known: paths(&["crates/x/lib.rs", "crates/y/lib.rs"]),
        uncommitted: paths(uncommitted),
        since: BTreeMap::from([(commit(STAMP), Since::Unknown)]),
        fallback: fallback.then(|| Fallback {
            objects: BTreeMap::from([
                (
                    b"crates/x".to_vec(),
                    ObjectId::try_from(TREE_X.to_owned()).unwrap(),
                ),
                (
                    b"crates/y".to_vec(),
                    ObjectId::try_from(TREE_Y.to_owned()).unwrap(),
                ),
            ]),
            staged: paths(staged),
        }),
    })
}

fn flags_in(elements: Vec<Element>, facts: &GitFacts, id: &str) -> Vec<Flag> {
    review(&snapshot_of(elements), facts)
        .flags(&element_id(id))
        .to_vec()
}

fn upstream_from_y() -> Flag {
    Flag::Upstream {
        kind: RelationKind::DependsOn,
        target: element_id("y"),
        priority: Priority::Task,
    }
}

#[rstest]
#[case::unchanged(&format!("verified_content:\n  - {{path: crates/x, object: \"{TREE_X}\"}}\n"), &[], &[], vec![])]
#[case::staged(
    &format!("verified_content:\n  - {{path: crates/x, object: \"{TREE_X}\"}}\n"),
    &["crates/x/lib.rs"],
    &[],
    vec![Flag::Changed(vec![b"crates/x/lib.rs".to_vec()])]
)]
#[case::in_the_working_tree(
    &format!("verified_content:\n  - {{path: crates/x, object: \"{TREE_X}\"}}\n"),
    &[],
    &["crates/x/new.rs"],
    vec![Flag::Changed(vec![b"crates/x/new.rs".to_vec()])]
)]
#[case::another_object(
    &format!("verified_content:\n  - {{path: crates/x, object: \"{TREE_Y}\"}}\n"),
    &[],
    &[],
    vec![Flag::UnknownStamp]
)]
#[case::an_object_of_the_other_length(
    &format!("verified_content:\n  - {{path: crates/x, object: \"{}\"}}\n", "1".repeat(64)),
    &[],
    &[],
    vec![Flag::UnknownStamp]
)]
#[case::held_nothing_then(
    "verified_content:\n  - {path: crates/x}\n",
    &[],
    &[],
    vec![Flag::UnknownStamp]
)]
#[case::no_entry_for_the_anchor(
    &format!("verified_content:\n  - {{path: crates/y, object: \"{TREE_Y}\"}}\n"),
    &[],
    &[],
    vec![Flag::UnknownStamp]
)]
#[case::no_recorded_content("", &[], &[], vec![Flag::UnknownStamp])]
fn judges_the_own_anchor_by_recorded_content_after_a_rewrite(
    #[case] content: &str,
    #[case] staged: &[&str],
    #[case] uncommitted: &[&str],
    #[case] expected: Vec<Flag>,
) {
    let element = recorded_element("x", Some("crates/x"), content, &[]);

    assert_eq!(
        flags_in(vec![element], &rewritten(staged, uncommitted, true), "x"),
        expected
    );
}

#[test]
fn flags_an_unusable_stamp_with_recorded_content_when_the_facts_hold_no_content() {
    let element = recorded_element(
        "x",
        Some("crates/x"),
        &format!("verified_content:\n  - {{path: crates/x, object: \"{TREE_X}\"}}\n"),
        &[],
    );

    assert_eq!(
        flags_in(vec![element], &rewritten(&[], &[], false), "x"),
        [Flag::UnknownStamp]
    );
}

#[rstest]
#[case::unchanged(&format!("  - {{path: crates/y, object: \"{TREE_Y}\"}}\n"), &[], &[], vec![])]
#[case::another_object(&format!("  - {{path: crates/y, object: \"{TREE_X}\"}}\n"), &[], &[], vec![upstream_from_y()])]
#[case::not_recorded("", &[], &[], vec![upstream_from_y()])]
#[case::staged(
    &format!("  - {{path: crates/y, object: \"{TREE_Y}\"}}\n"),
    &["crates/y/lib.rs"],
    &[],
    vec![upstream_from_y()]
)]
#[case::in_the_working_tree(
    &format!("  - {{path: crates/y, object: \"{TREE_Y}\"}}\n"),
    &[],
    &["crates/y/lib.rs"],
    vec![upstream_from_y()]
)]
fn passes_a_flag_on_by_recorded_content_after_a_rewrite(
    #[case] target_entry: &str,
    #[case] staged: &[&str],
    #[case] uncommitted: &[&str],
    #[case] expected: Vec<Flag>,
) {
    let content = format!("verified_content:\n{target_entry}");
    let content = if target_entry.is_empty() {
        "verified_content: []\n".to_owned()
    } else {
        content
    };
    let source = recorded_element("x", None, &content, &[("depends_on", "y")]);

    assert_eq!(
        flags_in(
            vec![source, target_y()],
            &rewritten(staged, uncommitted, true),
            "x"
        ),
        expected
    );
}

#[test]
fn passes_nothing_on_by_recorded_content_from_a_target_outside_the_source_s_repository() {
    let source = recorded_element(
        "x",
        Some("sub/x"),
        &format!("verified_content:\n  - {{path: sub/x, object: \"{TREE_X}\"}}\n"),
        &[("depends_on", "y")],
    );
    let mut facts = rewritten(&[], &[], true);
    facts.nested.insert(
        b"sub".to_vec(),
        RepositoryFacts {
            known: paths(&["sub/x/lib.rs"]),
            uncommitted: BTreeSet::new(),
            since: BTreeMap::from([(commit(STAMP), Since::Unknown)]),
            fallback: Some(Fallback {
                objects: BTreeMap::from([(
                    b"sub/x".to_vec(),
                    ObjectId::try_from(TREE_X.to_owned()).unwrap(),
                )]),
                staged: BTreeSet::new(),
            }),
        },
    );

    assert_eq!(flags_in(vec![source, target_y()], &facts, "x"), []);
}

#[test]
fn records_the_own_anchor_and_each_target_that_passes_something_on() {
    let source = recorded_element(
        "x",
        Some("crates/x"),
        "",
        &[("depends_on", "y"), ("depends_on", "z"), ("see_also", "v")],
    );
    let not_anchored =
        Element::from_markdown(&element_id("z"), "---\nid: z\ntype: t\nname: Z\n---\n").unwrap();
    let seen_also = Element::from_markdown(
        &element_id("v"),
        "---\nid: v\ntype: t\nname: V\nanchor: crates/v\n---\n",
    )
    .unwrap();
    let snapshot = snapshot_of(vec![source, target_y(), not_anchored, seen_also]);
    let head = HeadFacts {
        objects: BTreeMap::from([
            (
                b"crates/x".to_vec(),
                ObjectId::try_from(TREE_X.to_owned()).unwrap(),
            ),
            (
                b"crates/v".to_vec(),
                ObjectId::try_from(TREE_Y.to_owned()).unwrap(),
            ),
        ]),
        ..head(&[])
    };

    let (stamped, _) = stamped(&snapshot, &element_id("x"), &head, Verifier::Ai).unwrap();

    let anchor = |path: &str| Anchor::try_from(path.to_owned()).unwrap();
    assert_eq!(
        stamped.verified().and_then(|stamp| stamp.content.clone()),
        Some(RecordedContent::from([
            (
                anchor("crates/x"),
                Some(ObjectId::try_from(TREE_X.to_owned()).unwrap())
            ),
            (anchor("crates/y"), None),
        ]))
    );
}
