**Repository:** root
**Status:** InProgress
**Created:** 2026-11-04

## Goal

A verification stamp names a commit, so a rebase, a squash merge, or a cherry-pick that rewrites
that commit flags every stamped element even when nothing under any anchor changed, and each
rewrite then needs a commit that only stamps again. Stamps record the content they vouch for, so
that after a rewrite an element stays unflagged while the content under its anchor and under its
relation targets' anchors is unchanged, and is flagged when it changed.

## Context

- Today a stamp is `verified: <commit>` with `verified_by` (`crates/abyme-model/src/verification.rs`,
  the element file format in `docs/5 Architecture/Storage in Git.md`). Review flags
  (`crates/abyme-core/src/review.rs`) measure an element's own anchor and its relation targets'
  anchors against that commit, from facts that `crates/abyme-core/src/project/git.rs` gathers; a
  commit that Git does not know or that is no ancestor of HEAD gives `UnknownStamp` and passes
  nothing on, which `docs/3 Reconciliation/Reconciliation.md` ("Limits") states as the design.
- The readers of a stamp besides review are `outline` (the `Verified:` line of `read_element`) and
  the facts that `Project` asks Git for.
- Risk categories: element files are untrusted input, and the stamp gains a field read from them; new
  Git commands run under the Security Baseline (`docs/5 Architecture/Hexagonal Architecture.md`); a
  recorded content entry, like a stamp's commit today, can suppress flags if forged.

## Steps

- [x] Review flags resolve a stamp's baseline and a relation target's path in one place each
- [x] Element files read lists of flat mappings through one reader
- [ ] Element files carry the content a stamp vouches for
- [ ] `verify` records the content, and review falls back to it after a rewrite

## Tasks

### Task 1: Review flags resolve a stamp's baseline and a relation target's path in one place each

`own_flags` and `passed_on` each look up what changed since the element's stamp and handle an
unusable commit their own way, and `passed_on` decides inline which path stands for a relation
target. Task 4 changes both rules, so each moves to one place first. Behavior is unchanged.

Review path: Reviewer-only

- [x] The baseline of an element's stamp, what changed since its commit or that the commit is
      unusable, is resolved once per element and used by both the own flags and the flags relations
      pass on.
- [x] One function decides which path, if any, stands for a relation target in the source's
      repository: the target's anchor in the same repository, the root of a nested repository the
      anchor lies in, or none.
- [x] The tests in `crates/abyme-core/src/review/tests.rs` and `crates/abyme-core/tests/review.rs`
      pass with their assertions unchanged.

### Task 2: Element files read lists of flat mappings through one reader

The reader of `relations` checks a fixed-depth list of flat mappings for duplicate, unknown, and
non-scalar keys; Task 3 adds a second such list, which reuses these checks instead of copying them.
Behavior is unchanged.

Review path: Security-Hybrid

- [x] Reading a list of flat mappings with its key checks lives in one place, and `relations` reads
      through it.
- [x] The tests in `crates/abyme-model/src/element/tests.rs` and
      `crates/abyme-model/tests/element_properties.rs` pass with their assertions unchanged.

### Task 3: Element files carry the content a stamp vouches for

A stamp holds, besides its commit, the Git object ID of each path it vouches for: the element's own
anchor and the path that stands for each relation target. Nothing writes the field until Task 4;
this task makes it readable, writable, and removed with the stamp.

Review path: Security-Hybrid

- [ ] An element file with a stamp and recorded content reads and writes back unchanged, and the
      round-trip property tests cover it.
- [ ] A reader can tell a stamp that recorded content from one that did not, a stamp that recorded
      no path from one that recorded no content, and a path recorded as holding nothing at the
      stamp's commit from a path not recorded; each pair reads and writes back distinct, by a test.
- [ ] An object ID that is not 40 or 64 lowercase hex digits, a path recorded twice, recorded
      content without `verified`, and an entry beyond a flat mapping are rejected with their own
      problem, each by a test.
- [ ] An element file without recorded content reads as before.
- [ ] Every `apply` operation that removes a stamp removes the recorded content with it.
- [ ] `docs/5 Architecture/Storage in Git.md` and `docs/5 Architecture/Data Model.md` describe the
      field as the files hold it.

