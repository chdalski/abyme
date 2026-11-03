**Repository:** root
**Status:** InProgress
**Created:** 2026-11-03

## Goal

A sticky note on the canvas lies loose beside the box it comments on, so moving the box leaves it
behind, and nothing tells the AI which box it means. Let a person and the AI attach a note to a box:
the note gets the relation `concerns` to the box, and on that board it moves along with the box until
it is detached.

## Context

- The concept: `docs/2 Concepts/Presentation.md` ("Sticky Notes", "Attaching"),
  `docs/5 Architecture/Data Model.md` ("Rules", "Operations"), `docs/5 Architecture/Storage in Git.md`
  ("Board File", "Later fields"), `docs/4 AI Interface/Placement and Layout.md`, and
  `docs/5 Architecture/Canvas.md`.
- The model and the core: `crates/abyme-model/src/board.rs` (`Placement`, the one line per placement),
  `crates/abyme-core/src/apply.rs` (the operations), `operation.rs` (their format and the MCP schema),
  `layout/` (free space and the layered layout), and `outline.rs` (what `read_board` shows).
- The canvas: `crates/abyme-canvas/src/admission.rs`, `projection.rs`, and `ui/src/` (`nodes.ts`,
  `boardNodes.ts`, `BoardPane.tsx`, `edges.ts`, `client.ts`).
- The MCP server's tools `apply` and `read_board` describe the operations and the output in
  `crates/abyme-mcp/src/server.rs`; `crates/abyme-mcp/README.md` and
  `docs/4 AI Interface/MCP Server.md` describe them for people.
- Risk categories: untrusted input (a board file from a clone gains a field, and an operation two
  forms), an access decision (what the canvas may attach, also on mirrored boards), and data
  persistence.

## Steps

- [x] One way to read a position
- [x] Attached notes in the board file
- [x] Attaching and detaching a note
- [ ] Attaching on the canvas

## Tasks

### Task 1: One way to read a position

Today each reader interprets a placement's stored rect itself, among them free space, the layered
layout, `move_element` and `layout_board`, the canvas's admission and projection, and the outline.
Every production reader goes through the board first. Behavior does not change.

Review path: Security-Hybrid

- [x] Every production site that reads where an element lies on a board asks the board for it; no
      production code outside `abyme-model` reads a placement's stored rect as a position.
- [x] Every existing test passes unchanged, among them the tests of free space, of the layered layout,
      the property tests of `apply`, and the tests of the outline, the admission, and the projection,
      which pin these reads; the board's new way to read a position has tests of its own.
- [x] `cargo test --workspace`, `cargo clippy --workspace --all-targets`, and
      `cargo fmt --all --check` pass.

### Task 2: Attached notes in the board file

A placement can name the element it hangs on, with a position relative to it, and everything that
reads or changes positions honors it. Nothing writes an attachment yet; a board file written by hand
or by a later task already reads and behaves right.

Review path: Security-Hybrid

- [x] A placement line can end with `"attached_to": "<id>"`, written only when present and read back
      unchanged, so attaching, detaching, or moving a note changes exactly one line, and moving its
      box changes only the box's line. A malformed value is refused as any other malformed field is.
- [x] An attachment holds only when the element it names lies on the same board, is not the placed
      element itself, and is not attached itself. Then the placement's position is its box's
      position plus its stored one; otherwise the stored position is a board position, and the file
      keeps the field as written.
- [x] Moving a box moves the notes that hang on it on that board, with no line of theirs written, and
      the free space of a request counts them where they now are. A move of an attached note takes a
      board position and stores it relative to its box.
- [x] An attachment whose board position would lie outside the range of `move_element`, -1,000,000
      to 1,000,000, does not hold when read. No request leaves an attached note's board position
      outside that range: a move or a layout of its box, or a move of the note, that would is refused,
      writing nothing.
- [x] A layout of the board leaves an attached note on its box, neither moving it nor letting it block
      another box, and counts none of its relations.
- [x] Deleting an element that notes hang on writes those notes' board positions on every board where
      they hung, so none jumps.
- [x] `read_board` with coordinates shows an attached note's board position and the box it hangs on,
      and says so when an attachment does not hold; without coordinates it is unchanged. The canvas's
      `open_board` returns board positions.
- [x] A board file without `attached_to` reads, lays out, and writes exactly as before, as the
      existing tests show.
- [x] Rust tests cover each rule above, also with board files written by hand. A property test with
      attached notes among the generated boards shows, after any request of moves, layouts, and
      deletions: each attached note's board position is its box's plus its stored one and lies in
      range, a repeated layout changes no byte, and pins and freedom from overlap hold for every
      placement that is not attached.
- [x] A reader of Storage in Git, Data Model, and Placement and Layout can tell how an attachment is
      stored and read, and what moves along with a box. The tool description of `read_board`, its
      text in `crates/abyme-mcp/README.md` and in `docs/4 AI Interface/MCP Server.md`, and Canvas's
      sentence on what `open_board` returns state the attached note's position.
