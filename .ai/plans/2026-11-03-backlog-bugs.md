**Repository:** root
**Status:** InProgress
**Created:** 2026-11-03

## Goal

Ten defects collected in `.ai/backlog.md` under "Bugs" make the canvas and the free space behave
differently from what they promise: clicks land one level too deep or are swallowed, typed text
can vanish, notes linger or get a scroll bar, marks overlap, and `place` can return a box that
does not fit in `i32`. Fix all ten, together with three small entries of the backlog that touch
the same code, so that the canvas and the core keep their stated behaviour before the MVP work.

## Context

- The core part is `crates/abyme-core/src/layout/free_space.rs`; everything else is the canvas UI in
  `crates/abyme-canvas/ui/src/` (`App.tsx`, `BoardCanvas.tsx`, `BoardPane.tsx`, `ElementBox.tsx`,
  `NoteContent.tsx`, `nodes.ts`, `boardNodes.ts`, `boards.ts`, `flags.ts`, `canvas.css`) and
  `docs/5 Architecture/Canvas.md`.
- jsdom has no layout and does not run WebKitGTK, so what a task can only show in a real window
  goes on a webview checklist: arrow ends after a measure, the cut marks, the note field's height,
  real double clicks, and `visibilitychange` on a workspace switch. Task 13 delivers the checklist
  with every item the earlier tasks name.
- **A press** is a run of clicks that the page counts up in their `detail`: the second and later
  clicks of a double click belong to the press of the first, while a single click, a click with
  `detail` 0 as from the keyboard, and a click after a pause start a new press.
- Each fixed backlog entry leaves `.ai/backlog.md` with the task that fixes it; the Cleanup entry
  on `Cell` and `raise_bottom` leaves with Task 3.
- Risk categories: no input path changes; the tasks change what the canvas offers and sends, which
  `src/admission.rs` and the core still decide.

## Steps

- [x] A box placed below fits on its right edge too
- [x] `Cell` derives nothing it does not use
- [x] The free space raises its bottom inline
- [x] Arrow ends stay where they are after a measure
- [x] A box whose file is missing offers neither a release nor a drag
- [x] The marks on a box's top edge share it
- [x] A window that turns visible asks for flags
- [x] A note's field grows with wrapped text
- [x] A held-back click ends with its press
- [x] One press opens one board
- [ ] A new note that is deleted before a view lists it goes
- [ ] The canvas knows which note's field is open
- [ ] A note deleted while it is typed keeps its field

## Tasks

### Task 1: A box placed below fits on its right edge too

`place` promises `None` when no position fits in `i32`, but a box placed below everything is
checked only on its bottom edge.

Review path: Security-Hybrid

- [x] A box of any width whose right edge would pass `i32::MAX` below the content gets no
      position, and a Rust test in `free_space.rs` shows it.
- [x] `cargo test --workspace`, `cargo clippy --workspace --all-targets`, and
      `cargo fmt --all --check` pass.

### Task 2: `Cell` derives nothing it does not use

`Cell` derives `Clone` and `Copy`, which nothing uses. Behavior is unchanged.

Review path: Reviewer-only

- [x] `Cell` derives only what the code uses.
- [x] The tests of `free_space.rs` and the property tests of `apply` pass with their assertions
      unchanged.
- [x] `cargo test --workspace`, `cargo clippy --workspace --all-targets`, and
      `cargo fmt --all --check` pass.

### Task 3: The free space raises its bottom inline

`raise_bottom` has one caller, `occupy`, and its body stands there instead. Behavior is unchanged.

Review path: Reviewer-only

- [x] No function `raise_bottom` remains; `occupy` raises the lowest bottom itself.
- [x] The tests of `free_space.rs` and the property tests of `apply` pass with their assertions
      unchanged.
- [x] `cargo test --workspace`, `cargo clippy --workspace --all-targets`, and
      `cargo fmt --all --check` pass.

### Task 4: Arrow ends stay where they are after a measure

The handles a node declares are 1×1, the handles in the page are larger, so an arrow's end shifts
by a few pixels once React Flow measures the box. Declared and drawn handles get the same size, set
by the canvas's own style, not by copying React Flow's.

Review path: Reviewer-only

- [x] Handles have no size, declared and drawn, so an arrow leaves at the exact middle of its
      source's bottom edge and arrives at the exact middle of its target's top edge; the existing
      arrow test in `BoardCanvas.test.tsx` asserts `M90,90` and `90,300` for its two boxes.
- [x] The declared handle geometry and the canvas's handle style are marked as changing together.
- [x] The webview checklist names an arrow between two boxes, looked at after the boxes are
      measured.