### Task 4: `verify` records the content, and review falls back to it after a rewrite

`verify` records the object ID of each path the stamp vouches for at HEAD, and review compares them
with HEAD whenever the stamp's commit is unusable, so a rebase or a squash merge leaves unchanged
elements unflagged.

Review path: Security-Hybrid

- [ ] `verify` records the element's own anchor and the path that stands for each relation target
      whose kind passes something on, as Task 1's function decides it, each with its object ID at
      HEAD or as holding nothing.
- [ ] With a stamp whose commit is known and an ancestor of HEAD, the flags are those of today: the
      existing tests of review, `verify`, and the MCP tools pass, every flag and result assertion
      unchanged; the only assertions edited are those on the stamped file's text that account for
      the recorded content, such as in `verify_stamps_head_by_ai_and_changes_nothing_else`.
- [ ] Integration tests in temporary repositories show, for elements stamped by `verify` with a
      relation between them:
      - after a rebase, a squash merge, and a cherry-pick that change nothing under their anchors,
        `status` reports no flag;
      - after such a rewrite, a staged change and a working-tree change under the element's anchor
        flag it `Changed` with their files, a commit that changes the content under its anchor flags
        it `UnknownStamp`, and a change under a target's anchor passes the relation's flag on;
      - a stamp whose commit is known but no ancestor of HEAD is judged by its recorded content;
      - after a rewrite, a relation target anchored or re-anchored after the source's stamp passes
        the relation's flag on;
      - after a rewrite, an element without an anchor whose stamp recorded no path is unflagged;
      - an element whose stamp recorded no content, with or without an anchor, is flagged
        `UnknownStamp` after a rewrite and passes nothing on, as today.
- [ ] After a rewrite, an anchor that equals the root of a nested repository and a target inside a
      submodule are compared in the repository the rule of Task 1 names, each by a test.
- [ ] No anchor or recorded path reaches Git's command line.
- [ ] `docs/3 Reconciliation/Reconciliation.md`, `docs/3 Reconciliation/Checks.md`, and
      `docs/Decisions.md` tell a reader what a stamp records and how flags are computed after a
      rewrite; no doc says that a rewrite always makes an element be checked again from scratch.

## Decisions

- **Recorded content beside the commit, not instead of it:** the commit keeps today's flags and file
  lists for un-rewritten history without another Git call, and stamps without recorded content keep
  working.
- **The recorded content lives inside the stamp, keyed by path:** every `apply` change that removes
  the stamp removes it too, so it never outlives the text it vouches for; one path covers several
  relations to one target and targets that share an anchor.
- **After a rewrite, a stamp with recorded content falls back to it also when its commit is known but
  no ancestor of HEAD:** such a commit gives no usable baseline either.
- **A stamp without recorded content keeps today's result after a rewrite:** `UnknownStamp` and
  nothing passed on; comparing it would pass a flag on along every relation.
- **An element without an anchor whose stamp recorded no path is unflagged after a rewrite:** nothing
  it vouches for can have changed.
- **A relation target whose path the stamp did not record passes the relation's flag on after a
  rewrite:** the target was anchored or re-anchored after the stamp, so what it stands for changed.
  While the stamp's commit is usable, such a target is measured as today, by the files under its
  anchor that changed since the commit: there the commit, not the recorded content, is the baseline.
- **An element whose stamp would exceed the frontmatter's size limit is refused by `verify`:** the
  recorded content makes the frontmatter larger, and the existing refusal applies.
- **After a rewrite, an own anchor whose recorded object differs from HEAD flags the element
  `UnknownStamp`:** without the files that changed, the element is checked from scratch; a change in
  the index or the working tree under an anchor whose object matches HEAD is flagged with its files.
- **Task 2 adds relation cases that pin the reader's check order:** a non-scalar value before an
  unknown key, one item read after the other, and a tag on a plain item, so that the shared reader
  keeps the order of the one it replaces.
- **The project format stays 1:** the new field is optional, and an older Abyme rejects only element
  files that carry it.

## Non-Goals

- The files that changed under an anchor after a rewrite, through `git diff-tree` between the
  recorded and the current object (separate plan after this one).
- Restamping elements whose stamps predate this plan: the elements a later task flags get new stamps
  through the rule in the root `CLAUDE.md`.
