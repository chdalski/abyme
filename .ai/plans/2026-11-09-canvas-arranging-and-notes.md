**Repository:** root
**Status:** InProgress
**Created:** 2026-11-09

## Goal

People working on the same boards need to arrange a mirrored board for everyone and to leave notes
that say which element they are about. Boxes on a mirrored board can be arranged as on a free one,
a note created on a box hangs on it so it moves with it and the AI sees which element it concerns,
notes are placed with a note-mode button where the user clicks and look like speech bubbles, and
highlighting a selection keeps the notes on a selected box undimmed.

## Context

- The canvas is `crates/abyme-canvas`: Rust side in `src/` (what it may write in `admission.rs`, a
  board's projection in `projection.rs`, IPC commands in `app.rs`), UI in `ui/src/` (the board in
  `BoardCanvas.tsx`, its nodes and the drop rule in `nodes.ts` and `boardNodes.ts`, note text in
  `NoteContent.tsx`, the requests it may send in `client.ts` with their type checks in
  `admitted.typecheck.ts`, highlighting in `edges.ts`). `docs/5 Architecture/Canvas.md` describes
  what the canvas does; each task brings it in line with the behavior it changes.
- **Drop rule:** where a note the user drops lands: on the box that is no note, whose file exists,
  and that holds the note's centre, the one drawn on top where boxes overlap; beside every box it
  stands free (`onTheBoardAndHung` in `nodes.ts`).

## Steps

- [x] Notes on a selected box stay undimmed
- [ ] Mirrored boards are arranged like free ones
- [ ] One function finds the box a point lies on
- [ ] A new note created on a box hangs on it
- [ ] A note-mode button places a note where the user clicks
- [ ] Notes look like speech bubbles pointing to their box

## Tasks

### Task 1: Notes on a selected box stay undimmed

Selecting a box keeps the notes hanging on it undimmed, as selecting a note already keeps its box
undimmed.

Review path: Reviewer-only

- [x] With a box selected, every note hanging on it on the board is undimmed; with a note selected,
      its box stays undimmed as before.
- [x] Tests through `BoardCanvas` show both directions.
- [x] `docs/5 Architecture/Canvas.md` says which boxes and notes a selection keeps undimmed.

### Task 2: Mirrored boards are arranged like free ones

On a mirrored board a person may now move and pin every box whose element file exists, and release a
pin, as on a free board; what a mirrored board shows still follows the code.

Review path: Security-Hybrid

- [ ] On a mirrored board, dragging or moving with the arrow keys a box whose element file exists
      moves it and pins it, and its marker releases the pin; a box whose element file is missing
      stays where it is, as on a free board.
- [ ] The canvas's admission refuses nothing on a mirrored board that it admits on a free board, and
      admits nothing there that it refuses on a free board; a release still keeps the position in
      the board file.
- [ ] The board view the UI receives carries no mode, and `ui/src/bindings.ts` matches the Rust
      types.
- [ ] Rust tests of admission and of the command `apply` show moves, pins, and releases of boxes on
      a mirrored board written; UI tests show such boxes draggable and their pin releasable.
- [ ] No text an AI, a developer, or a reader of the code or docs gets still gives the old rule of
      what a person may change on a mirrored board: the MCP tool descriptions, the abyme-mcp
      README, the doc comment of `Mode::Mirrored`, the canvas's module docs, the `abyme-canvas` row
      of the root `CLAUDE.md`, `Canvas.md`, `Boards and Zones.md`, and the summary of the element
      `admission` say what mirrored now means; `docs/Decisions.md` holds a row for the decision.

### Task 3: One function finds the box a point lies on

The part of the drop rule that finds the box under a point moves into a function of its own, so
that creating a note can use it too. Behavior is unchanged.

Review path: Reviewer-only

- [ ] One exported function in `nodes.ts` finds the box a point of the board lies on by the drop
      rule, and `onTheBoardAndHung` uses it.
- [ ] The tests of `onTheBoardAndHung` and of dropping notes on boxes in `nodes.test.ts` pass with
      their assertions unchanged.

### Task 4: A new note created on a box hangs on it

A note created with `N` or a double-click is centred on the point where it is created, and when it
is written it hangs on the box its centre lies on by the drop rule, in the same request that
creates it. The core already applies a request that creates, places, pins, and attaches a note.

Review path: Security-Hybrid

- [ ] `N` over a box that is no note and whose file exists creates a note centred on the mouse;
      once its text is written it hangs on that box, with the relation `concerns` and its
      placement attached, in one request; beside every box it is written free, as today.
