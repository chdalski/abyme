**Repository:** root
**Status:** InProgress
**Created:** 2026-10-26

## Goal

Boards should say when they may no longer match the code. Compute review
flags from what Git reports under each element's anchor since its
verification stamp, pass them one hop along the relations, and show them
to the AI in `read_element`, `read_board`, and `search`. `verify` sets a
new stamp, the only way to set one, and `status` lists the open flags, so
the AI can keep the boards honest from one session to the next.

## Context

- Elements carry an optional anchor, a path relative to the project root
  that covers everything below it, and an optional verification stamp,
  `verified` (a full commit ID) with `verified_by` (`ai` or `person`):
  `crates/abyme-model/src/anchor.rs` and `verification.rs`, commits
  345f56a and 0d724fd.
- `abyme-core` (`crates/abyme-core/src/`, the Git module under
  `project/`) computes the review flags; the MCP server
  (`crates/abyme-mcp/src/server.rs`) shows them and offers `verify` and
  `status`.
- Risk categories: anchors and stamp commits from element files that
  clones and pull requests bring in reach Git's command line; file names
  that Git reports reach the AI; Abyme starts `git` from `PATH` in the
  repository, with its configuration, hooks, and filters, which are
  trusted; `verify` writes element files. These hit the escalation
  triggers "commands constructed from untrusted data" and "behavior no
  existing test covers"; every behavioral task is Security-Hybrid and
  adds the tests its criteria name.
