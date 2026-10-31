**Repository:** root
**Status:** NotStarted
**Created:** 2026-10-31

## Goal

The AI builds a C4 model through the MCP server and should not have to
compute coordinates. Let a request lay out a board as a layered diagram from
its elements and relations, size each box from its text, and keep what a
person placed by hand where it is.

## Context

- The concept: `docs/4 AI Interface/Placement and Layout.md`,
  `docs/5 Architecture/Data Model.md`, `docs/5 Architecture/Storage in Git.md`,
  and `docs/5 Architecture/Canvas.md`.
- Boards live in `crates/abyme-model/src/board.rs`; `apply` and the operations
  in `crates/abyme-core/src/apply.rs` and `operation.rs`, where `apply` is pure
  and a request writes all or nothing; free-space placement in
  `crates/abyme-core/src/layout.rs`; what the canvas may write in
  `crates/abyme-canvas/src/admission.rs`; how it draws a box in
  `crates/abyme-canvas/ui/src/ElementBox.tsx` and `canvas.css`.
- Readers of the new placement field: `read_board` with coordinates, every
  writer of a board file, and the layout itself.
- Risk categories: untrusted input (board files gain a field; `apply` gains an
  operation and a field on `move_element`; element text decides box sizes), a
  trust boundary (the one write the canvas may send changes meaning, and a page
  from the dev server's address can send it), data persistence (a layout
  rewrites many lines of a board file), availability (the cost of a layout on a
  hostile board while the write lock is held), and supply chain (`unicode-width`
  becomes a direct dependency).

## Steps

- [ ] `Rect` knows its edges and overlaps
- [ ] A board holds placements, not bare rectangles
- [ ] Free-space placement takes a size
- [ ] Pinned placements
- [ ] Box sizes from text
- [ ] `layout_board`

## Tasks

### Task 1: `Rect` knows its edges and overlaps

The layout code and its tests would otherwise define overlap a third time.
Behavior is unchanged; the tests in `crates/abyme-core/src/layout.rs` and the
overlap property in `crates/abyme-core/tests/apply_properties.rs` pin it.

Review path: Reviewer-only

- [ ] `Rect` answers its right and bottom edge and whether it overlaps another
      rectangle, and no other definition of these remains in
      `crates/abyme-core/src/layout.rs` or
      `crates/abyme-core/tests/apply_properties.rs`.
- [ ] Those tests pass with their assertions unchanged.
- [ ] `cargo test --workspace`, `cargo clippy --workspace --all-targets`, and
      `cargo fmt --all --check` pass.

### Task 2: A board holds placements, not bare rectangles

`Board.placements` maps an element to a `Placement` holding its rectangle, so
that the pin of the next tasks has one home and cannot exist without a
placement. Behavior is unchanged, and the bytes of every board file with it;
the tests in `crates/abyme-model/src/board.rs` and
`crates/abyme-model/tests/board_properties.rs` pin the file, those in
`crates/abyme-core/tests/apply_properties.rs` the writes.

Review path: Security-Hybrid

- [ ] `Board.placements` holds a `Placement` per element, and every reader and
      writer goes through it.
- [ ] Those tests pass, changed only mechanically to reach the rectangle
      through the placement.
- [ ] Data Model and the rustdoc of `Board` describe the type as delivered.
- [ ] `cargo test --workspace`, `cargo clippy --workspace --all-targets`, and
      `cargo fmt --all --check` pass.

### Task 3: Free-space placement takes a size

`Layout` means free space, while the name belongs to the layered layout to
come, and a box taller than one grid row would overlap the row below it. Give
free-space placement a size and split the module. Behavior is unchanged: every
caller still passes 180 × 90, which takes one row, so every position stays as
it is; the tests in `crates/abyme-core/src/layout.rs`, the placement tests in
`crates/abyme-core/src/apply/tests.rs`, and the overlap property in
`crates/abyme-core/tests/apply_properties.rs` pin it.

Review path: Reviewer-only

- [ ] Free-space placement lives in a module of its own under `layout/`, under
      a name that says free space, and takes the size of the box.
- [ ] A box taller than one grid row takes a cell whose column has enough free
      rows below it, and no placement overlaps it; a new test covers this.
