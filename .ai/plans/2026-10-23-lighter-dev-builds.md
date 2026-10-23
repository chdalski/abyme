**Repository:** root
**Status:** Completed (2026-10-23)
**Created:** 2026-10-23

## Goal

Builds and tests froze the user's computer: the test binaries link ONNX
Runtime and LanceDB with full debug info, and every review starts with a
clean build and the full test suite. Dev builds get much less debug info,
and reviews build without cleaning, on the one selection of packages whose
artifacts stay cached, so that a review no longer rebuilds everything.

## Context

- The workspace has no `[profile.*]` section; the only cargo configuration
  is the mold linker setting in `$CARGO_HOME`.
- The test binaries of `abyme-index` and `abyme-mcp` link ONNX Runtime and
  LanceDB/DataFusion.

## Steps

- [x] Narrow the reviewer's first step
- [x] Build with less debug info
- [x] Keep reviews on the workspace selection

## Tasks

### Task 1: Narrow the reviewer's first step

`.claude/agents/reviewer.md` starts every review with a clean build and
the full test suite. Its first step becomes building and testing the
changed crates and the crates that depend on them; the crates depend on
each other in the order `abyme-model` ← `abyme-core` ← `abyme-index` ←
`abyme-mcp`. The file comes from the user's blueprint, and this change
applies to this repository only.

Review path: Reviewer-only

- [x] Step 1 of `reviewer.md` tells the reviewer to run the build and the
      tests of the changed crates and of the crates that depend on them,
      in the foreground and one run at a time, and to run the clean
      command only when its results disagree with the handoff's or it
      suspects stale build state. For the exact commands, it points to the
      root `CLAUDE.md`.
- [x] No other passage of `reviewer.md` still assumes a clean build or the
      full suite: the reason that step 1 gives, the approval condition on
      the build and the tests, and the drift checks that refer to a failing
      listed command, which then also cover its per-crate form.
- [x] No other file under `.claude/` changes.

### Task 2: Build with less debug info

The dev profile, which the tests use too, keeps line tables for the
workspace's own crates and drops debug info for dependencies, to reduce
the memory that linking the test binaries needs and the size of
`target/`.

Review path: Reviewer-only

- [x] The workspace `Cargo.toml` gives the dev profile line tables for the
      workspace's crates and no debug info for dependencies.
- [x] `cargo build --workspace`, `cargo clippy --workspace --all-targets`,
      and `cargo test --workspace` pass.
- [x] The handoff reports the size of `target/debug` and of the largest
      test binary of `abyme-index` and of `abyme-mcp` before and after the
      change, each measured after one `cargo test --workspace --no-run`
      run in the foreground.
- [x] The handoff shows which crates a `cargo test -p abyme-index --no-run`
      run compiles right after `cargo test --workspace --no-run`.
- [x] From the root `CLAUDE.md`, a reader learns that the dev profile
      carries little debug info, how to build once with full debug info,
      and how to run only some tests without leaving the workspace
      selection, and why a run with `-p` builds a second set of
      artifacts.

### Task 3: Keep reviews on the workspace selection

Step 1 of `.claude/agents/reviewer.md` now builds and tests the changed
crates and their dependents. In this workspace, a run that selects single
crates with `-p` resolves features differently from the workspace build
and compiles the dependencies a second time. Step 1 instead runs the
workspace commands without cleaning and narrows a run only by selecting
tests.

Review path: Reviewer-only

- [x] Step 1 of `reviewer.md` tells the reviewer to run the build and test
      commands of the root `CLAUDE.md` for the whole workspace, in the
      foreground and one run at a time, to narrow a run only by selecting
      tests, never by selecting packages, and to run the clean command
      only when its results disagree with the handoff's or it suspects
      stale build state.
- [x] No passage of `reviewer.md` still tells the reviewer to build or
      test single components, and the drift checks name only the listed
      commands and their narrowed form.
- [x] No other file under `.claude/` changes.

## Decisions

- **Line tables for the workspace's crates, no debug info for
  dependencies:** backtraces still name file and line in Abyme's own code.
  Rejected:
  - no debug info anywhere, because it saves little and loses those line
    numbers;
  - `split-debuginfo`, because `target/` stays large;
  - a change to the `test` profile only, because it misses the
    `abyme-mcp` binary that the tests of `abyme-mcp` spawn.
- **The sizes and the crates a per-crate run compiles are reported, not
  required,** since the effect is measured only once. A per-crate run may
  rebuild workspace crates whose features differ from the workspace build,
  such as `abyme-model` and `abyme-core` without `json-schema`.
- **`reviewer.md` changes first,** so that the reviewer of the profile
  change already follows the narrower step and does not rebuild
  everything right after the profile change's own full rebuild.
- **Builds keep the workspace selection:** after the workspace build, a
  `cargo test -p abyme-index --no-run` run compiled about 100 dependencies
  again (3 min 22 s, 2.7 GB more in `target/debug`), because without
  `abyme-mcp` in the selection `serde` lacks `rc`, `indexmap` lacks
  `serde`, and `abyme-model` and `abyme-core` lack `json-schema`; a second
  workspace run compiled nothing. Rejected:
  - unifying the features with a helper crate, because it has to be kept
    in step with every dependency change;
  - leaving `-p` runs as they are, because each new selection costs a
    rebuild of the dependencies once.

## Non-Goals

- A limit on parallel build jobs
- A swap limit for the container
- Changes to `.claude/rules/lang-rust.md`; outside reviews, how runs are
  narrowed is a working practice, not a file change