- [x] `pnpm --dir crates/abyme-canvas/ui check` and `pnpm --dir crates/abyme-canvas/ui test` pass.

### Task 5: A box whose file is missing offers neither a release nor a drag

The core refuses both a move and a release of an element whose file is missing, yet such a box
shows a release button and can be dragged.

Review path: Security-Hybrid

- [x] A pinned box whose element file is missing shows its pin marker as plain text, as on a
      mirrored board, and Vitest shows it.
- [x] A box whose element file is missing cannot be dragged or moved with the arrow keys on any
      board, and Vitest shows it.
- [x] The tests of dragging and the arrow keys on free and mirrored boards keep their coverage, on
      boxes whose file exists.
- [x] A reader of Canvas can tell that such a box offers no release and no move.
- [x] `pnpm --dir crates/abyme-canvas/ui check` and `pnpm --dir crates/abyme-canvas/ui test` pass.

### Task 6: The marks on a box's top edge share it

On a box narrower than about 110 px the review mark paints over the pin marker. Each mark takes at
most half of the top edge and is cut with "…" when it does not fit.

Review path: Security-Hybrid

- [x] The review mark and the pin marker each take at most half of the box's top edge and end in
      "…" where their text is cut, following the style of the `▸` buttons; Vitest shows that both
      carry the class that cuts them.
- [x] The webview checklist names a box of 100 px width with both marks.
- [x] `pnpm --dir crates/abyme-canvas/ui check` and `pnpm --dir crates/abyme-canvas/ui test` pass.

### Task 7: A window that turns visible asks for flags

A window that turns visible without getting focus, as on a workspace switch, asks for review flags
only at its next tick.

Review path: Security-Hybrid

- [x] A window that turns visible asks for review flags at once, also without focus, and Vitest
      shows it; turning visible and getting focus together asks again only after the run in
      progress ends, never beside it.
- [x] A reader of Canvas, and of the canvas row of the root `CLAUDE.md`, can tell when the canvas
      asks for review flags.
- [x] The webview checklist names a workspace switch back to the canvas's window.
- [x] `pnpm --dir crates/abyme-canvas/ui check` and `pnpm --dir crates/abyme-canvas/ui test` pass.

### Task 8: A note's field grows with wrapped text

The field counts only line breaks for its height, so a long wrapped line shows three rows and a
scroll bar. It takes the height its text needs, at least three rows.

Review path: Security-Hybrid

- [x] A note's text field is as high as its text, wrapped lines included, and never lower than
      three rows; Vitest shows that its height follows the text's scroll height when it opens and
      on every input.
- [x] The webview checklist names a note with one long wrapped line.
- [x] `pnpm --dir crates/abyme-canvas/ui check` and `pnpm --dir crates/abyme-canvas/ui test` pass.

### Task 9: A held-back click ends with its press

After a failed write, the canvas holds back the click of the same press so that the board does not
change under the open field. When the field then ends without a write, the next click to another
board does nothing and says nothing.

Review path: Security-Hybrid

- [x] After a failed write, the clicks of the press that ended the field open no board, and the
      click of a later press opens its board, also when the field ended without a write; Vitest
      shows both with clicks that carry their `detail`.
- [x] The existing App tests on leaving a board while a note is written pass unchanged.
- [x] `pnpm --dir crates/abyme-canvas/ui check` and `pnpm --dir crates/abyme-canvas/ui test` pass.

### Task 10: One press opens one board

The first click of a double click opens a board, and the rest of the press lands on the new view:
on a `▸` button, a breadcrumb, a box with an inner board, or the empty pane, where it opens a new
note's field.

Review path: Security-Hybrid

- [x] One press opens at most one board, whether its second click or its double click lands on a
      `▸` button, a breadcrumb button, or a box with an inner board in the new view; Vitest shows
      each with clicks that carry their `detail`.
- [x] The rest of a press that opened a board starts no new note on the new view, and Vitest
      shows it.
- [x] `Enter` on a focused `▸` button still opens its board, also right after another opening.
- [x] Vitest shows the `▸` button and the double click on a mirrored board as well as a free one.
- [x] The existing tests of opening boards pass unchanged.
- [x] The webview checklist names a double click on a `▸` button and on a box with an inner
      board.
- [x] A reader of Canvas can tell that one press opens one board.
- [x] `pnpm --dir crates/abyme-canvas/ui check` and `pnpm --dir crates/abyme-canvas/ui test` pass.

### Task 11: A new note that is deleted before a view lists it goes

A written new note stays on the screen until a view lists it. When its file is deleted first, it
stays until the board is left. It goes once a view that was asked for after the write answered
leaves it out; a view asked for earlier may still lack it and keeps it.