- [ ] The named tests pass with their assertions unchanged, changed only
      mechanically for the new name.
- [ ] `cargo test --workspace`, `cargo clippy --workspace --all-targets`, and
      `cargo fmt --all --check` pass.

### Task 4: Pinned placements

A placement can be pinned: a person's move in the canvas pins it, and neither
the AI nor a layout moves it without saying so.

Review path: Security-Hybrid

- [ ] A placement line in a board file holds an optional `"pinned": true` at
      its end, written only when true, so a board without a pin reads and writes
      byte for byte as before. `false` reads as not pinned and is written as
      nothing; `null` or a value that is no boolean is rejected like other
      invalid fields. The test of an unknown placement field uses a field that
      stays unknown.
- [ ] `move_element` takes an optional `pinned`. Without it, an unpinned
      element moves and stays unpinned, and a pinned one is rejected with a
      message that tells the AI to ask first. `pinned: true` moves and pins,
      `pinned: false` moves and unpins.
- [ ] The canvas admits `move_element` only with `pinned: true` and sends that
      for every drag and arrow key, also for several boxes at once; any other
      value is refused like an operation it may not send.
- [ ] `read_board` with coordinates shows `pinned` with the position of a
      pinned element; without coordinates, and for an unpinned element, its output
      stays as it is.
- [ ] Unit and property tests cover the file, each cell of the rule on
      `move_element`, and deleting a pinned element, whose placement goes with it;
      IPC and Vitest tests cover what the canvas sends and refuses;
      an integration test through the MCP tool `apply` moves a pinned element
      without and with `pinned` and reads the board file back.
- [ ] Data Model, Storage in Git, Placement and Layout, Canvas, MCP Server, the
      Glossary, the `apply` and `read_board` descriptions, and the MCP README tell
      the reader what a pin does, how to set and clear one, and how `read_board`
      shows it.
- [ ] `cargo test --workspace`, `cargo clippy --workspace --all-targets`,
      `cargo fmt --all --check`, `pnpm --dir crates/abyme-canvas/ui check`, and
      `pnpm --dir crates/abyme-canvas/ui test` pass.

### Task 5: Box sizes from text

A new placement gets a box that fits its element's type, name, and summary, so
that the canvas does not cut off what it shows.

Review path: Security-Hybrid

- [ ] One pure function gives a box's size from an element's type, name, and
      summary, counting character widths with `unicode-width`: the width stays
      180, the height grows by whole lines from 90 up to a fixed maximum, and it
      reads no more text than fits that maximum. Text that fits three lines gives
      180 × 90.
- [ ] `place_element` places an element at the size of its text and overlaps
      nothing. `move_element` and `update_element` leave a placement's size as it
      is, and an existing placement keeps its size.
- [ ] The canvas draws a box's text with the line height and the wrapping the
      function assumes, and the function and the styles each say that they change
      together.
- [ ] Unit tests cover the function for short text, wide characters, an
      unbroken long word, and text far beyond the maximum; the tests of
      `place_element` cover a tall box.
- [ ] A reader of Canvas, Placement and Layout, and Data Model's rule on
      placement can tell how a box gets its size and why a box can still cut off
      text, and the Decisions table holds the estimate instead of a font
      measurement.
- [ ] `cargo test --workspace`, `cargo clippy --workspace --all-targets`,
      `cargo fmt --all --check`, `cargo deny --locked check`, `pnpm --dir
      crates/abyme-canvas/ui check`, and `pnpm --dir crates/abyme-canvas/ui test`
      pass.

### Task 6: `layout_board`

A request lays out a board: `{"op": "layout_board", "board": <ref>}` arranges
it as a layered diagram, in order with the request's other operations.

Review path: Security-Hybrid

- [ ] `layout_board` takes a board, also one the same request creates, and no
      other field, and sees the elements, relations, and placements of the request
      as it stands at that operation. It writes only the board file, so no
      verification stamp changes.
- [ ] A relation of the kind `uses`, `depends_on`, `describes`, or `concerns`
      between two elements on the board ranks its source above its target. A walk
      over the elements in ID order leaves out, for the ranks only, each relation
      that would close a cycle; every other relation holds, so a tree hanging off a
      cycle stays ranked. `see_also` only influences the order within a rank.
      Relations to elements not on the board count for nothing.
