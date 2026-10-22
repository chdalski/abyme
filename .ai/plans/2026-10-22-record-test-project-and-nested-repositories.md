**Repository:** root
**Status:** InProgress
**Created:** 2026-10-22

## Goal

Record in the concept docs that Abyme itself is the main test project
of the MVP, with its boards in the Abyme repository. rlsp stays only for
the history replay and the size test. Since repositories nested in a
project belong to it, narrow the open question "Several repos" to
repositories that lie side by side. Record that change detection across
nested repositories is still open before the MVP.

## Context

- Project discovery already crosses nested Git repositories.

## Steps

- [x] Record Abyme as the main test project
- [x] Narrow "Several repos" and record the open detection point

## Tasks

### Task 1: Record Abyme as the main test project

The MVP is tested first on Abyme's own repository. The AI builds the C4
model and the docs map of Abyme, and the boards are committed in the
Abyme repository with the code they describe.

A local clone of rlsp stays a second test project. It serves the
history replay, which needs a past with many code changes, and the size
test, which needs a larger code base. Its boards stay in that clone,
which is never pushed, and it stays only as long as those two tests
need it.

The test setup is in `docs/5 Architecture/First Slice.md#Test Setup`
and in a row of the "First Slice" section of `docs/Decisions.md`.

Review path: Reviewer-only

- [x] A reader of `First Slice.md#Test Setup` learns:
      - which repository the MVP is tested on first, and that its
        boards are committed there;
      - what the rlsp clone is still used for, that its boards are
        never pushed, and how long it stays.
- [x] The row about the test setup in `Decisions.md` states the
      current setup with its reason, and no row still names rlsp as
      the only test project.

### Task 2: Narrow "Several repos" and record the open detection point

Repositories nested in a project, such as clones and submodules, belong
to the outer Abyme project. That settles part of the open question
"Several repos" in `Storage in Git.md` and `docs/Open Questions.md`.
Change detection in `docs/3 Reconciliation/Reconciliation.md#Detecting
Changes` reads the history of one repository only.

Review path: Reviewer-only

- [x] In `Storage in Git.md` and `Open Questions.md`, "Several repos"
      asks only about repositories that lie side by side.
- [x] A reader of `Reconciliation.md#Detecting Changes` learns, from
      an open point, that changes inside nested repositories are not
      yet detected.
- [x] `Open Questions.md` lists change detection across nested
      repositories under "Before the MVP", with a link to that open
      point.

## Decisions

- **Detection across nested repositories is an open point before the
  MVP,** not a design in this plan: how Abyme reads several histories
  belongs to the Git access, which is itself still open.

## Non-Goals

- Changing change detection or anything else in code