- [x] `cargo test --workspace`, `cargo clippy --workspace --all-targets`, and
      `cargo fmt --all --check` pass.

### Task 3: Attaching and detaching a note

Two new operations, for the AI and the canvas alike, keep the relation and the attachment together:
attaching writes both, all or none.

Review path: Security-Hybrid

- [x] `attach_note` takes a note, a board, and an element that is no note, both on that board. It adds
      the relation `concerns` from the note to the element if the note has none yet, keeping an
      existing one with its label, and attaches the note there where it lies now, so it does not
      move. Attaching an attached note to another element moves the attachment; the old relation
      stays.
- [x] `detach_note` takes a note and a board. It ends the attachment where the note lies now; the
      relation stays.
- [x] Neither changes a pin. `attach_note` removes the note's verification stamp when it adds the
      relation, as `create_relation` does, and keeps it when the relation exists; `detach_note` changes
      no element file. Each refuses, writing nothing, an element that is no note, an element no board
      places, and attaching to a note or to itself.
- [x] Deleting the relation `concerns` from a note to an element also detaches the note from that
      element on every board, so an attached note always has the relation.
- [x] The MCP tool `apply` describes both operations and the effect of deleting the relation, and its
      schema takes them. The canvas's admission refuses both, naming no other text.
- [x] Rust tests cover each rule through `apply` in the core and through the MCP server's `apply`.
- [x] A reader of Data Model, Presentation, and the Glossary's entry on attaching can tell how a note
      is attached and detached, and that only a note hangs on an element that is no note. No sentence
      of Data Model's Operations or of `crates/abyme-mcp/README.md` lists the operations without the
      two, or says that `delete_relation` rewrites only the element file.
- [x] `cargo test --workspace`, `cargo clippy --workspace --all-targets`, and
      `cargo fmt --all --check` pass.

### Task 4: Attaching on the canvas

A person attaches a note by dropping it onto a box and detaches it by dragging it off, on every board.

Review path: Security-Hybrid

- [ ] A note dropped with its centre inside a box that is no note is attached there; where its centre
      lies inside several such boxes, to the one drawn on top. Dragged off with its centre outside its
      box and inside no other, it is detached; dropped inside another box, it is attached there with
      one `attach_note` and no `detach_note`. Each goes out in the same request as the move, in order
      with the canvas's other writes. A move that ends on the same box attaches nothing.
- [ ] Moving a note with the arrow keys attaches and detaches it the same way.
- [ ] An attached note moves on the screen with its box while the box is dragged, and a drag of the
      box writes only the box's move.
- [ ] The admission takes `attach_note` and `detach_note` of an existing note, on every board, also a
      mirrored one; any other form is refused by position, naming no other text.
- [ ] The canvas does not draw the relation `concerns` from an attached note to its box as an arrow;
      the note's other relations still show.
- [ ] Rust tests cover the admission through `apply` over IPC with the bytes of the files; Vitest
      covers the drop, the drag off, a drop from one box onto another, a drop on overlapping boxes,
      the arrow keys, a drop on a note, a drag of the box, and the hidden arrow.
      `admitted.typecheck.ts` covers the types.
- [ ] A reader of Canvas can tell how a note is attached and detached, and `First Slice.md`'s MVP and
      the canvas row of the root `CLAUDE.md` state what is built.
- [ ] `cargo test --workspace`, `cargo clippy --workspace --all-targets`,
      `cargo fmt --all --check`, `pnpm --dir crates/abyme-canvas/ui check`, and
      `pnpm --dir crates/abyme-canvas/ui test` pass.

## Task Notes

- Task 1: the security engineer's pre-gate conditions. The admission's `lies_at` keeps refusing an
  element its board does not place, with no fallback rect. `Board::placed` yields every placement
  once, in the order of `placements`, unfiltered, since `layout_board` zips the layout's rects back in
  that order, and its doc comment says so. `move_element` still occupies the moved box, taking its
  position from the board, with no `unwrap`. Projection and outline take width and height from the
  same rect as the position. Parsing, the writes of the stored rect, the range check, and the error
  texts stay unchanged. Neither method scans all placements per item.
- Task 2: `Placement.rect` stays public, so reading it as a position is only a convention; the
  security engineer will ask in Task 2's pre-gate for something that makes a new direct read visible,
  and each later review reruns the search for `.rect` reads outside `abyme-model`.
- Task 2: `move_element` now reports `NotOnBoard` when `rect_of` finds no position after the write,
  which cannot happen yet; once an attachment can make `rect_of` differ, decide what that branch means.
- Task 3: `Board::named_attachment` reads the field as written, and the lint on the fields does not
  see a call to it. Before attaching or detaching adds a reader, add `disallowed-methods` for it to
  `clippy.toml`, with one `#[expect]` in the outline's `position`; that change goes through the
  task's security gate (security engineer, Task 2's post-gate).
