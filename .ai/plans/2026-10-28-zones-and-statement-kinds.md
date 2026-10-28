**Repository:** root
**Status:** InProgress
**Created:** 2026-10-28

## Goal

So that the AI weighs what it reads correctly and never takes an idea for a
fact, a board becomes a zone with a mode and a statement kind. Let the AI
set a board's zone through `apply` with a mode and a statement kind, the
pairs documented as the presets Current, Target, and Brainstorm; show each
element's statement kind, inherited from its home board, wherever the AI
reads elements; let `search` filter by statement kind; and let a mirrored
board refuse the drags a person makes in the canvas.

## Context

- The concept: `docs/2 Concepts/Boards and Zones.md` (modes, statement
  kinds, presets, inheritance), `docs/5 Architecture/First Slice.md`
  (the MVP has the modes free and mirrored and a search filterable by
  statement kind), `docs/5 Architecture/Data Model.md`,
  `docs/5 Architecture/Storage in Git.md`,
  `docs/4 AI Interface/Search and Index.md`, and
  `docs/5 Architecture/Canvas.md`.
- Boards live in `crates/abyme-model/src/board.rs`, closed value sets
  such as `RelationKind` in `crates/abyme-model/src/relation.rs`, and the
  object-only guard against the array form in
  `crates/abyme-model/src/object_only.rs`. `apply` and the operations are
  in `crates/abyme-core/src/apply.rs` and `operation.rs`; the hierarchy,
  with each element's home, in `crates/abyme-core/src/board_tree.rs`,
  which is private to the core; the detail levels in
  `crates/abyme-core/src/outline.rs`; the search index in
  `crates/abyme-index`; the tools in `crates/abyme-mcp/src/server.rs`; the
  canvas's write path in `crates/abyme-canvas/src/app.rs` and
  `admission.rs`, its board view in `projection.rs`, and its UI in
  `crates/abyme-canvas/ui/src`.
- Readers of new output: the AI through `read_board`, `read_element`,
  `overview`, `search`, and `status`; the canvas UI through the board view
  and `bindings.ts`.
- Risk categories: untrusted input (board files from the repository gain
  a field; `apply` gains zone fields; `search` gains a filter argument
  that reaches the index query; the canvas's write path decides from
  files it reads), data persistence (the store rewrites board files; the
  index table in the user's cache changes its columns), and output (new
  lines and columns in the tools, a new field for the webview), and
  availability (a board file whose zone does not read makes every tool
  call fail, as any invalid field does). A statement kind is a claim from
  repository content that steers how the AI weighs what it reads.

## Steps

- [ ] Zones in the board file and in `apply`
- [ ] Each element's statement kind, shown wherever the AI reads elements
- [ ] Search filtered by statement kind
- [ ] Mirrored boards in the canvas

## Tasks

### Task 1: Zones in the board file and in `apply`

A board file names the board's zone, and `apply` creates and changes it,
so that every later task reads one closed set of modes and kinds.

Review path: Security-Hybrid

- [ ] A board file holds an optional `zone` with a `mode`, `free` or
      `mirrored`, and a `statement`, one of `current`, `target`,
      `decision`, `idea`, `knowledge`, and `question`, both required. A
      board without a zone reads and writes byte for byte as before, and a
      board with one writes it on a line of its own. A zone with another
      mode or kind, a missing or unknown field, or an array in its place
      is rejected as other invalid fields are.
- [ ] `create_board` takes an optional `zone`, and `update_board` takes
      `zone`, where `null` removes it; an update that names only `zone`
      names a field. A zone in a request is rejected by the same rules as
      in a file.
- [ ] Unit and property tests cover the file and the operations,
      including a zone without its mode or its kind. The tests of unknown
      fields in a board file, in `create_board`, in `update_board`, and in
      the arguments of the MCP tool `apply` each still use a field that
      stays unknown. An integration test through the MCP tool `apply`
      creates a board with a zone, changes and removes it, and reads the
      board file back after each step.
- [ ] A reader of Data Model, Storage in Git, Boards and Zones, First
      Slice, and the Decisions table, and the AI reading the `apply` tool's
      description or the MCP README, can give a board each of the presets
      Current (mirrored, current), Target (free, target), and Brainstorm
      (free, idea), and knows that the board file stores mode and kind,
      never a preset.
- [ ] `cargo test --workspace`, `cargo clippy --workspace --all-targets`,
      and `cargo fmt --all --check` pass.

### Task 2: Each element's statement kind, shown wherever the AI reads elements

An element takes the statement kind of its home board's zone, and the AI
sees it in every tool that shows elements or boards.

Review path: Security-Hybrid

- [ ] One public core function gives every element with a file the
      statement kind of its home board's zone; an element without a home,
      or whose home has no zone, has none. An element placed on several
      boards takes the kind of its home, the one of them with the lowest
      ID.
