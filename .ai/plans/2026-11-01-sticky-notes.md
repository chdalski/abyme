**Repository:** root
**Status:** InProgress
**Created:** 2026-11-01

## Goal

A person working on the canvas has no quick way to comment on a board without the AI. Let them
create a sticky note where the mouse is, edit its text, move it, and delete it, on every board,
without the canvas being able to change the text of any other element. Attaching a note to a box
is the next plan.

## Context

- The concept: `docs/2 Concepts/Presentation.md` ("Sticky Notes"), `docs/5 Architecture/Canvas.md`
  ("Commands", "Projecting a Board", "Editing"), `docs/5 Architecture/Data Model.md`, and
  `docs/5 Architecture/First Slice.md` ("MVP").
- The canvas: `crates/abyme-canvas/src/admission.rs` (what it may write), `app.rs` (the command
  `apply`), `projection.rs` (what the UI receives), and `ui/src/` (`client.ts`, `moves.ts`,
  `BoardPane.tsx`, `BoardCanvas.tsx`, `nodes.ts`, `boardNodes.ts`, `ElementBox.tsx`).
- The core: `crates/abyme-core/src/apply.rs` (the operations), `project/store.rs` (`write_locked`),
  and `slug.rs`.
- Risk categories: untrusted input (the page's text reaches the repository, and a note's body
  newly reaches the page), a trust boundary (a page from the dev server's address can send every
  operation admitted, and the AI reads what it writes), an access decision (which elements the
  canvas may change or delete), and data persistence.

## Steps

- [x] Check and write under one lock
- [x] A note shows as a note
- [x] A note moves on every board
- [x] A failed reload keeps the board
- [x] Editing a note's text
- [x] Creating a note
- [ ] Deleting a note

## Tasks

### Task 1: Check and write under one lock

The canvas checks a request against the files and then the core reads them again to write. With
text and deletion, an element changed in that gap could be overwritten, so the check moves into the
write.

Review path: Security-Hybrid

- [x] The canvas decides a request from the same reading of the files that the write starts from,
      under the project's lock, and a refusal writes nothing to `abyme/`.
- [x] Every existing test of what `apply` refuses and writes passes unchanged, apart from the
      comments that described the gap, which state the new rule.
- [x] A Rust test in `abyme-core` shows that a refusing check writes nothing, and that its decision
      is based on the files as they lie under the lock.
- [x] A reader of Canvas's Commands section can tell that the check runs under the project's lock.
- [x] `cargo test --workspace`, `cargo clippy --workspace --all-targets`, and
      `cargo fmt --all --check` pass.

### Task 2: A note shows as a note

An element of type `note`, however it was made, looks like a sticky note on the canvas and shows its
whole text. It is not a box with a type line.

Review path: Security-Hybrid

- [x] `open_board` returns an element of type `note` with its name and body; for every other type
      it still returns no body, as a test shows.
- [x] A note on the canvas shows its whole text as text in one fixed color, in the same node type as
      a box, with its review mark and without a pin marker. Its box on the screen grows with the
      text, and its rect in the board file does not change.
- [x] A name without ASCII letters or digits on an element of type `note` gives the ID `note`, then
      `note-2`, for the AI as for the canvas; other types keep `element`.
- [x] Rust tests go through IPC and through `apply`; Vitest covers the note, markup in its text, and
      a note whose file has only a name.
- [x] A reader of Canvas and Presentation can tell how a note is recognized and shown, and that its
      color is fixed; Data Model's rule on slugs states the fallback. The Glossary's entry on a sticky
      note says it is an element of type `note`. Neither Canvas nor a comment in the canvas crate says
      the projection never carries a body: they say it carries only a note's.
- [x] `cargo test --workspace`, `cargo clippy --workspace --all-targets`,
      `cargo fmt --all --check`, `pnpm --dir crates/abyme-canvas/ui check`, and
      `pnpm --dir crates/abyme-canvas/ui test` pass.

### Task 3: A note moves on every board

Notes are allowed in every mode, so a person may move a note on a mirrored board, and nothing else
there.

Review path: Security-Hybrid

- [x] On a mirrored board, the canvas lets a person drag a note, and the admission writes a
      `move_element` with `pinned: true` of an element whose file says it is a note. An element of
      another type, an element whose file is missing, and a release of a pin stay refused there, by
      position and board.
- [x] Rust tests cover each of these states through `apply` over IPC with the bytes of the board file;
      Vitest covers which boxes can be dragged on a mirrored board.
- [x] A reader of Canvas and of Boards and Zones can tell what a person may move on a mirrored board.
      The canvas row of the root `CLAUDE.md` no longer says `apply` admits only `move_element` or
      refuses it on a mirrored board; it says that `admission.rs` decides.
- [x] `cargo test --workspace`, `cargo clippy --workspace --all-targets`,
      `cargo fmt --all --check`, `pnpm --dir crates/abyme-canvas/ui check`, and
      `pnpm --dir crates/abyme-canvas/ui test` pass.

### Task 4: A failed reload keeps the board