- [ ] Between two unpinned boxes, the box of a relation's source lies entirely
      above the box of its target, for every relation the walk does not leave out.
- [ ] A pinned element takes part in the ranks like any other, so a chain through
      it stays connected, but it keeps its position and size. No unpinned box
      overlaps a pinned one or another unpinned one.
- [ ] Each connected part of the board is laid out on its own, the parts side
      by side by their smallest element ID, and boxes without relations stand in a
      grid below them.
- [ ] Every unpinned box gets the size of its text, or 180 × 90 when its element
      has no file.
- [ ] The result depends only on IDs, sizes, relations, and pins: laying out a
      board twice writes the same bytes. Every unpinned box lies within ±1,000,000,
      the range of `move_element`; pinned boxes do not count. A layout that cannot
      keep to it rejects the request with the existing `BoardFull`.
- [ ] A `place_element` after `layout_board` in the same request overlaps no box
      the layout produced.
- [ ] The canvas refuses `layout_board` as it refuses the other operations it
      may not send.
- [ ] Unit and property tests cover ranks, cycles with a tree hanging off them,
      disconnected parts, pinned elements, repeating a layout, the range, and a
      `place_element` after a layout; an integration test through the MCP tool
      `apply` builds a board with relations and lays it out in one request, reads
      the board file back, then lays it out again with a pinned element.
- [ ] A reader of Placement and Layout, MCP Server, Data Model, Decisions, the
      Glossary, the `apply` description, the server's instructions, and the MCP
      README can use `layout_board` and sees that semantic placement and a tool
      `place` are not built yet; the root `CLAUDE.md` Components table names the
      layout.
- [ ] `cargo test --workspace`, `cargo clippy --workspace --all-targets`,
      `cargo fmt --all --check`, and `pnpm --dir crates/abyme-canvas/ui check`
      pass.

## Decisions

- **`layout_board` is an operation of `apply`, not a tool `place`:** it runs
  in the same request as the content, so one call builds and arranges a board
  and a failure writes nothing. An option on `create_board` was declined:
  relations come after the board, and an existing board could not be laid out
  again.
- **The layered layout is our own code:** `rust-sugiyama` 0.4.0 and
  `layout-rs` 0.1.3 are MIT, but neither knows fixed obstacles for pinned
  elements, and `rust-sugiyama` takes no box sizes and uses `HashMap`
  internally. Crossings on dense graphs are reduced less well in exchange.
- **Box sizes come from an estimate by character width:** the canvas shows the
  system font, which no measurement in Rust can match; a bundled font or a text
  layout crate would cost far more, and a later measurement can replace the one
  function.
- **`unicode-width` becomes a direct dependency of the core:** it is in
  `Cargo.lock` already, and it keeps the table of wide characters that an own rule
  would have to maintain.
- **A drag or arrow key in the canvas pins the element:** otherwise nothing
  sets the field in this plan, and a layout could overwrite what a person placed
  by hand. Boxes dragged before this plan are not pinned, because nothing recorded
  them.
- **A pin is a convention, not a permission:** `apply` has no notion of who
  writes, so `move_element` rejects a pinned element unless the request says what
  it wants with `pinned`, and the AI is told to ask first.
- **A layout moves every unpinned box and ignores old positions:** the result
  then depends only on the board's content, and a repeated layout changes no byte.
  Starting from old positions was declined as hard to test and possibly never at
  rest; moving only new elements was declined because no file records what is new.
  `place_element` remains the way to add a box without moving others.
- **The AI may lay out a mirrored board,** since only the AI changes one.
- **Sizes change only through `place_element` and `layout_board`:** a content
  change rewrites no board file, so a renamed box can be cut off until the next
  layout. `delete_element` also removes a pinned placement, since a pin protects a
  position, not an element.
- **`Placement` as the map's value:** a `pinned` field on `Rect` would mix
  geometry and state, and a separate set on the board would allow a pin without a
  placement.

## Non-Goals

- Semantic placement ("to the right of Z") and a tool `place`.
- Dummy nodes for long edges, arrow routing, and a choice of direction.
- A layout button, a pin marker, an unpin gesture, and resizing in the canvas.
- A real font measurement and a bundled font.
- Layout across boards, `create_from_template`, and frames.
