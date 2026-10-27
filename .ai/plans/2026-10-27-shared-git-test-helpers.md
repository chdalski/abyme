**Repository:** root
**Status:** InProgress
**Created:** 2026-10-27

## Goal

The tests of abyme-core and abyme-mcp each carry their own helper for
running Git in a temporary repository, and the copies have already drifted
apart in the options they set; the same add, commit, and read-HEAD calls
repeat in their setup. Give all of them one shared way to run Git and to
commit everything, and fold the repeated submodule setup of the core review
tests, without changing what any test checks.

## Context

- The three copies of `git(directory, arguments)` live in
  `crates/abyme-core/tests/review.rs`, `crates/abyme-mcp/tests/server.rs`,
  and the unit tests in `crates/abyme-mcp/src/server/tests.rs`. Unit tests
  in `src/` and integration tests in `tests/` share code only through a
  crate both depend on.
- Risk categories: none; the plan changes only test code, manifests, and
  one documentation line.

## Steps

- [x] Shared Git helpers in a test-support crate
- [ ] Local helpers for the submodule setup of the core review tests

## Tasks

### Task 1: Shared Git helpers in a test-support crate

A new workspace crate gives every test one way to run Git without the
user's or the system's configuration and one way to commit everything in a
repository; the three copies of the Git helper and the repeated commit
sequences go. Every test in the three files pins the behavior: among them,
`flags_a_stamp_that_names_no_ancestor_commit` switches to `main` and creates
an annotated tag, and every commit needs the test identity; no test pins
the two `gpgSign` options, which only guard against signing.

Review path: Reviewer-only

- [x] A crate `crates/abyme-test-support`, not published, under the
      workspace's license and lints and with no dependency, provides
      running Git in a directory with the options all three copies set
      today taken together, without global or system configuration, and
      committing everything in a repository, which answers with the new
      HEAD.
- [x] `cargo test --workspace` runs no test binary and no doc test of the
      new crate.
- [x] abyme-core and abyme-mcp take the crate as a dev-dependency, no copy
      of the Git helper remains in their tests, and wherever their tests
      add everything in a repository and commit it, they do so through
      the shared helper.
- [x] The sorted output of `cargo test --workspace -- --list` is the same
      before and after the task, and no assertion and no expected text in
      the tests changes.
- [x] The Components table of the root `CLAUDE.md` names the crate and
      what it is for.
- [x] `cargo test --workspace`, `cargo clippy --workspace --all-targets`,
      and `cargo fmt --all --check` pass, and `cargo deny --locked check`
      reports nothing but the advisories for `zstd` 0.13.3 and
      `zstd-safe` 7.3.0.

### Task 2: Local helpers for the submodule setup of the core review tests

The core review tests add submodules with the same call in several places
and build a repository nested two levels deep twice; local helpers in that
file hold each once. The tests that add submodules or nest two levels deep
pin the behavior.

Review path: Reviewer-only

- [ ] `crates/abyme-core/tests/review.rs` adds a submodule in one place, and
      both tests of a repository nested two levels deep build it in one
      place.
- [ ] The sorted output of `cargo test --workspace -- --list` is the same
      before and after the task, and no assertion and no expected text in
      the tests changes.
- [ ] `cargo test --workspace`, `cargo clippy --workspace --all-targets`,
      and `cargo fmt --all --check` pass.

## Decisions

- **The shared crate depends on no workspace crate:** abyme-core's own
  tests use it, so a dependency on abyme-core would give core's unit tests
  two different abyme-core crates; projects are initialized at the call
  sites.
- **Only running Git and committing everything are shared:** the
  element-file writers, core's `Repository`, and mcp's `project()` stay in
  their files. They write differently quoted YAML on purpose, assertions
  depend on it, and each serves one file.
- **One crate, not aligned copies, a feature on abyme-core, or `#[path]`
  includes:** aligned copies can drift apart again; a feature puts test
  code into a production crate's API; includes keep core's copy, and an
  included helper one file does not use breaks the build under the
  workspace's denied warnings.
- **Where the shared helpers differ from today's copies:** Git now runs
  with `init.defaultBranch=main` in the mcp unit tests and with
  `tag.gpgSign=false` in all mcp tests, and no mcp test names a branch or
  creates a tag; every commit allows an empty commit, and every test
  commit but core's `Repository::commit`, which already allows it,
  follows a change; all commits get one message, which
  no test reads; a source repository commits everything instead of its two
  files, which are all it holds; an embedded clone is committed without
  the option that silences Git's hint about it, which goes to error output
  no test reads.
- **`protocol.file.allow=always` stays at the calls that add submodules:**
  only those need it.
- **`cargo deny` may still report the advisories for `zstd` 0.13.3 and
  `zstd-safe` 7.3.0** (RUSTSEC-2026-0391 and 0395 to 0406): `lancedb`
  pulls in these crates, the advisories predate this plan, and they need
  a dependency decision of their own, handled as a separate request right
  after this plan.
- **Only the root `CLAUDE.md` names the crate:** the Crates table of the
  architecture lists the crates the architecture consists of, and a crate
  for tests is none of them.
