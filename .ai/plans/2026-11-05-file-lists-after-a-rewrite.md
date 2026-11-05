**Repository:** root
**Status:** InProgress
**Created:** 2026-11-05

## Goal

After a rebase, a squash merge, or a cherry-pick, a stamp is judged by the content it recorded; an
element whose anchor holds another object than recorded gets `UnknownStamp`, which names no file,
and every review then reads the whole Git tree below the project root and keeps every entry. A
flag after a rewrite names the files that changed under the anchor whenever Git still holds the
object the stamp recorded, the review keeps only the tree entries it needs, and four small
cleanups of the same code are done: one path argument in `recorded_matches`, the wording of the
`Upstream` doc and of the `UnknownStamp` reason, and `Display for ObjectId`.

## Context

- The fallback lives in `crates/abyme-core/src/review.rs` (`Baseline::Content`, `own_flags`,
  `passed_on`, `recorded_matches`) and `crates/abyme-core/src/project/git.rs`
  (`repository_facts`, `objects`, `root_tree`, `tree_entries`); `docs/3 Reconciliation/
  Reconciliation.md` ("After a Rewrite") and `docs/Decisions.md` describe it.
- The flags reach the AI through `read_element` and `status`, whose reason text comes from
  `crates/abyme-core/src/outline.rs`.
- Risk categories: a recorded object ID, read from an element file (untrusted input), reaches Git's
  standard input and command line, an escalation trigger handled as Security-Hybrid; Git output
  that names files reaches the AI; new Git commands run under the Security Baseline
  (`docs/5 Architecture/Hexagonal Architecture.md`).

## Steps

- [x] Recorded content is compared by one path argument, and one rule assigns an element to a
      repository
- [ ] A flag after a rewrite names the files that changed under the anchor
- [ ] Review keeps only the tree entries it needs

## Tasks

### Task 1: Recorded content is compared by one path argument, and one rule assigns an element to a repository

`recorded_matches` takes a path twice, as an anchor and as bytes that always equal it, and
`stamps()` in `git.rs` decides inline which elements belong to a repository; Task 2 compares
recorded content again and selects the elements of a repository again, so each moves to one place
first. Behavior is unchanged.

Review path: Security-Hybrid

- [x] `recorded_matches` takes the path once, and `StandingPath::bytes` is gone.
- [x] No place in `git.rs` but one function decides whether an element belongs to the repository at
      a root, and `stamps()` uses it.
- [x] The tests in `crates/abyme-core/src/review/tests.rs`, `crates/abyme-core/src/project/git/
      tests.rs`, and `crates/abyme-core/tests/review.rs` pass with their assertions unchanged.
- [x] The backlog entry about `recorded_matches` taking the path twice is gone.

### Task 2: A flag after a rewrite names the files that changed under the anchor

When an own anchor's recorded object differs from HEAD's, the flag lists the files that differ
between the two objects, together with what the index and the working tree changed under the
anchor. Whether the anchor is a folder or a file is decided by the entry HEAD holds there.

Review path: Security-Hybrid

- [ ] After a rewrite, an anchor where HEAD holds a tree, whose recorded object is a tree in its
      repository and differs from HEAD's tree, is flagged `Changed` with the files that differ,
      plus the staged and working-tree files under it.
- [ ] After a rewrite, an anchor where HEAD holds a file, whose recorded object differs in any way,
      a missing object and one of another length included, is flagged `Changed` with the anchor
      itself, plus the staged and working-tree files under it, by a test of each.
- [ ] Two elements on one anchor with different recorded objects each get the files of their own
      object, by a test.
- [ ] Where HEAD holds a tree, a recorded object that is missing, is no tree, or has another length
      than the repository's object IDs, two trees whose diff names no file, and a failing or
      malformed Git answer for one element leave that element `UnknownStamp` and every other
      element as it would be without it, by a test of each.
- [ ] An anchor where HEAD holds a gitlink, an anchor the stamp recorded as holding nothing while
      HEAD holds an entry there, and an anchor HEAD no longer holds keep their flags of today:
      `UnknownStamp`, plus `AnchorMissing` for the last, by a test of each.
- [ ] Integration tests in temporary repositories show the file lists after a rebase for a folder
      anchor, a file anchor, an anchor equal to a nested repository's root, and a project below the
      top of its repository; a partial clone without its trees fetches nothing and gives a folder
      anchor no file list; and a hostile nested repository runs nothing while its files are listed.