- Task 4: the hold rule ignores element types, so a hand-written file can hang a box that is no note
  on a note, and since Task 3 `update_element` reaches such states too, by changing the type of an
  attached note or of its box; on a mirrored board an admitted move or deletion of that note then
  carries the box along. When attaching is admitted there, decide from the element files whether
  such a move or deletion is refused (security engineer, Task 2's pre-gate and Task 3's).
- Task 4: `Board::attach` succeeds while other placements hang on the element, which then stop
  holding; `attach_note` refuses that case, so call `attach` only through `apply`, or make it
  return `DoesNotHold` there (security engineer, Task 3's post-gate, Low).
- Task 4: `attach_note` refuses a pinned note without `"pinned": true`, so the canvas sends it on the
  attachment that follows its own pinned move, and the admission requires it there.

## Decisions

- **All positions go through the board first, as its own task:** seven readers take the stored rect
  as a board position, and a relative one would compile everywhere and be wrong everywhere.
- **An attached note's position is stored relative to its box:** moving the box changes only its line,
  merges stay line by line, and a hand edit of the box carries its notes along. Absolute positions
  that the core rewrites with the box were declined: a box's move would change several lines and
  merges would conflict.
- **An attachment that does not hold reads as no attachment:** a merge or a hand edit can break it, and
  readers show such boards rather than fail, as they do for a broken board tree. So a chain of
  attachments reads as broken, and the core never writes one.
- **New operations `attach_note` and `detach_note`, and positions in operations stay board positions:**
  `move_element` and `layout_board` keep writing only the board file, and neither the AI nor the
  canvas computes an offset or needs to know whether the relation exists. A field on `move_element`
  and attaching through `create_relation` were declined: the first makes a move write element files,
  the second fails on a relation that exists.
- **Deleting the relation ends the attachment:** an attached note always has the relation, so the board
  shows nothing the content does not hold.
- **Only a note hangs on an element, and only on one that is no note:** it is what the MVP needs, and
  it rules out chains.
- **The canvas hides the arrow `concerns` from an attached note to its box:** the note visibly hangs on
  it, and the AI's outline still shows the relation.
- **Arrow keys attach and detach like a drag:** the rule is the note's centre, however it moves.
- **Positions stay within the range of `move_element`:** a board file or a request cannot put an
  attached note's board position beyond it, so every position still survives a JavaScript number.
- **An attached note is a child node of its box on the canvas:** it follows a drag of the box live, and
  the view still carries board positions. Computed board positions without a parent node were
  declined: the note would jump after the reload instead of following.
- **A replacement of the stored rect by a position type was declined:** it would touch every reference
  to the rect, most of them in tests, and the first task gives most of its safety.
- **Gate demands on Task 2 (security engineer):** a clippy `disallowed-fields` rule on the stored
  `rect` and `attached_to` of a placement, so that only `abyme-model` and tests read them; every write
  of a position goes through `Board::set_rect`, after which the core checks that the board reads the
  element where the operation put it, which replaces the `NotOnBoard` after a move's write; canvas
  admission tests for an attached note; and a property test that generates broken attachments too.
- **A layout leaves out every placement that names a box on the board, even one whose attachment does
  not hold:** which placements take part then depends on no position, so a repeated layout changes no
  byte; a note whose attachment does not hold because of the range keeps its position. Found by the
  property test, decided with the security engineer.
- **Hand-written states may move a note:** in a hand-written chain, a deletion can make a note start
  to hold; a note whose attachment does not hold only because of the range can start to hold when its
  box moves; and a stale `attached_to` holds once its element is placed. The core never writes these
  states, and the result stays in range. Accepted by the security engineer.
- **`attach_note` takes `"pinned": true` as the person's word for a pinned note:** without it, the
  AI could hang a person's pinned note on a box and then move or lay out the box, which carries the
  note along. Only `true` is accepted, so attaching never changes a pin, and releasing one stays the
  job of `move_element`. The user chose this over a `pinned` that also releases, and over accepting
  the risk (security engineer, Task 3's pre-gate).
- **Gate demands on Task 3 (security engineer):** `Board::named_attachment` is a disallowed method
  outside `abyme-model` but for the outline; deleting `concerns` rewrites only the boards whose file
  names the attachment, and clears it there also when it does not hold; `Board::attach` restores the
  placement on any failure; error texts name only IDs.
- **The hold rule still ignores element types:** a state with a box that is no note hanging on a note,
  reached by hand or by `update_element`, reads and moves as any attachment; a stale `attached_to` on
  another element that names an attached note keeps reading as a board position. Accepted by the
  security engineer.
- **All tasks are Security-Hybrid:** the board file and the operations take new input, and the canvas
  decides what it may attach, also on mirrored boards.

## Non-Goals

- Showing attached notes where their box lies only as a reference, an open point of Presentation.
- Attaching anything but a note, and chains of attachments.
- Resizing a note by hand, and writing its grown size.
- An export, such as JSON Canvas, that resolves attachments.
