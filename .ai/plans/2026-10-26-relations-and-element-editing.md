**Repository:** root
**Status:** InProgress
**Created:** 2026-10-26

## Goal

The MVP's core loop has the AI keep boards honest as the code changes:
review flags will travel along relations between elements, and the AI
brings flagged elements up to date. Today elements have no relations and
can only be created. Give elements typed relations, let the AI create
and delete relations and change and delete elements over MCP, and let it
read the relations: on a board through `read_board`, and per element
through a new tool `read_element`.

## Context

- Today an element holds `id`, `type`, `name`, `summary`, and a body,
  and its frontmatter is one flat mapping (Storage in Git, "Element
  File"). A request knows `create_element`, `create_board`,
  `place_element`, and `move_element`; the MCP server offers `apply`,
  `read_board`, and `search`; the canvas admits only `move_element`.
- The concept: `docs/5 Architecture/Data Model.md`,
  `docs/5 Architecture/Storage in Git.md`,
  `docs/2 Concepts/Relations.md`, `docs/2 Concepts/Board Graph.md`
  ("Deleting"), and `docs/4 AI Interface/MCP Server.md` ("Detail
  Levels").
- MCP integration tests live in `crates/abyme-mcp/tests/server.rs`.
  What the AI reads about the tools lies in
  `crates/abyme-mcp/src/server.rs` (the module documentation, the
  server's instructions, and each tool's description) and in the Tools
  section of `crates/abyme-mcp/README.md`.
- Risk categories: untrusted input (element files from the repository
  gain relations; four new operations and a new tool over MCP), data
  persistence (the store rewrites existing element files and deletes
  files), and repository text in tool output (relation labels in the
  outline, the element's description in `read_element`).

## Steps

- [x] Build elements from a fields struct
- [ ] Keep a working set of elements in `apply`
- [ ] Relations between elements
- [ ] Relations in the board outline
- [ ] The tool `read_element`
- [ ] `update_element`
- [ ] `delete_element`

## Tasks

### Task 1: Build elements from a fields struct

An element is built from its ID and one struct that names each field,
and can be taken apart into that struct again, so that later tasks
rebuild a changed element through the one constructor that checks it.
Behavior is unchanged.

Review path: Security-Hybrid

- [x] Elements are built from an ID and a struct of named fields, and an
      element can be turned back into those fields; no constructor with
      positional fields remains.
- [x] The tests in `crates/abyme-model/src/element/tests.rs`,
      `crates/abyme-model/tests/element_properties.rs`,
      `crates/abyme-core/src/apply/tests.rs`, and
      `crates/abyme-core/tests/apply_properties.rs` pass with their
      assertions unchanged; only the calls that build elements change.
- [x] `cargo test --workspace`, `cargo clippy --workspace
      --all-targets`, and `cargo fmt --all --check` pass.

### Task 2: Keep a working set of elements in `apply`

`apply` resolves and records elements through one working set with the
snapshot as fallback, as it already does for boards, so that later
operations change and remove elements in one place. Behavior is
unchanged.

Review path: Security-Hybrid

- [ ] `apply` looks up and records the elements of a request through one
      working set, with the snapshot as fallback.
- [ ] `{"existing": "<id>"}` still names only an element whose file
      exists before the request, and new element files are still written
      in the order of their creation, before every board.
- [ ] The tests in `crates/abyme-core/src/apply/tests.rs`,
      `crates/abyme-core/tests/apply_properties.rs`, and
      `crates/abyme-core/tests/project.rs` pass with their assertions
      unchanged.
- [ ] `cargo test --workspace`, `cargo clippy --workspace
      --all-targets`, and `cargo fmt --all --check` pass.

### Task 3: Relations between elements

An element holds its outgoing relations, each with a kind, a target, and
an optional label, and the AI creates and deletes them with
`create_relation` and `delete_relation`.

Review path: Security-Hybrid

- [ ] An element file lists its relations under `relations:`, one per
      line in the form `- {kind: "uses", to: "orders-db", label:
      "REST"}`, sorted by the kind's name alphabetically, then by target.
      An element without relations has no `relations` key, so every
      existing element file reads and writes as before, byte for byte.
- [ ] The relation kinds are exactly `depends_on`, `uses`, `concerns`,
      `see_also`, and `describes`, from a document to the element it
      describes; any other kind is rejected in a file and in an
      operation.
- [ ] Reading accepts relations in any YAML style. It rejects a
      `relations` value that is not a sequence, an entry that is not a
      mapping of `kind`, `to`, and an optional `label`, the same kind and
      target twice, a relation to the element itself, and a label with a
      character that `name` may not hold. Every other frontmatter rule
      holds as before. A relation whose target has no element file
      reads.
- [ ] `create_relation` with `from`, `kind`, `to`, and an optional
      `label` adds a relation from one element to another. It rejects an
      unknown source or target, a relation with the same kind and target
      that exists already, and a relation to the element itself.
- [ ] `delete_relation` with `from`, `kind`, and `to` removes that
      relation, also when the target's file is missing, and rejects one
      the source does not have.
- [ ] Both operations accept elements created earlier in the same
      request.
- [ ] The canvas refuses both operations.
- [ ] The property tests of element files generate elements with
      relations: every element written reads back equal, and arbitrary
      text never makes reading panic.
- [ ] The properties of `apply` in
      `crates/abyme-core/tests/apply_properties.rs` state the new
      contract: an existing element file changes only through an
      operation that names that element.
- [ ] An MCP test sends `create_relation` and `delete_relation` through
      `apply` and finds the result in the element files.
- [ ] A reader of Data Model and Storage in Git can tell how relations
      are stored and which operations change them; Relations and the
      Glossary name the five kinds; Decisions records how relations are
      stored and why the set of kinds is closed.
- [ ] The `apply` tool's description and the README's Tools section
      describe both operations.
- [ ] `cargo test --workspace`, `cargo clippy --workspace
      --all-targets`, and `cargo fmt --all --check` pass.

### Task 4: Relations in the board outline

`read_board` shows the relations among the elements on a board, so the
AI sees a board's arrows.

Review path: Security-Hybrid

- [ ] After the element lines and a blank line, the outline lists one
      line per relation whose source and target both lie on the board,
      as `<from> → <to>  <kind>  <label>`, sorted by source, then in the
      order of the element file. A label stays on its line and is escaped
      like every other text in the outline. A board without such
      relations shows no relation lines.
- [ ] An MCP test reads a board with relations through `read_board` and
      sees their lines.
- [ ] The outline example in `docs/4 AI Interface/MCP Server.md`, the
      `read_board` tool's description, and the README's Tools section
      match the delivered format.
- [ ] `cargo test --workspace`, `cargo clippy --workspace
      --all-targets`, and `cargo fmt --all --check` pass.

### Task 5: The tool `read_element`

A new MCP tool `read_element` returns one element's detail, so that the
AI can read an element fully and sees what points to it before deleting
it.

Review path: Security-Hybrid

- [ ] `read_element` with an element ID returns the element's ID, name,
      type, and summary, every board that places it, its outgoing
      relations, its incoming relations, and its description.
- [ ] An outgoing relation whose target file is missing shows the target
      with `⚠ missing`.
- [ ] An ID without an element file is a tool error that names the ID.
- [ ] An MCP test reads an element with outgoing and incoming relations
      through `read_element`.
- [ ] The server's module documentation and instructions, the README's
      Tools section, and the tools table and detail levels in
      `docs/4 AI Interface/MCP Server.md` describe the tool as
      delivered. The Glossary has an entry for the detail levels,
      element detail included, and the `abyme-mcp` row of the root
      `CLAUDE.md` names the tool.
- [ ] `cargo test --workspace`, `cargo clippy --workspace
      --all-targets`, and `cargo fmt --all --check` pass.

### Task 6: `update_element`

The AI changes an element's content with `update_element`, so it can
bring a flagged element up to date.

Review path: Security-Hybrid

- [ ] `update_element` changes only the fields it names among `name`,
      `type_ref`, `summary`, and `body`; `"summary": null` removes the
      summary. The element keeps its ID, also when its name changes.
- [ ] Changed values pass the same checks as on creation.
      `update_element` rejects an unknown element and an update that
      names no field. It accepts an element created earlier in the same
      request.
- [ ] The canvas refuses the operation.
- [ ] An MCP test changes an element through `apply` and reads the new
      content through `read_element`.
- [ ] Data Model, the `apply` tool's description, and the README's Tools
      section describe the operation as delivered.
- [ ] `cargo test --workspace`, `cargo clippy --workspace
      --all-targets`, and `cargo fmt --all --check` pass.

### Task 7: `delete_element`

The AI deletes an element with `delete_element`. The deletion is
rejected while other elements still relate to it, which is the warning
beforehand that Board Graph asks for.

Review path: Security-Hybrid

- [ ] `delete_element` removes the element's file and its placement on
      every board that places it. It also takes a placed element whose
      file is missing and removes its placements.
- [ ] It rejects an element that other elements still relate to, as the
      request stands at that operation: relations that an earlier
      operation of the request deleted, or whose source it deleted, no
      longer count. The error names the ID and relation kind of each
      remaining source, never a label.
- [ ] It accepts an element created earlier in the same request; that
      element's file is then never written.
- [ ] Later operations of the same request cannot refer to a deleted
      element, and an element created later in the request does not get
      its ID.
- [ ] The files are written in this order: new element files, changed
      element files, boards, removals. So no board ever places an element
      whose file was removed, even if writing stops partway. Removing a
      file that is already gone counts as removed.
- [ ] The canvas refuses the operation.
- [ ] The properties of `apply` state that a placement disappears only
      through `delete_element`.
- [ ] An MCP test deletes an element through `apply`, and `read_board`
      no longer shows it. A second test is refused while a relation
      points to the element.
- [ ] Data Model, Storage in Git ("Writing"), Board Graph ("Deleting"),
      the `apply` tool's description, and the README's Tools section
      describe deletion as delivered.
- [ ] `cargo test --workspace`, `cargo clippy --workspace
      --all-targets`, and `cargo fmt --all --check` pass.

## Decisions

- **Relations in the frontmatter:** one line per relation, sorted, the
  key left out when empty. One changed relation is one changed line, and
  existing files stay valid. Other options were declined:
  - a separate file per element splits what Data Model makes part of
    the element;
  - the board file mixes content into layout;
  - a key per kind leaves no room for the label;
  - a string format inside YAML needs a second parser.
- **A closed set of five kinds:** a later plan's check policy matches
  each kind exhaustively.
  - `contains` comes with inner boards, which it usually follows from.
  - `corresponds_to` and `replaces` come with current and target, after
    the MVP.
  - Whether `describes` triggers a task or a hint is decided with the
    review flags.
- **Deleting an element with incoming relations is rejected.**
  - Removing those relations along with it would silently change other
    element files.
  - Leaving them would make Abyme create the dangling relations its
    structural check reports.
- **A placed element whose file is missing can be deleted:** otherwise
  no operation could clear a `⚠ missing` line.
- **`update_element` is partial:** the AI need not resend a long
  description to fix a summary.
- **The relation operations are named `create_relation` and
  `delete_relation`,** following the verb-object names of the other
  operations.
- **Tasks 1 and 2 keep behavior unchanged:**
  - `{"existing": …}` keeps meaning "exists before the request";
  - new element files keep their order of creation.
- **A deleted element's ID stays taken within its request:** no write
  batch removes and recreates the same file.
- **Write order: new elements, changed elements, boards, removals.**
  - A crash between two renames can leave an element without a home,
    which Data Model allows.
  - It can also leave a relation between two new elements dangling,
    which reading accepts.
- **A file already gone at removal counts as removed:** the request's
  boards are already written by then, and the element is gone either
  way.
- **Relations stay out of the search index's text:** nothing searches
  by relation, and every new relation would embed its source again.
- **Relation kinds sort by name:** the element file and the outline use
  the same order, which a reader of either can follow without knowing
  the code.
- **Within a request, deletion sees the request's own changes,** as every
  other operation does: operations run in order.

## Non-Goals

- Anchors, verification stamps, review flags, and the structural check
  that reports dangling relations: the next plan.
- `contains`, inner boards, and `overview`.
- Arrows and relation editing in the canvas, which refuses the new
  operations.
