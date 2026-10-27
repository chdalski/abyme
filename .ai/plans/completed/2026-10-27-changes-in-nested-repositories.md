**Repository:** root
**Status:** Completed (2026-10-27)
**Created:** 2026-10-27

## Goal

Repositories nested in a project, such as submodules, embedded clones, and
untracked clones, belong to it but have their own history, so review flags
miss what changes inside them and `verify` cannot stamp an element anchored
there. Detect changes in nested repositories for the elements anchored in
them, pass flags on across repository boundaries, and let `verify` stamp
such elements against their own repository, without running anything that
a nested repository's own configuration sets up.

## Context

- Review flags (commits e7c3635, e05731c) come from the pure
  `crates/abyme-core/src/review.rs` over facts that
  `crates/abyme-core/src/project/git.rs` gathers with a hardened `git`
  command line; paths are relative to the project root, and anchors are
  matched in Rust only. `verify` (ff19269) stamps through
  `store::write_locked` with `git::head` and the pure `review::stamped`.
- Today the project repository's Git output shows a nested repository only
  by its top path: an anchor below it matches no file, and an untracked
  clone under an anchor of the outer repository counts as an untracked
  change, so that element stays flagged and `verify` refuses it.
- Risk categories: the configuration, attributes, and file names of nested
  repositories become a new external input; Git runs in them with their
  own configuration; file names they report reach the AI. These hit the
  escalation trigger "new external input source"; every task is
  Security-Hybrid.
- The concept: `docs/3 Reconciliation/Reconciliation.md` (open point
  "Nested repositories"), `docs/Open Questions.md`, the Security Baseline in
  `docs/5 Architecture/Hexagonal Architecture.md`, `docs/Decisions.md`.

## Steps

- [x] Review facts per repository
- [x] Changes in nested repositories in the review
- [x] `verify` in nested repositories

## Tasks

### Task 1: Review facts per repository

The pure review judges each element by the facts of its own repository and
measures passed-on flags in the source's repository; until Task 2, Git
delivers facts for the project repository only.

Review path: Security-Hybrid

- [x] An element is judged by the facts of its repository: the deepest
      repository whose root is the element's anchor or lies above it; an
      element without an anchor belongs to the project repository. A
      change inside a nested repository never flags an element whose
      anchor merely contains that repository; only a change of that
      repository's root in the element's own repository does.
- [x] A passed-on flag is measured in the source's repository: the target
      counts as changed when a changed path there lies under the target's
      anchor and the target belongs to the source's repository, or when
      the changed path is the root of a nested repository that the
      target's anchor lies in. A source inside a nested repository passes
      nothing on to a target outside it.
- [x] The reason line of a passed-on flag says that what the target's
      anchor stands for may have changed since this element's stamp, and no
      longer that files under its anchor changed; `read_element`'s
      description, the MCP README, MCP Server, Checks, Relations, and
      Reconciliation's step 3 describe what a relation passes on as
      delivered.
- [x] With the project repository alone, the review gives the same flags
      with the same priorities as before; the review unit tests,
      `crates/abyme-core/tests/review.rs`, the outline tests, and the MCP
      tests pass with only the passed-on reason text changed.
- [x] Unit tests with hand-built facts of nested repositories cover the
      repository of an element, an anchor that contains a nested
      repository, and each rule for passed-on flags within and across
      repositories.
- [x] `cargo test --workspace`, `cargo clippy --workspace --all-targets`,
      and `cargo fmt --all --check` pass.

### Task 2: Changes in nested repositories in the review

`Project::review` finds the nested repositories through Git's own output
and reads those that hold an anchor, so changes inside them flag the
elements anchored there.

Review path: Security-Hybrid

- [x] The review finds nested repositories only through Git's output, as
      gitlinks and untracked repositories, also nested more than one
      level.
- [x] A file committed, staged, changed, or untracked in a nested
      repository since an element's stamp flags the element anchored over
      it, measured in that repository; a stamp commit that repository does
      not know is an unknown stamp.
- [x] For an element of the outer repository whose anchor contains a
      nested repository: a gitlink that moved flags it, and the dirty files
      of a submodule or an embedded clone do not; an untracked clone
      neither flags it as changed nor leaves its anchor matching no file.
      The test that expected an untracked clone to count as a change
      expects this instead.
- [x] Git runs in no nested repository whose Git directory lies outside it
      and outside the outer repository's store of submodules, whose work
      tree is not its own path, that is reached through a symbolic link, or
      whose directory is empty; an anchor inside such a repository is
      judged by the outer repository and gets the missing-anchor flag. In
      the nested repositories it does read, Git runs only the read-only
      commands it runs in the project repository.
