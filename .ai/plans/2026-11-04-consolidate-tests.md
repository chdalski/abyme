**Repository:** root
**Status:** InProgress
**Created:** 2026-11-04

## Goal

Three test files grew with every plan until a topic is hard to find in them and a new field of
`ElementFields` means editing hand-written literals in many places: `crates/abyme-core/src/apply/tests.rs`,
`crates/abyme-core/src/outline/tests.rs`, and `crates/abyme-mcp/tests/server.rs`. Give tests a
builder for elements, remove the duplicated setup in these files, and split each into modules by
topic, so that every test keeps what it checks, and fix the small backlog entries on these files on
the way.

## Context

- Every task keeps what each test checks; only the form changes, and no assertion changes except
  where a criterion says a test gets stricter. Each task shows this with the test names: the output of
  `cargo test --workspace -- --list` before and after the task, with the module path and rstest's
  `case_NN_` numbering normalized away, differs only in names the task's handoff maps to their
  replacement.
- A fixture or helper that a task replaces builds the same value as the code it replaces; the
  handoff reports a one-off comparison of old and new value, run in a scratch test that is deleted
  afterwards.
- Risk categories: none; every task changes test code or code that only tests compile.

## Steps

- [x] Tests build elements with a builder
- [x] The backlog's assertions on these files are tightened
- [x] The apply tests share their setup
- [x] The apply tests are split by topic
- [x] The outline tests are split by topic
- [x] The server tests share their session steps
- [x] The server tests are split by topic

## Tasks

### Task 1: Tests build elements with a builder

Tests of several crates build an `Element` from a full `ElementFields` literal, so a new field
means editing each of them. A builder in `abyme-model`, behind a feature that only dev-dependencies
turn on, gives them one place that follows `ElementFields`.

Review path: Reviewer-only

- [x] `abyme-model` offers a builder for elements behind a feature that no non-dev dependency
      turns on, and `cargo build --workspace` compiles none of it.
- [x] Every full `ElementFields` literal that only sets up a test, in a helper or inside a test
      body, builds its element through the builder; literals stay where `Element::new` or
      `ElementFields` is the subject under test, in property-test generators, in struct updates of
      an existing element, and in production code.
- [x] The Conventions of the root `CLAUDE.md` tell a reader which commands compile the feature,
      as they do for `ts`.

### Task 2: The backlog's assertions on these files are tightened

Three entries of `.ai/backlog.md` under Tests concern these files: a detach test that never shows
the pin survives, an anchor test that accepts any invalid element, and a doc comment that calls an
unpinned note pinned.

Review path: Reviewer-only

- [x] `detaching_keeps_where_the_note_lies_and_the_relation_and_writes_no_element` detaches a
      pinned note and shows that the pin stays.
- [x] `the_anchor_of_those_requests_is_invalid_on_its_own` accepts only an invalid anchor.
- [x] The doc comment of `a_note_beside_the_element` says how the request places the note.
- [x] For each tightened assertion, the handoff names a change of production code that the new
      assertion catches and the old one did not, applied and reverted.
- [x] The three entries leave `.ai/backlog.md`.

### Task 3: The apply tests share their setup

`apply/tests.rs` finds a written element in three ways, reaches a board's placements through the
same chain of calls many times, and builds boards by hand that its board helper already builds; one
test repeats a case of another with the same input and assertion.

Review path: Reviewer-only

- [x] One helper finds a written element and one reaches a board's placements; no test of the file
      repeats either inline.
- [x] A hand-built board that the board helper builds identically comes from the helper.
- [x] `place_refuses_an_element_the_request_deleted` is gone, and the handoff maps it to the case
      of `knows_a_deleted_element_no_longer` with the same input and assertion; the backlog entry on
      the unreachable `UnknownElement` of `place_element` names that case instead.
- [x] Every other test of the file keeps its name and its assertions.

### Task 4: The apply tests are split by topic

The tests of `apply` cover requests, IDs, placement, moves and pins, relations, updates, deletion,
stamps, owners and inner boards, layout, statements, notes, and attachments; each topic becomes a
module of its own, so that a reader finds a topic by its file.

Review path: Reviewer-only

- [x] The tests of `apply` lie in one module per topic, and the root test module holds only the
      helpers that two or more of them use.
- [x] The move changes no test body: `git diff --color-moved=dimmed-zebra
      --color-moved-ws=allow-indentation-change` shows as new only `use` and `mod` lines, lint
      attributes, and one doc line per module.
- [x] Each expectation of `clippy::disallowed_fields` and `clippy::disallowed_methods` stands on
      exactly the modules or tests that read the disallowed fields or call the disallowed method.

### Task 5: The outline tests are split by topic

The tests of `outline` cover a board's outline, escaping, the board list, geometry, relations, an
element's detail, review markers, the hierarchy, the overview, and statement kinds; each becomes a
module of its own.

Review path: Reviewer-only

- [x] The tests of `outline` lie in one module per topic, and the root test module holds only the
      helpers that two or more of them use.
- [x] The move changes no test body, by the same `git diff --color-moved` check as the apply split.

### Task 6: The server tests share their session steps

Most tests of `server.rs` open a project and a session in the same two lines, end with the same
check of the protocol, and check every call's error flag by hand; some setup calls never check it.
Each test starts a server process, so they share helpers rather than become parameterized cases.

Review path: Reviewer-only

- [x] Helpers open a project with a session, end a session with the protocol check, and make a
      call that must succeed or must be refused; no test repeats these steps inline.
- [x] Every setup call checks that it succeeded.
- [x] Every test of the file keeps its name and its other assertions.

### Task 7: The server tests are split by topic

The tests of the MCP server cover the command line, the protocol, the environment, boards,
elements, review, notes, and search; each becomes a module of one integration test binary, so that
the split adds no link step.

Review path: Reviewer-only

- [x] The tests of the server lie in one module per topic of one integration test target, still
      run by `cargo test --workspace --test server`, and its root holds the session type and the
      helpers that two or more modules use.
- [x] The move changes no test body, by the same `git diff --color-moved` check as the apply split.
- [x] The backlog entry that asks for this consolidation leaves `.ai/backlog.md`.

## Decisions

- **The builder lives in `abyme-model`, behind a feature:** it changes in the same crate and commit
  as `ElementFields`; `abyme-test-support` holds Git helpers and depends on no crate of the
  workspace.
- **Only full literals that set up a test switch to the builder, inside a test body too:** where
  `Element::new` or `ElementFields` is under test, in property generators, and in struct updates of
  an existing element, the literal shows what is tested or breaks on no new field.
- **Each file is cleaned up in place first and then moved:** a move commit changes no test body, so
  its review is mechanical, and only the small cleanup diffs need reading. `outline/tests.rs` has
  too little duplicated setup for a cleanup task of its own.
- **The server tests stay one integration test binary:** each binary adds a link step, which the
  container's memory limit makes costly, and helpers shared between binaries warn as dead code in
  each that skips one.
- **Setup calls in the server tests check that they succeeded:** a failed setup step then fails
  with its own message instead of a follow-on error further down; the tests get stricter, not
  weaker.
- **Similar refusal tables of different topics stay apart:** each belongs to its topic's module,
  and one table across topics would hide them.

## Non-Goals

- Fixing the unreachable `UnknownElement` of `place_element` in `crates/abyme-core/src/apply.rs`:
  it changes production code, so its backlog entry stays open.
- The canvas UI's TypeScript tests.
