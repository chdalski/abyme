**Repository:** root
**Status:** InProgress
**Created:** 2026-10-21

## Goal

The Working Skeleton starts with its core: an AI connected
over MCP can create a board and an element, place the element
on the board, and read the board back as an outline, with the
data stored as Abyme files in the repository. This plan builds
the Cargo workspace with `abyme-model`, `abyme-core`, and
`abyme-mcp`; search and the canvas follow in their own plans.

## Context

- The decided design is in the concept vault and serves as
  the specification: `docs/5 Architecture/First Slice.md`
  (Working Skeleton), `Hexagonal Architecture.md` (crates,
  core API, security baseline), `Data Model.md` (slug IDs,
  operations), `Storage in Git.md` (files, `.abyme/`,
  writing), and `docs/4 AI Interface/MCP Server.md` (detail
  levels, outline example).
- The workspace lint set is the one in
  `.claude/skills/project-init/rust-init.md`.
- Library versions: `rlsp-yaml-parser` 0.11.2 (MIT), `rmcp`
  3.6.0 (Apache-2.0, depends on `schemars` ^1.0), `schemars`
  1.2.3 (MIT), `toml` 0.9.8 (MIT OR Apache-2.0), `proptest`
  1.12.0 (MIT OR Apache-2.0), `rustix` 1.1.5 (Apache-2.0 WITH
  LLVM-exception OR Apache-2.0 OR MIT), `tempfile` 3.27.0 (MIT
  OR Apache-2.0, tests only).
- Risk categories: untrusted input from element, board, and
  project files that can arrive through a clone or a pull
  request, and from change sets the AI sends over MCP; file
  paths built from IDs; file persistence with two writing
  processes; a project file that another local user places in
  a directory above the working directory; a new external
  input source (MCP over stdio).
  The security baseline and its accepted risks are in
  `Hexagonal Architecture.md#Security Baseline`.
- Accepted limits of the skeleton: every call reads all
  files, so one rejected file makes every call fail; a failed
  write between two renames can leave an element without a
  home, which the data model allows.

## Steps

- [x] Settle the design of the skeleton core
- [x] Workspace and board file format
- [x] Run `/project-init` and commit its outputs: it fills the
      Build and Test section of the root `CLAUDE.md` and
      enables the rust-analyzer plugin
- [x] Element files
- [x] Pure apply
- [x] Persisting operations
- [x] Board outline
- [ ] MCP server

## Tasks

### Task 1: Create the workspace and the board file format

The Cargo workspace exists with the workspace lint set, and
`abyme-model` holds the slug ID types and the board file, so
that all later code is written against the lints. A slug
consists of lowercase ASCII letters, digits, and single
hyphens between them, with at most 64 characters; every ID
read from a file or received from outside passes the same
check as a generated one. The board file follows
`Storage in Git.md#Board File`, reduced to `format`, `id`,
`title`, and placements with `element`, `x`, `y`, `width`, and
`height`. A board's `id` must equal its file name.

Review path: Security-Hybrid

- [x] The workspace builds, and `cargo clippy` with the
      workspace lint set reports nothing; every crate inherits
      the workspace lints
- [x] `target/` is ignored by Git, and `Cargo.lock` is
      committed
- [x] A string that is not a valid slug cannot become an
      `ElementId` or a `BoardId`; tests cover accepted and
      rejected forms, including 64 and 65 characters
- [x] A board written and read back is unchanged; the file has
      one placement per line, sorted by element ID, and moving
      one placement changes exactly one line
- [x] A board file whose placements are not sorted is read and
      written back sorted
- [x] A board file is rejected, with an error naming the file,
      when it has invalid JSON, an unknown field, an invalid
      ID, an `id` other than its file name, an unknown format
      version, or two placements of the same element; each case
      has a test
- [x] The status lines of the root `README.md` and
      `docs/README.md` and the concept-phase statements of the
      root `CLAUDE.md` outside its Build and Test section say
      that the Working Skeleton is being built and that search
      and the canvas follow its core

### Task 2: Read and write element files

Element files are Markdown with YAML frontmatter
(`Storage in Git.md#Element File`), reduced to `id`, `type`,
`name`, `summary`, and the body below the frontmatter.
`rlsp-yaml-parser` reads the frontmatter, `abyme-model` maps
its AST to the element, and an emitter of its own writes the
frontmatter. Decided rules:

- The first line is exactly `---`, and the frontmatter ends at
  the next line that is exactly `---`; the body is everything
  after it, kept byte for byte. A byte order mark is rejected;
  CRLF is accepted on reading, LF is written.
- The failsafe schema keeps every scalar a string.
- `id`, `type`, and `name` are required, `summary` is
  optional; `id` must equal the file name, and `type` must not
  be empty. An absent summary is written as no key and read
  back as absent; an empty summary stays an empty string.