Today one unreadable file in the project replaces the open board with a message, which would destroy
text being typed. This changes what every board shows.

Review path: Security-Hybrid

- [x] When a reload of an open board fails, the board stays as last loaded with the message above it
      as text, and a later reload that succeeds removes the message. A first load that fails still
      shows the message alone.
- [x] Vitest covers both, and a move or a release during a failed reload shows its own failure as
      text.
- [x] A reader of Canvas can tell what the board shows when the project cannot be read.
- [x] `pnpm --dir crates/abyme-canvas/ui check` and `pnpm --dir crates/abyme-canvas/ui test` pass.

### Task 5: Editing a note's text

A person changes a note's text in place, on every board. The canvas may change only a note's name and
body. Updating an element drops its verification stamp, so an unchanged text must not be written.

Review path: Security-Hybrid

- [x] The admission takes `update_element` only with an existing element whose file says it is a
      note, with a `name` that is not blank and holds no line break, a `body` of only the characters
      a name may hold, and no other field.
      Any other update, of any element, is refused by position, naming no other text; a request
      with such an operation writes nothing.
- [x] A double-click on a note opens only its text field, holding the name and, when the body is not
      empty, a line break and the body; also on a note that owns a board, whose `▸` button stays and
      opens that board. `Esc` and a click next to it end the input and write the text, in order with
      the canvas's moves.
- [x] Text becomes a name and a body by one rule: blank lines at its start and whitespace at its end
      are dropped, the first line left, without whitespace at its ends, is the name, and the rest is
      the body. Text with nothing left is blank.
- [x] Text whose name and body equal those of the text the field opened with writes nothing and keeps
      a verification stamp, also with a line break added at its end. Blank text writes nothing, and the
      note returns to its old text.
- [x] While the text field is open, a reload does not change what is typed. When a write fails, the
      field stays open with the text, and the message shows as text. Switching to another window does
      not end the input.
- [x] Keys pressed in the text field move no box.
- [x] Rust tests cover each state through `apply` over IPC with the bytes of the element file, and
      Vitest covers the input, the double-click on a note that owns a board, the split and join of the
      text as a round trip, a reload during typing, and a failed write. `admitted.typecheck.ts` covers
      the wider type.
- [x] A reader of Canvas can tell how a note's text is edited, and `Canvas.md`'s Commands section and
      the comments in `admission.rs` and `client.ts` state the new rule.
- [x] `cargo test --workspace`, `cargo clippy --workspace --all-targets`,
      `cargo fmt --all --check`, `pnpm --dir crates/abyme-canvas/ui check`, and
      `pnpm --dir crates/abyme-canvas/ui test` pass.

### Task 6: Creating a note

A person creates a note with the key `N`, a double-click on an empty spot, or the toolbar, on every
board. Only here does the canvas create an element.

Review path: Security-Hybrid

- [x] The admission takes a request that creates an element only as exactly one
      `create_element` of type `note` with the statement kind `idea`, a name by the same rule as for
      an update of a note's text, a body, and no summary or anchor, then `place_element` and
      `move_element` with `pinned: true` of that key on one existing board, of any mode. Every other
      shape is refused by position, naming no other text; such a request writes nothing.
- [x] `N` and a double-click on an empty spot put the note at the mouse; the toolbar and `N` without a
      known mouse position put it in the middle of the view. Its text field is open at once, and the
      `N` does not land in it. `N` with a modifier, a repeat, or in a text field creates nothing.
- [x] The note is written when the input ends, once, at the position it appeared, with its text
      split by the rule of Task 5; blank text writes nothing, and the note disappears.
- [x] From the moment it appears until the board's next view lists it, the note stays visible, also
      through a reload that started before the write. `apply` returns the created element's ID for
      this. A failed write keeps the field open with the text.
- [x] A double-click on a box that is not a note still opens its first inner board and creates no
      note, and a double-click on an empty spot no longer zooms.
- [x] Rust tests cover the accepted request and every refused shape through `apply` over IPC with the
      bytes of the files; Vitest covers the three ways, the draft through a reload, and a failed write.
      `admitted.typecheck.ts` covers the request's type.
- [x] A reader of Canvas can tell how a note is created, and its Editing section describes what is
      built.
- [x] `cargo test --workspace`, `cargo clippy --workspace --all-targets`,
      `cargo fmt --all --check`, `pnpm --dir crates/abyme-canvas/ui check`, and
      `pnpm --dir crates/abyme-canvas/ui test` pass.

### Task 7: Deleting a note

A person deletes a selected note with `Delete` or `Backspace`, on every board, without a question. A
box stays undeletable.

Review path: Security-Hybrid

- [ ] The admission takes `delete_element` only of an existing element whose file says it is a note.
      An element of another type and an element whose file is missing are refused by position, naming
      no other text; a request with such an operation writes nothing, and a missing element's
      placements stay on every board.
- [ ] The key sends one request for the selected notes, in order with the canvas's moves; a selected
      box beside them stays and is not sent. The note stays on the screen until the board's next view
      leaves it out, and a refusal from the core shows as text.