- [ ] A note created by a double-click on an empty spot or by the toolbar is also centred on its
      point, and hangs on a box only where its centre lies on one when it is written.
- [ ] The canvas admits a request that creates one note and attaches that new note to an existing
      element on the same board; it admits no creating request with any other attachment or any
      other added operation; the existing requests that move and attach or detach a note stay
      admitted as they are; the UI's types allow exactly these shapes.
- [ ] If the box is gone or no longer under the note when its text is written, the note is written
      free.
- [ ] Rust tests of admission and of the command `apply` cover the admitted request and its refused
      variants; UI tests cover creating on a box, beside a box, and a box removed while the text is
      typed.
- [ ] `docs/5 Architecture/Canvas.md` and `docs/2 Concepts/Presentation.md` describe where a note
      appears and that creating it on a box hangs it there, as dropping it there does.

### Task 5: A note-mode button places a note where the user clicks

The toolbar's button switches into note mode; the next press on the board places a note there and
ends the mode.

Review path: Security-Hybrid

- [ ] The toolbar button, with a speech-bubble icon and an accessible name, switches note mode on
      and off and shows its state.
- [ ] In note mode, a press on an empty spot places a free note centred there, and a press anywhere
      on a box, its buttons and markers included, places a note there that hangs on the box when
      written, as in Task 4; the press moves, selects, pans, opens, and releases nothing, and the
      mode ends.
- [ ] `Esc` outside a text field ends note mode and places nothing; `N` places a note at the mouse,
      as it does outside note mode, and ends note mode; the zoom bar and the toolbar work as before
      while it is on; opening another board ends it.
- [ ] Tests through `BoardCanvas` cover a press on a box, on a box's button, on an empty spot, the
      end of the mode after one note, `Esc`, and `N`.
- [ ] `docs/5 Architecture/Canvas.md` describes note mode.

### Task 6: Notes look like speech bubbles pointing to their box

A note shows as a speech bubble; a note hanging on a box points with its tail toward that box. The
side is chosen from the centres of the note and the box: along the axis on which they lie farther
apart, toward the box; on a tie, the vertical side; with equal centres, and for a free note, a
note being created, or a note whose box is gone, the bottom.

Review path: Security-Hybrid

- [ ] A note hanging on a box shows its tail on the side the rule above names; every other note
      shows it at the bottom.
- [ ] The tail takes no clicks and covers no text of the note.
- [ ] Unit tests cover a box whose centre lies above, right of, below, and left of the note's,
      overlapping rects, a diagonal tie, and equal centres; tests through `BoardCanvas` show the
      tail of an attached and of a free note.
- [ ] `docs/5 Architecture/Canvas.md` and `docs/2 Concepts/Presentation.md` describe the look.

## Decisions

- **Admission no longer looks at a board's mode:** `docs/2 Concepts/Boards and Zones.md` and
  `Canvas.md` say that only reconciliation, that is the AI, changes a mirrored board and that a
  person moves only notes there; this plan reverses that: a person arranges a mirrored board as a
  free one, while what it shows still follows the code. Making it the absence of a rule, not a
  second rule, leaves admission free of modes; a later mode that really restricts people adds its
  own.
- **The board view drops its mode:** after Task 2 nothing in the UI reads it.
- **A note is created and attached in one request:** the note can never be created and then fail to
  attach.
- **The box a new note hangs on is decided when its text is written, by the drop rule:** a note can
  always be saved; if its box was removed or moved while the user typed, it is written free.
- **New notes are centred on their point:** "the click lands on a box" and "the note's centre lies on
  the box" are then the same test, the one a dropped note follows.
- **Note mode ends after one note, and the other ways to create a note stay:** one click makes one
  note, so no further click creates one by accident; `N` keeps placing at the mouse, also in note
  mode, which it then ends; a double-click on an empty spot keeps creating a free note, and one on a
  box still opens its inner board, so the gestures people know keep working.
- **A person may release a pin on a mirrored board:** whoever may move a box there may hand it back
  to the AI's layout.
- **The tail points to one of four sides by the dominant axis between the centres, free notes at the
  bottom:** four sides drawn in CSS cover every layout without measuring the note's shape, and the
  rule needs only the rects the canvas already has.

## Non-Goals

- Highlighting what the AI just changed, references, stubs, and any other canvas idea in
  `docs/Ideas.md`.
- Notes on boxes on other boards, and attaching a note to another note.