- `type`, `name`, and `summary` hold only YAML-printable
  characters: no C0 control other than tab, line feed, and
  carriage return, no DEL, no C1 control other than U+0085,
  and neither U+FFFE nor U+FFFF. The frontmatter, counted in
  bytes between the two `---` lines as the emitter writes it,
  is at most 64 KiB. Building an element checks both rules,
  and reading builds the element through that same check
  after rejecting a file whose frontmatter exceeds 64 KiB
  before parsing; so every element read can be written again,
  and no element can be written that the reader rejects.
- Rejected: aliases, anchors, tags written in the file,
  non-scalar keys, non-scalar values, unknown keys, duplicate
  keys, a missing required key, directives, document start or
  end markers inside the frontmatter, and more than one
  document.
- The emitter writes the known keys plain and every value
  double-quoted. It escapes backslash, double quote, tab,
  line feed, carriage return, U+0085, U+2028, U+2029, and
  U+FEFF and keeps every other character as it is, so no
  written line can be `---`.

Review path: Security-Hybrid

- [x] An element with arbitrary strings of YAML-printable
      characters in `name` and `summary` within the frontmatter
      limit, an arbitrary body, a non-empty `type`, and a valid
      `id`, including line breaks and lines that read `---`, is
      unchanged after writing and reading back; a property test
      checks this, and its regression files are committed
- [x] A value with a character outside the allowed set, and a
      frontmatter over 64 KiB, are rejected both when reading
      and when building an element; tests cover each excluded
      character class, an emitted frontmatter of exactly 64 KiB
      and one byte more, and a file whose escapes would grow
      its frontmatter past the limit when written
- [x] Each decided rule has its own test: every rejection with
      a file that breaks only that rule, and every accepted
      form (CRLF, the body kept byte for byte, an absent and an
      empty summary)
- [x] The element example from `Storage in Git.md`, reduced to
      this task's fields, reads into the expected element
- [x] A reader of `Storage in Git.md` can tell which fields the
      Working Skeleton reads and writes in element, board, and
      project files, which come later, how frontmatter values
      are written, and which characters and size `type`, `name`,
      and `summary` may have; the examples match what the
      skeleton writes
- [x] The YAML library is no longer an open question in
      `Open Questions.md` or in `Storage in Git.md`, and
      `Decisions.md` holds `rlsp-yaml-parser` with its reason

### Task 3: Apply operations without touching files

`abyme-core` turns a list of operations into file writes as a
pure function over the current elements and boards, so the
decisions can be tested without a file system. Operations:
`create_element` (key, `type_ref`, name, optional summary,
body), `create_board` (key, title), and `place_element`
(element, board). A reference is either `{"new": "<key>"}`, a
key created by an earlier operation of the same request, or
`{"existing": "<slug>"}`. Elements and boards share one set of
keys per request. Operations are tagged by `op` in snake case
and reject unknown fields; an optional `json-schema` feature
derives their JSON Schema with `schemars` 1.x. A slug comes
from the name: lowercase ASCII letters and digits stay, every
other run of characters becomes one hyphen, hyphens at the
ends are dropped, and an empty result becomes `element` or
`board`. A slug that would be longer than 64 characters is
shortened so that it fits, together with any collision suffix.
A placed element gets the fixed skeleton size of 180 × 90.
Apply accepts only input that yields files the readers of
Tasks 1 and 2 accept; `type_ref` must not be empty.

Review path: Security-Hybrid

- [x] An empty list of operations succeeds with no writes
- [x] Apply returns either all file writes or an error and no
      writes; tests cover an unknown slug, an unknown key, a
      key used before the operation that creates it, a key
      created twice (also once for an element and once for a
      board), a key of a board used as an element and the
      reverse, placing an element that already has a home on
      disk, placing one element twice in one request, an
      empty `type_ref`, a character outside the allowed set in
      `type_ref`, name, and summary, and an element whose
      frontmatter would exceed 64 KiB
- [x] Every file in the returned writes is accepted by the
      readers of Tasks 1 and 2; a property test over arbitrary
      names, summaries, bodies, and titles checks it
- [x] Every new element and board gets a slug that collides
      neither with existing ones nor with others from the same
      request, through the suffixes `-2`, `-3`, and so on;
      apply reports the slug each key received
- [x] A name whose slug would exceed 64 characters yields a
      valid slug of at most 64 characters, also when a
      collision suffix is added; tests cover both cases
- [x] A placed element overlaps no other placement on its
      board, including placements made earlier in the same
      request; a test places two elements on one board in one
      request
- [x] In the returned writes, element files come before board
      files
- [x] Operations in the JSON shape described in this task are
      accepted, and unknown fields and unknown `op` values are
      rejected; with the `json-schema` feature the operations
      yield a JSON Schema