- [ ] `read_board` shows, for a board with a zone, a header line
      `Zone: <mode> · <kind>` after the owner line and before the summary
      line. `overview` shows `<mode> · <kind>` in a column after the
      title. The element lines of `read_board`, `search`, and `status`
      show each element's statement kind in a column after its type.
      Each of these appears only where a zone or kind exists: for a
      project without zones every output is byte for byte as before, and
      the outline, search, status, and overview tests in
      `crates/abyme-core/src/outline/tests.rs`,
      `crates/abyme-core/tests/outline.rs`,
      `crates/abyme-mcp/tests/server.rs`, and
      `crates/abyme-mcp/src/server/tests.rs` pass with their assertions
      unchanged.
- [ ] `read_element` shows, after `Type:`, `Statement kind: <kind>, from
      board <id>` or `Statement kind: none`.
- [ ] Unit tests cover the function for an element without a home, on a
      board without a zone, and placed on two boards, and each display; an
      integration test through the MCP tools reads a zoned board, its
      overview, and one of its elements.
- [ ] The read tools' descriptions, the MCP README, MCP Server, and the
      inheritance rule in Data Model and Boards and Zones describe the
      statement kind as delivered: inherited from the home board's zone,
      none without one.
- [ ] `cargo test --workspace`, `cargo clippy --workspace --all-targets`,
      and `cargo fmt --all --check` pass.

### Task 3: Search filtered by statement kind

`search` finds only elements of one statement kind when asked, so that an
idea is never found as a fact.

Review path: Security-Hybrid

- [ ] `search` takes an optional `statement`, one of the six kinds. With
      it, it answers with the elements of that kind that match the query
      best, also when elements of other kinds match better and outnumber
      them; every element it answers with has that kind in the files at
      the time of the search. Without it, it answers as before. Any other
      value is rejected as invalid arguments.
- [ ] A change of a board's zone, or of an element's home, changes the
      kind an element is found under at the next search, with no other
      change to the element.
- [ ] An index built before this task is rebuilt at the next search, and
      search works on the rebuilt index.
- [ ] Tests in `crates/abyme-index` cover the filter with more matching
      elements of other kinds than results, a changed kind, and the
      rebuild. Tests through the MCP tool `search` cover a search with
      `statement` that answers only with elements of that kind, and a
      rejected `statement`.
- [ ] Search and Index, MCP Server, the MCP README, and the `search` tool
      description describe the filter and the kind each result line
      shows.
- [ ] `cargo test --workspace`, `cargo clippy --workspace --all-targets`,
      and `cargo fmt --all --check` pass.

### Task 4: Mirrored boards in the canvas

On a mirrored board only the AI changes things, so the canvas does not
let a person drag its elements.

Review path: Security-Hybrid

- [ ] The canvas refuses every `move_element` onto a mirrored board,
      writes nothing for that request, and says which board refused it;
      moves on free boards and on boards without a zone write as before.
- [ ] The board view the UI receives carries the board's mode, and the
      UI does not let the user drag elements on a mirrored board;
      `crates/abyme-canvas/ui/src/bindings.ts` matches the Rust types.
- [ ] Rust tests cover the refusal and the moves that stay allowed; a
      Vitest test covers that elements on a mirrored board are not
      draggable.
- [ ] Canvas, Boards and Zones, and the root `CLAUDE.md` Components table
      describe what a mirrored board does in the canvas.
- [ ] `cargo test --workspace`, `cargo clippy --workspace --all-targets`,
      `cargo fmt --all --check`, `pnpm --dir crates/abyme-canvas/ui
      check`, and `pnpm --dir crates/abyme-canvas/ui test` pass.

## Decisions

- **Tasks 1 to 4 run as Security-Hybrid:** they widen the input that
  board files, `apply`, and `search` accept, and bring behavior no test
  covered before; the user chose this path over handling them outside
  this workflow.
- **Only boards are zones:** frames serve Current and Target frames on
  one board, which come after the MVP; they will reuse the zone type.
- **The file stores mode and kind, never a preset:** a stored preset would
  need resolving on every read, and Current changes meaning once synced
  exists. Presets are named pairs in the docs and the tool description;
  an operation that takes a preset was declined as a second shape for
  the same request.
- **Modes `free` and `mirrored`, six kinds without `historic`:** synced
  and locked are not in the MVP and are rejected like any unknown value,
  as `contains` is; `historic` needs a check it skips and the Archive
  preset, which needs locked.
- **No zone, no kind:** an element without a home, or whose home has no
  zone, is unclassified and matches no filter; a default of `current`
  would make every board without a zone, a brainstorm too, a source of
  facts. Boards do not inherit zones from the board above.
- **Review flags stay independent of the statement kind** in this plan.
- **`search` has no default filter:** one would hide every unclassified
  element.
- **The filter runs in the index before ranking:** filtering after
  ranking returns too few results exactly when the filter is selective.
- **The canvas decides from the files before it applies a move:** a rule
  in the core would give `apply` a notion of who writes, for one
  operation of one caller; a board that turns mirrored between the check
  and the write lets that one move through, which changes only layout.

## Non-Goals

- A statement kind on the element itself, and kinds that element types
  bring (separate plan after this one).
- Frames, zone anchors, and the modes synced and locked.
- Review flags that depend on the statement kind.
- Choosing a zone in the canvas.
- Search filters by type or board.