- The concept: `docs/3 Reconciliation/Reconciliation.md` ("Detecting
  Changes"), `docs/3 Reconciliation/Checks.md`, `docs/2 Concepts/Relations.md`,
  `docs/4 AI Interface/MCP Server.md`, and the security baseline in
  `docs/5 Architecture/Hexagonal Architecture.md` and `docs/Decisions.md`.
- Git's references for the output formats Abyme reads:
  https://git-scm.com/docs/git, https://git-scm.com/docs/git-status,
  https://git-scm.com/docs/git-diff, https://git-scm.com/docs/git-ls-files,
  https://git-scm.com/docs/git-merge-base.

## Steps

- [x] A content change removes the stamp
- [x] One helper reads the project for the MCP read tools
- [x] Review flags of an element from Git, in `read_element`
- [x] Flags passed one hop along relations
- [x] Markers in `read_board` and `search`
- [ ] One locked write path in the store
- [ ] The `verify` tool
- [ ] The `status` tool

## Tasks

### Task 1: A content change removes the stamp

A stamp vouches only for the element as it was verified: every change of
an element's content through `apply` removes it, so `apply` never writes a
stamp. This needs no Git.

Review path: Security-Hybrid

- [x] An `update_element`, `create_relation`, or `delete_relation` that
      changes an element with a stamp writes it without one, also when
      the update leaves every value as it was; `place_element` and
      `move_element` leave the element file untouched.
- [x] No element file that `apply` writes carries a stamp; the property
      tests of `apply` check this for any request.
- [x] The `apply` tool description, the MCP README's Tools section, Data
      Model's operations, Checks' "Verification Stamp", and the decision on
      stamps in `docs/Decisions.md` say that a content change through
      `apply` removes the stamp.
- [x] `cargo test --workspace`, `cargo clippy --workspace --all-targets`,
      and `cargo fmt --all --check` pass.

### Task 2: One helper reads the project for the MCP read tools

`read_board`, `read_element`, and `search` each find the project and read
its files with the same lines; one function does it, so the review flags
of the next tasks enter in one place. Behavior is unchanged.

Review path: Reviewer-only

- [x] The three read tools find and read the project through one
      function, with the same errors as before.
- [x] These tests in `crates/abyme-mcp/tests/server.rs` pass with their
      assertions unchanged: `applies_operations_and_reads_the_board_back`,
      `read_board_shows_relations_with_their_labels_escaped_onto_one_line`,
      `read_element_shows_an_element_with_its_relations_and_description`,
      `read_element_names_an_element_without_a_file`,
      `read_board_shows_coordinates_only_on_request`,
      `without_a_project_every_tool_asks_for_init_and_writes_nothing`, and
      `without_a_cache_search_fails_and_the_other_tools_keep_working`.
- [x] `cargo test --workspace`, `cargo clippy --workspace --all-targets`,
      and `cargo fmt --all --check` pass.

### Task 3: Review flags of an element from Git, in `read_element`

Abyme asks Git what changed under each anchor since the element's stamp
and shows the resulting flags in `read_element`. Paths reach the review
relative to the project root, which may lie below the repository's root;
paths outside the project are left out.

Review path: Security-Hybrid

- [x] An element with an anchor gets a task flag when it has no stamp,
      when a file under its anchor differs from its stamp commit
      (committed, staged, or not) or is untracked and not ignored, or when
      no file that Git tracks or lists as untracked lies under its anchor.
      A file lies under an anchor when its path equals the anchor or
      starts with the anchor and `/`, compared byte for byte; a renamed
      file counts as removed and added.
- [x] Every element with a stamp, with or without an anchor, gets a task
      flag when its stamp commit is unknown to the repository or not an
      ancestor of HEAD.
- [x] `read_element` shows, after `Verified:`, the line `Review: task`,
      `Review: hint`, or `Review: none` with the element's highest
      priority, followed by one indented line per flag that names its
      reason; a flag for changed files names at most 10 of them, escaped
      like every other value, and how many more there are.
- [x] When the project lies in no Git repository, `git` cannot be run, or
      Git fails, `read_element` answers as before with the first line
      `Review flags unavailable: ` and the reason, and without the
      `Review:` lines.
- [x] The `read_element` description, the MCP README, and MCP Server's
      element detail describe the `Review:` lines and the line for
      unavailable flags.
- [x] The pure review function is tested with hand-built facts for every
      flag; an MCP test in a temporary Git repository shows the flags for
      a committed, an uncommitted, and an untracked change, an unknown
      stamp, a missing anchor, and an element without a stamp.
- [x] The security baseline in Hexagonal Architecture and
      `docs/Decisions.md` says that the core writes only inside `abyme/`
      and `.abyme/`, reads outside them only through Git with fixed
      arguments, and never opens a path built from an anchor; its accepted
      risks name the repository's Git configuration, hooks, and filters,
      `git` from `PATH`, and file names the AI reads.
- [x] Reconciliation's "Detecting Changes" describes the steps as
      delivered, renames included; the open point on Git access is gone
      from it and from `docs/Open Questions.md`. Hexagonal Architecture
      and the `abyme-core` row of the Components table in the root
      `CLAUDE.md` describe the core as holding the Git queries and the
      review flags.
- [x] `cargo test --workspace`, `cargo clippy --workspace --all-targets`,
      and `cargo fmt --all --check` pass.

### Task 4: Flags passed one hop along relations

An element is flagged when what it depends on or describes changed since
its own stamp, so the knowledge built on a changed part is reviewed too.

Review path: Security-Hybrid

- [x] An element with a stamp and a relation to an element with an anchor
      gets a flag when a file under the target's anchor differs from the
      source's stamp commit or is untracked and not ignored: a task for
      `depends_on` and `describes`, a hint for `uses` and `concerns`, and
      none for `see_also`. The flag names the relation's kind and target.
- [x] A flag is passed on one hop only; an element without a stamp gets no
      passed-on flag, and a target without an anchor passes none on.
- [x] `read_element` shows passed-on flags in its `Review:` lines.
- [x] Relations names, for each kind, whether it passes on a task, a hint,
      or nothing, and its open point on `describes` is closed; Checks
      describes the one-hop rule.
- [x] The pure review function is tested for every kind and for the
      states above; an MCP test in a temporary Git repository shows a
      passed-on task.
- [x] `cargo test --workspace`, `cargo clippy --workspace --all-targets`,
      and `cargo fmt --all --check` pass.

### Task 5: Markers in `read_board` and `search`

The board outline and search results show which elements need a review,
so the AI sees it before it reads an element.

Review path: Security-Hybrid

- [x] Every element line of a board outline and of search results ends
      with `⚠ task` or `⚠ hint` for the element's highest priority, or
      with no marker when it has no flag.
- [x] When the flags are unavailable, `read_board` and `search` answer as
      before with the first line `Review flags unavailable: ` and the
      reason.
- [x] The descriptions of `read_board` and `search`, the MCP README, and
      MCP Server's board outline describe the markers and the line for
      unavailable flags.
- [x] An MCP test in a temporary Git repository shows both markers in
      `read_board` and `search`.
- [x] `cargo test --workspace`, `cargo clippy --workspace --all-targets`,
      and `cargo fmt --all --check` pass.

### Task 6: One locked write path in the store

`apply` holds the lock, reads the files, decides its writes, and writes
them in one sequence; `verify` needs the same sequence with another
decision, so it becomes one function that takes the decision. Behavior is
unchanged.

Review path: Security-Hybrid

- [ ] `apply` writes through a store function that takes the decision as
      a parameter and holds the lock from reading to the last write.
- [ ] These tests in `crates/abyme-core/tests/project.rs` and the tests in
      `crates/abyme-core/src/project/store/tests.rs` pass with their
      assertions unchanged: `apply_writes_what_the_pure_apply_decided`,
      `a_rejected_request_changes_no_file`,
      `a_request_that_would_write_a_too_large_file_changes_no_file`,
      `a_relation_that_takes_an_existing_element_file_past_the_limit_changes_no_file`,
      `an_update_that_takes_an_element_file_past_the_limit_changes_no_file`,
      `applies_from_two_processes_without_losing_a_change`,
      `removes_leftover_files_before_writing_and_keeps_directories`, and
      `refuses_a_symbolic_link_at_a_fixed_path_when_writing`.
- [ ] `cargo test --workspace`, `cargo clippy --workspace --all-targets`,
      and `cargo fmt --all --check` pass.

### Task 7: The `verify` tool

The AI resolves an element's flags by checking it and calling `verify`,
which stamps it with the commit HEAD names, by `ai`.

Review path: Security-Hybrid

- [ ] The MCP tool `verify` takes only `element`, an element ID, sets the
      element's stamp to the commit HEAD names with `verified_by: ai`, and
      answers with that commit; the rest of the element file stays as it
      was, and it is written under the same lock as `apply`.
- [ ] `verify` refuses, and writes nothing, when the element has no file,
      when a file under its anchor differs from HEAD or is untracked and
      not ignored, with the answer that these changes must be committed
      first, when the repository has no commit yet, and when the flags are
      unavailable. An element whose anchor matches no file, and an element
      without an anchor, can be verified.
- [ ] After `verify`, the element has no flag for changed files until a
      file under its anchor changes, and flags passed on to it are
      measured against the new stamp; the flag for an anchor that matches
      no file stays.
- [ ] An MCP test in a temporary Git repository verifies an element, sees
      its flag for changed files gone in `read_element`, and sees each
      refusal; the test that checks every tool's parameters names `verify`
      with `element`.
- [ ] The server's instructions, the `verify` description, the MCP
      README, MCP Server, Checks, Reconciliation, and Data Model describe
      `verify` as delivered; Storage in Git asks for board and code in the
      same pull request rather than the same commit. The `abyme-mcp` row
      of the Components table in the root `CLAUDE.md` and the server's
      module documentation name every tool the server offers.
- [ ] `cargo test --workspace`, `cargo clippy --workspace --all-targets`,
      and `cargo fmt --all --check` pass.

### Task 8: The `status` tool

At the start of a session, the AI asks `status` for the open flags of the
whole project.

Review path: Security-Hybrid

- [ ] The MCP tool `status` takes no argument and answers with a line
      counting the elements by their highest priority, task or hint, then
      one line per flagged element as in search results, tasks first, each
      group by ID; at most 50 such lines, followed by how many more there are.
      Without flags it answers `No open review flags.`
- [ ] When the flags are unavailable, `status` answers with a tool error
      that names the reason.
- [ ] An MCP test in a temporary Git repository shows both groups, and
      the test that checks every tool's parameters names `status` with
      none.
- [ ] The server's instructions, the `status` description, the MCP
      README, MCP Server's tools, and Checks' "Reports" describe `status`;
      the `abyme-mcp` row of the Components table in the root `CLAUDE.md`
      and the server's module documentation name every tool the server
      offers.
- [ ] `cargo test --workspace`, `cargo clippy --workspace --all-targets`,
      and `cargo fmt --all --check` pass.

## Decisions

- **Git through its command line, not a Git library:** no new
  dependency, and Git documents its plumbing output formats.
- **Git access is a module of `abyme-core`, without a crate or a port
  trait:** Hexagonal Architecture gives a port its own trait only once a
  second implementation needs it.
- **Flags are computed by a pure function from facts as data,** in the
  core, not the model, and never stored, so they cannot go stale and every
  rule is tested without a repository.
- **A content change removes the stamp, also when it changes no value:**
  a stamp then never vouches for text nobody verified, and `apply` needs
  no comparison of before and after.
- **`verify` is no operation of `apply`:** the request format and the
  canvas's admission stay as they are, and no request can carry a stamp.
- **Passed-on flags:** an element without a stamp has no point in time
  to measure from, and a native target stands for nothing in Git.
- **An unknown stamp commit flags every element,** since it gives no point
  in time to measure from; "never verified" flags only an element with an
  anchor, since only an anchor ties an element to something Git can check.
- **Unavailable flags are named in one line** rather than left out, so
  the AI does not read missing markers as "nothing to review".
- **The flag for an anchor that matches no file does not depend on the
  stamp:** `verify` cannot make a missing path exist.
- **An anchor below the top of a nested repository matches no file and
  carries the missing-anchor flag** until nested repositories are covered;
  Git lists such a repository only by its top path.
- **`verify` stamps HEAD and refuses uncommitted changes under the
  anchor:** a stamp names a commit that holds what was checked.
- **No cache:** every call asks Git again, so flags never go stale.

## Non-Goals

- Changes inside repositories nested in the project, such as clones and
  submodules (separate plan after this one).
- Passing changes of an element's own content on to the elements that
  relate to it (separate plan after this one).
- Flags in the canvas.
- Anchors that follow renames, and hints for files that no anchor covers.