- [x] From `Data Model.md` alone, a reader can write a valid
      request with the skeleton's operations and predict the
      slug a name receives; `Glossary.md` defines local key

### Task 4: Persist operations in the repository

`abyme-core` finds the project, initializes it on request, and
applies operations to the files in it. A directory is an Abyme
project when it contains `abyme/abyme.toml`; the project root
is the nearest such ancestor, also from inside a Git repository
nested in the project. The root is accepted only when the root
directory, `abyme/`, and `abyme/abyme.toml` belong to the user
running Abyme; otherwise every call, init included, fails with
an error naming the root, without looking further up. The
skeleton builds for Unix only. Init creates `abyme/abyme.toml`
with `format = 1` and `.abyme/` with its own `.gitignore`
containing `*`. Inside an existing project, init changes
nothing and names the project's root; where
`abyme/abyme.toml` exists but is rejected, init fails with the
same error and leaves the file as it is; where only `.abyme/`
exists, init completes the project. Git does not carry
`.abyme/`, so a write creates it, with its `.gitignore`, when
it is missing; reads
do not need it. A missing `abyme/elements/` or `abyme/boards/`
counts as empty. In those folders, only files named
`<slug>.md` or `<slug>.board.json` are read; other entries are
ignored. Applying reads the current files, runs the pure apply,
and writes under the advisory lock on `.abyme/lock`, each file
through a temporary file in `.abyme/tmp/` and a rename. Every
path comes from one function that resolves the kinds of files
Abyme touches: element, board, project file, lock, and
temporary file.

Review path: Security-Hybrid

- [x] From a directory inside a project, the project root is
      found; without `abyme/abyme.toml` above, every call fails
      with an error that tells the developer to run init, and
      nothing is written
- [x] From a directory inside a Git repository nested in a
      project, the outer project's root is found; a project
      whose root directory, `abyme/`, or `abyme/abyme.toml`
      belongs to another user makes every call and init fail
      with an error naming the root, also when a project
      further up belongs to the user, and nothing is written;
      tests cover each of the three paths
- [x] An `abyme/abyme.toml` that is not valid TOML, lacks
      `format`, has an unknown `format`, or has an unknown key
      makes every call fail with an error naming the file
- [x] Init in a directory with no project in it or above it
      creates `abyme/abyme.toml` and the self-ignoring
      `.abyme/`; running it again, or in a subdirectory of the
      project, changes nothing and names the project root;
      with a rejected `abyme/abyme.toml` it fails and leaves
      the file unchanged; with only `.abyme/` present it
      creates `abyme/abyme.toml`; each case has a test
- [x] In a project without `.abyme/`, as after a fresh clone,
      reading works, and the first write creates `.abyme/` with
      its `.gitignore`; in a project without `abyme/elements/`
      or `abyme/boards/`, reading works and writing creates the
      folder
- [x] Entries in `abyme/elements/` and `abyme/boards/` that are
      not files named as a valid slug with the expected ending,
      including a directory with such a name, are ignored; a
      test shows it
- [x] After an apply, the files on disk hold what the pure
      apply decided; an apply that fails validation changes no
      file
- [x] Two processes that apply at the same time both land, and
      neither overwrites the other's change; an integration
      test shows it
- [x] Leftover files in `.abyme/tmp/` from an earlier crash are
      removed before the next apply writes, while that apply
      holds the lock, so a concurrent writer's temporary file
      is never removed
- [x] No read or write reaches outside `abyme/` and `.abyme/`;
      tests with an ID-shaped path and with a symlink inside
      `abyme/` show it
- [x] A reader of `Storage in Git.md` learns how a repository
      becomes an Abyme project, how Abyme finds the project
      root and when it rejects one, what a fresh clone needs,
      where temporary files go, and in which order files are
      written;
      `Decisions.md` holds the explicit init with its reason

### Task 5: Read a board as an outline

`abyme-core` renders a board in the compact text format of
`MCP Server.md#Detail Levels`, limited to what the skeleton
stores: a header line with the board's slug and title, and one
line per placement with the element's slug, type, name, and
summary, sorted by slug. A placement whose element file does
not exist shows its slug with the marker `missing`, which can
happen after a merge or while a writer adds an element.

Review path: Security-Hybrid

- [x] For a board with placed elements, the outline lists each
      element on one line with slug, type, name, and summary,
      sorted by slug; a snapshot test fixes the format
- [x] Each element takes exactly one line of the outline, also
      when its name or summary contains line breaks
- [x] A placement whose element file is missing appears as its
      slug with the marker `missing`, and the rest of the
      outline is unaffected; the snapshot test covers such a
      line
- [x] An unknown board slug gives an error that names it

### Task 6: Serve apply and read_board over MCP

