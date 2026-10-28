**Repository:** root
**Status:** Completed (2026-10-28)
**Created:** 2026-10-27

## Goal

The AI builds C4 models level by level: an element's inside is a board of
its own. Let a board be the inner board of an element and carry a summary,
let the AI create and change such boards through `apply`, show the hierarchy
in the board outline and the element detail, and give the AI `overview`, the
entry point of every session, which lists all boards as a tree with their
summaries and open review flags.

## Context

- Boards live in `crates/abyme-model/src/board.rs`; `apply`
  (`crates/abyme-core/src/apply.rs`, `operation.rs`) turns operations into
  file writes; `crates/abyme-core/src/outline.rs` renders the detail levels;
  `crates/abyme-mcp/src/server.rs` serves the tools, whose module
  documentation, instructions, descriptions, and README section tell the
  AI how to use them.
- An element has at most one home today, and references are not in the
  MVP, so a board's parent is the home board of its owner.
- The concept: `docs/2 Concepts/Board Graph.md`,
  `docs/2 Concepts/Relations.md`, `docs/5 Architecture/Data Model.md`,
  `docs/5 Architecture/Storage in Git.md`, and
  `docs/4 AI Interface/MCP Server.md` (detail levels, outline example).
- Risk categories: untrusted input (board files from the repository gain
  fields; new operation fields and a new operation over MCP; a new tool),
  data persistence (the store rewrites board files), and repository text
  in tool output (board titles and summaries in the outline and the
  overview).

## Steps

- [x] One view of the boards as a request stands
- [x] Inner boards and board summaries in the files and in `apply`
- [x] Inner boards in the board outline and the element detail
- [x] The `overview` tool

## Tasks

### Task 1: One view of the boards as a request stands

`apply` decides several things from the boards as the request stands: the
boards it has changed, and the snapshot's others. Give that view one place,
so the checks of the next task do not each rebuild it. Behavior is
unchanged; `crates/abyme-core/src/apply/tests.rs` (deleting elements with
placements on several boards, placements without a file) and
`crates/abyme-core/tests/apply_properties.rs` pin it.

Review path: Reviewer-only

- [x] Finding the boards that place an element uses the one view, which
      yields the request's changed boards and the snapshot's untouched
      ones.
- [x] The tests in `crates/abyme-core/src/apply/tests.rs` and
      `crates/abyme-core/tests/apply_properties.rs` pass with their
      assertions unchanged.
- [x] `cargo test --workspace`, `cargo clippy --workspace --all-targets`,
      and `cargo fmt --all --check` pass.

### Task 2: Inner boards and board summaries in the files and in `apply`

A board file names the element whose inside it is and carries a summary,
and `apply` creates and changes such boards while keeping every element to
at most one inner board and every board out of its own inside.

Review path: Security-Hybrid

- [x] A board file holds an optional `owner` (an element ID) and an
      optional `summary` (any text); a file without them reads and writes
      byte for byte as before, and an invalid owner ID is rejected as other
      invalid fields are.
- [x] `create_board` takes an optional `owner`, an element named as in
      `place_element`, also one the same request creates, and an optional
      `summary`.
- [x] `update_board` changes only the fields it names among `title`,
      `summary`, and `owner`, where `null` removes a summary or an owner;
      it is rejected when it names none, and its board keeps its ID and
      placements.
- [x] Every owner is an element, no element owns two boards, and no board
      lies inside itself: `create_board`, `update_board`, and
      `place_element` are each rejected when, as the request stands, they
      would bring in a breach of one of these that was not there before.
      A breach the files already hold rejects nothing else, so a request
      can repair it step by step. An owner without a home is accepted, so
      the order of a request's operations stays free.
- [x] `delete_element` is rejected while a board names the element as its
      owner, and the error names that board.
- [x] `apply`'s checks take the hierarchy of boards from one pure board
      tree, which the readers of the next tasks use too.
- [x] The canvas refuses `update_board` as it refuses the other operations
      it does not admit.
- [x] Unit tests cover each rule above, a cycle closed by `update_board` as
      well as by `place_element`, and a request that repairs one of two
      breaches the files hold; an integration test through the MCP tool
      `apply` builds a context board, a container placed on it, and the
      container's inner board in one request, and reads the files back.
- [x] The `apply` tool's description, the MCP README, Data Model, Storage in
      Git, and Board Graph's Deleting section describe the board fields and
      the operations as delivered, and no doc names `inner_board` as a
      field or link of the element.
- [x] `cargo test --workspace`, `cargo clippy --workspace --all-targets`,
      and `cargo fmt --all --check` pass.

### Task 3: Inner boards in the board outline and the element detail

`read_board` shows whose inside a board is and which elements on it open an
inner board, and `read_element` names an element's inner boards.

Review path: Security-Hybrid

- [x] The outline's first line keeps its form; a board with an owner gets a
      second line, `Inside of:` with the owner and `Above:` with the board
      the owner lies on, and a board with a summary then a line with it.