- [ ] No recorded object reaches Git before its full object ID and its type are confirmed through
      Git's standard input, and no anchor or recorded path reaches Git at all.
- [ ] Existing assertions change only where this task turns `UnknownStamp` into `Changed` or
      rewords the `UnknownStamp` reason: in `crates/abyme-core/tests/review.rs` the committed own
      change after a rebase, the stamp on another branch, the nested root, the project below the
      top of its repository, and the partial clone with a file anchor; in
      `crates/abyme-mcp/tests/server/review.rs` the reason line of the element `unknown`.
- [ ] `read_element` and `status` give one reason for every `UnknownStamp` that says the files
      that changed since the stamp cannot be told; the doc comment of `Flag::Changed` names the
      files that differ from the recorded content, that of `Flag::UnknownStamp` says when it
      replaces them, and that of `Flag::Upstream` tells a stamp that holds no entry for the path
      from one that recorded the path as holding nothing.
- [ ] `impl Display for ObjectId` has a caller in the finished code; otherwise it is gone.
- [ ] `docs/3 Reconciliation/Reconciliation.md` ("After a Rewrite"), `docs/Decisions.md`, the
      module doc of `git.rs`, and the Security Baseline in `docs/5 Architecture/Hexagonal
      Architecture.md` tell a reader which value from an element file reaches Git and when a flag
      names files after a rewrite.
- [ ] The backlog entries about the `Flag::Upstream` doc, the `UnknownStamp` reason, and
      `impl Display for ObjectId` are gone.

### Task 3: Review keeps only the tree entries it needs

The objects review reads from `ls-tree` for its fallback (`Fallback::objects`) keep only the paths
that are an anchor of an element or a folder above one; the objects `verify` reads for a stamp
(`HeadFacts::objects`) stay complete.

Review path: Security-Hybrid

- [ ] The objects review keeps hold no path that is neither an anchor nor a folder above one, by a
      unit test on literal `ls-tree` output.
- [ ] A malformed entry anywhere in the output still fails as today, inside and outside the kept
      paths, by a test.
- [ ] Every flag of the tests in `crates/abyme-core/src/review/tests.rs` and
      `crates/abyme-core/tests/review.rs` is as before this task.
- [ ] The content `verify` records is as before: `verify_records_the_content_of_the_anchor_and_of_
      each_target` and `compares_a_nested_root_by_its_tree_and_a_submodule_target_by_its_commit`
      pass with their assertions unchanged.
- [ ] The doc comments of `Fallback::objects`, `HeadFacts::objects`, and the function that reads
      the objects state what each holds.
- [ ] The backlog entry about the whole-tree `ls-tree` in every review is gone.

## Decisions

- **File lists are computed in the Git adapter, inside the facts of each repository:** the adapter
  holds the place in a nested repository, which a second round after review would have to enter
  again; review stays a pure function of the facts.
- **A recorded object reaches Git only after `git cat-file --batch-check` on standard input has
  echoed exactly that full ID as a tree:** Git then takes it neither as an abbreviation nor as
  another object type, and fetches nothing for it.
- **A failing pre-check or `diff-tree`, and Git output that does not fit, leave the element
  `UnknownStamp`, never the whole review unavailable:** an element file that names a broken tree
  must not block review for every user.
- **A file anchor gets its list without a Git call:** the list is the anchor itself, whatever the
  recorded object names.
- **An anchor HEAD no longer holds, an anchor recorded as holding nothing, and an anchor where HEAD
  holds a gitlink get no list:** each lists nothing the stamp vouched for, and a gitlink there
  means the nested repository the stamp vouched for was left unread.
- **One reason text for every `UnknownStamp`:** the flag carries no detail, and the text holds for
  a stamp without recorded content too.
- **Review keeps the entries of anchors and of the folders above them, `verify` keeps all:** they
  cover every path review looks up, an own anchor or the nested root above a target, without the
  relations, while `verify` learns the paths of its targets only after reading the objects.
- **Every `ls-tree` entry is still parsed and checked, also when it is dropped:** a malformed
  entry fails the review wherever it lies, as today.
- **Restamping unchanged elements after a rewrite stays out:** it would end the fallback's cost at
  its cause, but it adds a write path and commits nobody asked for.