Review path: Security-Hybrid

- [ ] A written new note that a view asked for after the write's answer does not list leaves the
      board, without a message, and Vitest shows it.
- [ ] A written new note that a view asked for before the write's answer does not list stays, and
      Vitest shows it.
- [ ] A reader of Canvas can tell when a new note whose file is deleted before a view lists it
      leaves the board.
- [ ] `pnpm --dir crates/abyme-canvas/ui check` and `pnpm --dir crates/abyme-canvas/ui test` pass.

### Task 12: The canvas knows which note's field is open

Whether a note's text field is open lives inside the note's component, where the step that merges
a new view into the boxes cannot see it. It moves into the boxes' state, so that Task 13 can keep
an open field. Behavior is unchanged.

Review path: Reviewer-only

- [ ] Whether a note's field is open is part of the board's box state, merged through a new view
      like the selection, and has one source.
- [ ] Every test of `BoardCanvas.test.tsx`, `nodes.test.ts`, and `App.test.tsx` that exists before
      the task passes with its assertions unchanged; only fixtures may gain the new field.
- [ ] `pnpm --dir crates/abyme-canvas/ui check` and `pnpm --dir crates/abyme-canvas/ui test` pass.

### Task 13: A note deleted while it is typed keeps its field

When the AI deletes a note whose text is being typed, the field and the text vanish without a
message. The field stays open with the text instead, until its input ends.

Review path: Security-Hybrid

- [ ] A note whose field is open stays on the board, at its last position on the board, with its
      field and text when a view leaves it out; it cannot be dragged or moved with the arrow keys
      while it stays. Vitest shows it.
- [ ] Ending that field's input with changed text shows the core's refusal and keeps the field
      and the text open, and Vitest shows it.
- [ ] Ending that field's input with unchanged or blank text writes nothing, and the note leaves
      the board at once, and Vitest shows it.
- [ ] A note whose field is closed still goes when a view leaves it out.
- [ ] A reader of Canvas can tell what happens to a note deleted while it is typed.
- [ ] The review before the last commit gives the user the webview checklist with every item the
      tasks name.
- [ ] `pnpm --dir crates/abyme-canvas/ui check` and `pnpm --dir crates/abyme-canvas/ui test` pass.

## Task Notes

- **Task 13:** webview checklist: an arrow between two boxes, looked at after the boxes are measured,
  ends at the exact middle of the source's bottom edge and of the target's top edge (Task 4).
- **Task 13:** webview checklist: a pinned box of 100 px width with a review mark shows the pin
  marker and the review mark side by side, cut with "…" and not overlapping; a click on the visible
  pin releases it, and a click on the review mark does nothing (Task 6).
- **Task 13:** webview checklist: after a switch to another workspace and back to the canvas's
  window without clicking it, the review marks show a flag raised meanwhile without waiting for the
  30 s tick (Task 7).
- **Task 13:** webview checklist: a note whose text has one long line that wraps shows the whole
  line in its open field, with no scroll bar and no empty row below it; the field grows while typing
  and shrinks again when the line is deleted (Task 8).
- **Task 13:** webview checklist: a double click on a `▸` button, and on a box with an inner
  board, opens one board, and the rest of the double click on the new board opens nothing and
  starts no note (Task 10).

## Decisions

- **A deleted note's open field stays with its text, and a write then fails:** no typed text is
  lost, and a deletion the AI made is not undone behind its back. Turning it into a new note, or
  letting it go with a message, were declined. Ending its input without a write lets the note go
  at once, since nothing typed is lost then; while it stays it offers no move, since the core
  refuses one.
- **The marks are cut, not stacked:** widths below 180 px come only from hand edits, and cutting
  follows the existing style of the `▸` buttons.
- **A box whose file is missing loses the drag too:** a box offers only what the admission and the
  core take.
- **One press guard in the app's navigation serves the held-back click and the double click:**
  both decide which press a click belongs to; a guard per button was declined.
- **`place` keeps its promise for every width:** the right-edge check costs one condition, although
  every box is 180 wide today.
- **A window that turns visible and gets focus at once may ask twice:** the second run waits for the
  first, so it costs one more Git run and no overlap; a debounce would add a timer for that case.
- **A new note that is deleted before a view lists it goes without a message,** as any deleted note
  does.
- **All but Tasks 2, 3, 4, and 12 are Security-Hybrid with new tests:** they change behaviour no
  existing test covers; what jsdom cannot show goes on the webview checklist.

## Non-Goals

- Consolidating the grown test files and a builder for test elements (separate plan after this
  one).
- The entries of `.ai/backlog.md` under Security, and those under Tests, Cleanup, and Documentation
  that no task here names.