- [x] Element lines have the columns ID, type, name, summary, inner boards,
      and review marker, and drop empty trailing columns: an element that
      owns boards shows `▸` and their IDs in the inner boards column, in
      `read_board`, `search`, and `status` alike, and an element whose
      summary is empty and that has nothing after it no longer ends in
      spaces. Apart from that, the outline, search, and status tests in
      `crates/abyme-core/src/outline/tests.rs`,
      `crates/abyme-core/tests/outline.rs`,
      `crates/abyme-mcp/tests/server.rs`, and
      `crates/abyme-mcp/src/server/tests.rs` pass with their assertions
      unchanged.
- [x] `read_element` shows `Inner board:` after `Boards:`, with every board
      the element owns, or `none`.
- [x] Reading never loops, and the line naming a board's owner marks an
      owner whose file is missing, an owner without a home, and a board
      that lies inside itself; two boards with one owner both appear on the
      owner's element line and in its detail. A test covers each.
- [x] The outline and `read_element` take the hierarchy from the board tree
      of Task 2.
- [x] Board titles and summaries are escaped onto their line as element
      text is.
- [x] The read tools' descriptions, the MCP README, and MCP Server (outline
      example, element detail, and how board summaries come about) describe
      the lines as delivered. Relations, the Decisions row on relation
      kinds, the Glossary, and Data Model no longer say that `contains`
      comes with inner boards; they say it is not stored and passes no
      review flags, and the kinds table claims no check for it.
- [x] `cargo test --workspace`, `cargo clippy --workspace --all-targets`,
      and `cargo fmt --all --check` pass.

### Task 4: The `overview` tool

`overview` gives the AI all boards at a glance, as the hierarchy of
insides, so a session starts there instead of guessing board IDs.

Review path: Security-Hybrid

- [x] The MCP tool `overview` takes no argument and lists every board once,
      depth-first from the boards without a parent, each level indented
      two spaces further: its ID, title, summary, the number of elements
      whose home it is, without placements whose element file is missing,
      and the open review flags of those elements by priority.
- [x] A board in a cycle, or whose owner has no home or no file, is listed
      at the top level with the marker the outline gives it.
- [x] At most 50 boards are listed, followed by how many more there are.
- [x] When the project lies in no Git repository, `git` cannot be run, or
      Git fails, the answer starts with the line `Review flags
      unavailable:` and the reason, and lists the boards without flag
      counts.
- [x] `overview` takes the hierarchy from the board tree of Task 2.
- [x] The server's instructions tell the AI to start a session with
      `overview`.
- [x] Integration tests through the MCP tool cover a tree of three levels,
      flag counts, the cap, a project without Git, and a cycle from a
      hand-edited file.
- [x] The tool's description, the server's module documentation, the MCP
      README, MCP Server, and the root `CLAUDE.md` Components table name
      `overview` and describe it as delivered.
- [x] `cargo test --workspace`, `cargo clippy --workspace --all-targets`,
      and `cargo fmt --all --check` pass.

## Decisions

- **Tasks 2 to 4 run as Security-Hybrid:** they widen the input that board
  files and `apply` accept and bring behavior no test covered before; the
  user chose this path over handling them outside this workflow.
- **The board names its owner; the element file stays as it is:** creating
  an inner board then never removes a container's verification stamp, the
  link travels with the board, and one owner per board follows from the
  file. Storing `inner_board` on the element would rewrite the element's
  content file and drop its stamp each time an inner board is created.
- **`format` stays 1:** the new fields are optional and written only when
  present, so every existing board file stays valid and unchanged.
- **`delete_element` of an owner is rejected,** as it is while other
  elements relate to the element: what gets deleted is what the request
  names; `update_board` with `"owner": null` unlinks the board first.
- **No `delete_board` in this plan:** removing board files is a new
  persistence path of its own.
- **`apply` rejects; reading tolerates:** cycles and second inner boards are
  rejected when a request would create them, but merges and hand edits can
  still bring them, so the readers show them instead of failing. One pure
  board tree serves both, so they cannot disagree.
- **A request is rejected only for the breaches it brings in:** a project
  that a merge or a hand edit left with several breaches stays usable, and
  the AI can repair them one at a time.
- **An owner needs no home:** an element without a home is allowed, and a
  board whose owner has none is a top-level board until the owner is
  placed.
- **The outline shows the owner above and each inner board on its
  element's line,** not a list of inner boards in the header, so the AI
  sees which element to open.
- **`overview` counts the flags of each board's own elements** and caps
  its list at 50 boards like `status`.
- **`overview` cuts titles after 80 and summaries after 160 characters,**
  ending in `…`: the user chose this at the review of Task 4 over letting
  one huge title or summary make the whole overview fail at the reply
  limit; `read_board` still shows them in full.
- **`contains` passes no review flags:** the hierarchy shows in the
  outline and the overview, and a container's anchor usually covers its
  components' paths already.

## Non-Goals

- Summaries of boards that update themselves or get review flags when the
  boards below change.
- References, stubs, and implied relations across boards.
- Navigating inner boards in the canvas.