- [ ] Keys pressed in a note's text field delete nothing.
- [ ] Rust tests cover each state through `apply` over IPC with the bytes of the files, including a
      note another element relates to; Vitest covers the key with notes and with boxes selected.
      `admitted.typecheck.ts` covers the type.
- [ ] A reader of Canvas can tell how a note is deleted and that boxes cannot be, and its table of
      editing and Board Graph's section on deleting agree with that: no question, notes only.
- [ ] No comment in `crates/abyme-canvas/src/` and `ui/src/` states a rule this plan changed: that a
      mirrored board offers no move, that the canvas writes only moves, or that it cannot delete.
- [ ] A reader of the canvas row of the root `CLAUDE.md` can tell what the canvas may write: moves,
      and creating, editing, and deleting notes, and that a mirrored board allows only what concerns
      a note.
- [ ] `cargo test --workspace`, `cargo clippy --workspace --all-targets`,
      `cargo fmt --all --check`, `pnpm --dir crates/abyme-canvas/ui check`, and
      `pnpm --dir crates/abyme-canvas/ui test` pass.

## Task Notes

- Task 5: on a long note that owns a board, the `▸` button at the rect's lower edge lies over part of
  the text; check it while working on the note's text field.
- Task 6: the rule for `create_element`'s `body` is the same as for an update's: only the characters
  a name may hold, as the security engineer requires.
- Task 6: the last sentence of the slug rule in `docs/5 Architecture/Data Model.md`, "A sticky note's
  slug will come from its first words, or be `note` if it has no text.", doubles the fallback and is in
  the future tense; state what is built.

## Decisions

- **Two plans, attaching second:** attaching changes the board file's format and touches layout and
  the core, so it gets its own review on top of finished code.
- **A note is an element of type `note`:** no format change, and the AI sees the type at once. A note
  therefore looks the same on every board; the field `presentation` on a placement stays later work.
- **A new note has its own statement kind `idea` and one fixed color, with no choice:** without its own
  kind, a note on a mirrored board would inherit `current`, and the AI's search would list it as the
  checked current state.
- **The check moves under the lock, as a first task:** with text and deletion, the gap between
  checking and writing could overwrite or delete what another writer just changed. The states that
  differ from today: a request refused by the check, or failing while the files are read, now creates
  `.abyme/` and its lock if they are missing, waits for another writer, and fails with the lock's
  error where the lock cannot be taken, also ahead of a broken file. All of them are accepted; nothing
  under `abyme/` changes.
- **`Esc` and a click next to the note both save, and blank text writes nothing:** typed text is never
  lost by a key, and deleting stays its own action. So there is no way to cancel except clearing the
  text, and emptying a note cannot delete it.
- **A note is written when the input ends:** the ID comes once from the name, so an earlier write would
  fix it from the first keystroke. A crash during typing loses the text, and an AI edit made during
  typing is overwritten.
- **Leaving a board waits for a note's write:** a click on the way to another board ends the input,
  so a failed write would take the text with the board. It goes only after the write worked; after a
  failure, the one click that can belong to that press stays and the next goes. A gate demand of the
  reviewer on keeping the text after a failed write.
- **A failed reload keeps the board:** text being typed survives a broken file. The board can be stale
  for that time, and writes fail with the core's message.
- **`apply` stays the one writing command, with the shapes in `admission.rs`:** typed commands were
  declined, since they would add commands, permissions, and a second write path; a position on
  `place_element` was declined, since it widens the AI's interface for a need the AI does not have.
- **The core knows the type `note` for one string:** its slug fallback, so the docs' rule holds.
- **One node type:** a separate node type would copy handles and marks and widen the node's type
  across the move logic; a later presentation can split it out.
- **The text comes with the view, not by a new command:** the box shows the whole text anyway, so a
  read command would add a permission and a race and still could not draw it.
- **A note grows by CSS only:** the file's rect stays as created, so a long note can overlap what lies
  below it, also after a layout, and arrows and `fitView` go by the rect. Fixing that needs a core
  change and a decision of its own.
- **Deleting has no question and no `×` button:** Git holds the history. A missing element's placements
  cannot be cleared from the canvas.
- **Only `Delete` and `Backspace` delete, and only notes:** a box stays undeletable, as today.
- **A body from the canvas holds only the characters a name may hold:** with NUL in it, Git treats
  the element file as binary, and `git diff`, the review of what the AI reads, shows nothing. A gate
  demand of the security engineer on editing a note's text. The core stays as it is, so MCP and hand
  edits can still write such a body.
- **All tasks are Security-Hybrid:** the canvas first writes text the AI reads and decides what it may
  change or delete, and the new behavior has no tests yet, which the risk rule would escalate.

## Non-Goals

- Attaching a note to a box (separate plan after this one).
- A color per statement kind, choosing the kind, and Markdown in a note.
- Writing a note's grown size to the board file, and resizing by hand.
- Undo, a `×` button, and a confirmation on deleting.
- A field `presentation` on placements, and showing a note differently per board.
- Mentions such as `@ai` and tasks on notes.