- [x] No filter that a nested repository's own configuration sets up runs;
      filters from the user's global or system configuration do.
- [x] An anchor inside a clone that Git ignores gets the missing-anchor
      flag.
- [x] Integration tests in temporary repositories show the flags for a
      submodule, an embedded clone, an untracked clone, two levels of
      nesting, and a passed-on flag within a nested repository and across
      repositories; and that an uninitialized submodule, a `.git` file or
      work tree pointing outside, a symbolic link to a repository, and
      hostile filters in a nested repository's configuration or attributes
      run nothing and report nothing from outside the project.
- [x] A reader of Reconciliation's "Detecting Changes" learns which nested
      repositories are covered and which are not, and its open point is
      gone, also from `docs/Open Questions.md`; a reader of the Security
      Baseline and of its row in `docs/Decisions.md` learns what Abyme
      reads and trusts in a nested repository and what it turns off.
- [x] `cargo test --workspace`, `cargo clippy --workspace --all-targets`,
      and `cargo fmt --all --check` pass.

### Task 3: `verify` in nested repositories

`verify` stamps an element anchored in a nested repository against that
repository, so its flags can be resolved like any other.

Review path: Security-Hybrid

- [x] `verify` stamps an element anchored in a nested repository with the
      commit that repository's HEAD names, and refuses while files under
      the anchor differ from it or are untracked there; Git runs there
      under the same conditions as in the review.
- [x] `verify` refuses an element anchored inside a nested repository that
      Git may not run in.
- [x] For an element of the outer repository whose anchor contains a
      nested repository, `verify` refuses while a gitlink there differs
      from HEAD, and neither an untracked clone nor the dirty files of a
      submodule or an embedded clone make it refuse.
- [x] After `verify`, an element anchored in a nested repository has no
      flag for changed files until a file under its anchor changes in its
      repository.
- [x] Integration tests in temporary repositories show the stamp, each
      refusal, and the cleared flag. The `verify` description, the MCP
      README, MCP Server, Checks' "Verification Stamp", Reconciliation's
      step 4, Data Model, and Storage in Git say that the stamp names a
      commit of the repository the element's anchor lies in.
- [x] `cargo test --workspace`, `cargo clippy --workspace --all-targets`,
      and `cargo fmt --all --check` pass.

## Decisions

- **Nested repositories are hardened, not trusted like the project
  repository:** Git runs in one only after checks of where it lies, and
  without the filters its own configuration sets up; a clone cannot bring
  such a configuration, but an archive, a sync tool, or a script can, and
  Abyme runs Git on every read.
- **The repository of an element is the deepest one whose root is its
  anchor or lies above it;** a stamp needs no repository field, since the
  anchor names it: after the layout changes, the stamp names no commit in
  the new repository and becomes an unknown stamp.
- **An anchor that contains a nested repository is judged by its own
  repository only:** a stamp pins one repository; whoever wants the inside
  covered anchors the nested repository itself.
- **Passed-on flags are measured in the source's repository:** a moved
  gitlink counts for every target in it. This is coarse, but never reports
  "nothing changed" for a submodule or an embedded clone; it passes nothing
  on for an untracked clone, or from a source inside a nested repository to
  a target outside it.
- **Ignored clones are not covered:** Git does not report them, and listing
  every ignored file on every read is too costly.
- **Only nested repositories that hold an anchor are read,** so a project
  with nested repositories nothing is anchored in pays nothing extra.
- **Two read-only queries beyond the project's commands:** in a nested
  repository, `rev-parse` also names its Git, common, and submodule
  directories, and `config --show-scope --name-only` lists its filter
  drivers, as the security engineer's controls require; the project's
  `rev-parse` names its submodule directory once a nested repository is
  wanted.
- **Untracked repository roots are no uncommitted paths,** so from Task 2
  on, an untracked clone under an anchor no longer makes `verify` refuse
  that element.
- **`verify` refuses an anchor on the root of a nested repository Abyme
  cannot read,** while the review judges that anchor by the outer
  repository: a stamp of the outer repository would turn into an unknown
  stamp once the nested one became readable.
- **`diff-index` ignores only dirty submodules,** so a gitlink that a
  `.gitmodules` file says to ignore still counts as changed, in `verify`
  and in the review's changes since a stamp; the security engineer
  required it.

## Non-Goals

- A list of nested repositories in `abyme/abyme.toml` for clones that Git
  ignores.
- Shared helpers for temporary Git repositories in the tests (separate plan
  after this one).
- Review flags in the canvas.