`abyme-mcp` is a binary. `abyme-mcp init` initializes the
project in the current directory; without arguments it runs an
MCP server over stdio with the tools `apply` and `read_board`.
`apply` takes the operations with the schema from the
`json-schema` feature and reports the slug each key received;
`read_board` returns the outline. Errors in the request's
content come back as tool results marked as errors; malformed
parameters are protocol errors. Standard output carries only
protocol messages, and logs go to standard error. The core
stays synchronous, and the server calls it without blocking
the async runtime.

Review path: Security-Hybrid

- [ ] Running `abyme-mcp init` in a directory with no project in
      it or above it creates the project there; inside an
      existing project it changes nothing and prints the
      project root
- [ ] An integration test starts the built binary in a project
      created by `abyme-mcp init`, lists the tools, creates a
      board and an element and places it with one `apply`,
      reads the board with `read_board`, and finds the
      element's slug in the outline and its files on disk
- [ ] The result of `apply` maps each key to the slug it
      received
- [ ] An apply rejected for its content returns an error result
      that names the problem, malformed parameters return a
      protocol error, and in both cases the server keeps
      serving
- [ ] In a directory without a project, both tools answer with
      the hint to run `abyme-mcp init`
- [ ] Everything the server writes to standard output is a
      protocol message; the integration test checks it
- [ ] A developer learns from `crates/abyme-mcp/README.md` how
      to install the server, initialize a repository, and
      register the server with Claude Code

## Decisions

- **References:** `new(key)` for what the same request
  creates, `existing(slug)` for the rest. Rejected: keys as
  slug proposals, because after a collision suffix the AI
  could silently address the wrong element; unmarked
  references, because a typo would silently hit an existing
  element; compound operations and two calls, because the
  MVP creates relations between new elements in one
  transaction.
- **Explicit init:** a repository becomes an Abyme project
  only through `abyme-mcp init`, which creates
  `abyme/abyme.toml`. Initializing on first write was
  rejected, because a server registered for all projects
  would let one `apply` create `abyme/` in any repository,
  also when the AI was steered by content it read. Recreating
  `.abyme/` in an initialized project is not initialization:
  Git does not carry it, so every fresh clone lacks it. Init
  never creates a project inside another one, so a project
  root is never shadowed, and never overwrites a project
  file it rejects.
- **Discovery crosses nested repositories and checks the
  owner:** Git repositories nested in a project, such as
  clones and submodules, belong to it, so the walk up does
  not stop at a Git boundary. A found root that belongs to
  another user is rejected, not skipped, so that a project
  file another local user places above the working directory,
  as in `/tmp`, is never adopted. Rejected: stopping at the
  Git boundary, because the outer project would not be found
  from a nested repository; stopping at the first directory
  that belongs to another user, because it misses a directory
  the user owns but others can write to. Costs: a project
  whose files belong to another user than the process, as in
  some containers, is rejected, and there is no override; the
  skeleton builds for Unix only until the check exists for
  Windows.
- **Element values are limited** (rule in Task 2), because
  `rlsp-yaml-parser` rejects non-printable characters even as
  escapes and parses escapes slowly; the round trip holds
  within the limits only.
- **Apply writes only readable files:** input that would
  produce a file the readers reject, such as an empty
  `type_ref`, is rejected by apply, because a written invalid
  file would make every later call fail.
- **No default board:** boards come only from `create_board`;
  a default board would need its own role later.
- **Slugs are ASCII:** other characters become hyphens, so a
  name such as "Überweisung" loses its umlaut; transliteration
  can follow when names need it. A long name yields a
  shortened slug rather than a rejected apply, so the AI is
  not sent into a retry for a name it cannot shorten
  usefully.
- **Files that disagree are rejected, foreign files ignored:**
  an `id` that differs from its file name, or a board that
  places one element twice, makes the file invalid; files
  whose names are not slugs, such as editor backups, are not
  Abyme files. A placement whose element is missing is shown,
  not rejected, because a merge or a concurrent write can
  produce it.
- **Operations run in order:** a key exists from the operation
  that creates it on; elements and boards share one set of
  keys, so a key always names one thing.
- **Format rules, not security controls:** the frontmatter
  rules in Task 2 and the confinement to `abyme/` and `.abyme/`
  are decisions on the file format and the security baseline;
  the security engineer specifies the controls at each task's
  gates.
- **Escalation:** the tasks touch escalation triggers (a new
  external input source, file paths built from IDs), and the
  user chose to proceed with Security-Hybrid for the Working
  Skeleton.

## Non-Goals

- Search and `abyme-index`, including the devcontainer
  changes it needs (next plan)
- The canvas, its file watcher, and moving placements for the
  canvas drag (plan after that)
- Relations, anchors, review flags, references, deleting
  elements, and text measurement (MVP)
- Recording in the concept docs that repositories nested in a
  project belong to it, and that Abyme itself is the main test
  project (separate plan after this one)
