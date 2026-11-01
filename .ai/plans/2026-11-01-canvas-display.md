**Repository:** root
**Status:** InProgress
**Created:** 2026-11-01

## Goal

The canvas shows boxes on one board at a time, without what connects them. Let a
person see the relations as arrows, move between a board and the inner boards of its
elements with a breadcrumb, see review flags and pins, and release a pin.

## Context

- The concept: `docs/5 Architecture/Canvas.md`, `docs/5 Architecture/First Slice.md`
  ("MVP"), `docs/2 Concepts/Relations.md`, and `docs/4 AI Interface/Placement and Layout.md`.
- The canvas: `crates/abyme-canvas/src/projection.rs` (what the UI receives), `app.rs` (the
  commands), `admission.rs` (what it may write), and `ui/src/` (`App.tsx`, `BoardPane.tsx`,
  `BoardCanvas.tsx`, `nodes.ts`, `ElementBox.tsx`, `live.ts`, `client.ts`).
- The core: `crates/abyme-core/src/outline.rs` (which relations a board shows),
  `board_tree.rs` (the hierarchy), and `review.rs` with `project/git.rs` (review flags).
- Risk categories: untrusted input (relation labels and titles of other boards newly reach
  the page), a trust boundary (the canvas may write something more, and a page from the dev
  server's address can send it), code execution (the canvas starts Git in a project for
  the first time), and availability (Git runs in the background).

## Steps

- [x] One test fixture for what the UI receives
- [x] One rule for the relations a board shows
- [x] One way up the board tree
- [x] Relations as arrows
- [x] Inner boards and a breadcrumb
- [x] A marker on pinned boxes
- [x] Releasing a pin
- [x] Review flags on boxes
- [x] Review flags stay current

## Tasks

### Task 1: One test fixture for what the UI receives

Each later task adds fields to `BoardView` and `Placement`, and the test files in
`crates/abyme-canvas/ui/src/` hold their own literals. Give them one fixture. Behavior is
unchanged, and only tests change.

Review path: Reviewer-only

- [x] A fixture module gives a board view and a placement with overrides, and the tests that
      build them use it, except `main.test.tsx`, whose literal lies in a hoisted mock.
- [x] What the tests assert does not change.
- [x] `pnpm --dir crates/abyme-canvas/ui check` and `pnpm --dir crates/abyme-canvas/ui test` pass.

### Task 2: One rule for the relations a board shows

`read_board` decides which relations a board shows in `crates/abyme-core/src/outline.rs`, and
the canvas needs the same answer. Move the rule to one public function of the core. Behavior is
unchanged; `shows_only_relations_whose_ends_both_lie_on_the_board` and the other tests in
`crates/abyme-core/src/outline/tests.rs` pin it.

Review path: Reviewer-only

- [x] One public function of `abyme-core` answers which relations a board shows: the source has
      a file and both ends are placed. `read_board` uses it.
- [x] The named tests pass with no assertion changed.
- [x] `cargo test --workspace`, `cargo clippy --workspace --all-targets`, and
      `cargo fmt --all --check` pass.

### Task 3: One way up the board tree

The breadcrumb needs the boards above a board, which `depth_first` computes inside its walk for
`overview`. Give `BoardTree` one method for that step up, apart from `parent`, which also follows
a cycle, and let `depth_first` use it. Behavior is unchanged;
`crates/abyme-core/src/board_tree/tests.rs` and the tests of `overview` in
`crates/abyme-core/src/outline/tests.rs` pin it.

Review path: Reviewer-only

- [x] `BoardTree` answers the board above a board, and `None` for a board inside a cycle, for an
      owner without a file, and for an owner without a home; `depth_first` uses that answer.
- [x] The named tests pass with no assertion changed.
- [x] `cargo test --workspace`, `cargo clippy --workspace --all-targets`, and
      `cargo fmt --all --check` pass.

### Task 4: Relations as arrows

The canvas draws an arrow for each relation the board shows, with React Flow's built-in edges.
`ElementBox` has no handles and nodes carry no measurements across a reload, so edges draw nothing
today.

Review path: Security-Hybrid

- [x] `open_board` returns the board's relations with source, target, kind, and label, by the rule
      of Task 2, so an arrow to a box whose file is missing is drawn and a relation to an element
      on another board is not.
- [x] Each relation draws one arrow from the bottom of its source to the top of its target, and
      `see_also` is dashed. An arrow carries the relation's label as text, cut by the UI after
      40 characters and ending in `…`, and no other text. Arrows cannot be selected, deleted, or
      reconnected.
- [x] Arrows stay drawn through a reload after a drag, and follow a box while it is dragged.
- [x] `bindings.ts` matches the Rust types, and tests in Rust and Vitest cover the projection, the
      arrows, the dashed kind, the label as text, and a reload; an IPC test covers `open_board`.
- [x] A reader of Canvas can tell what an arrow shows and that arrows are not routed around boxes,
      and its Security section names relation labels among what is shown as text. The question on
      routing has left Canvas and `docs/Open Questions.md`, and `docs/Decisions.md` holds the
      decision.
- [x] `cargo test --workspace`, `cargo clippy --workspace --all-targets`,
      `cargo fmt --all --check`, `pnpm --dir crates/abyme-canvas/ui check`, and
      `pnpm --dir crates/abyme-canvas/ui test` pass.

### Task 5: Inner boards and a breadcrumb

A person opens an element's inner board from its box and goes back up with a breadcrumb.

Review path: Security-Hybrid

- [x] `open_board` returns the boards above the open board, top first, as `overview` places them,
      and for each placement the boards its element owns, each with ID and title.
- [x] A box shows a `▸` button per inner board, which keyboard and mouse can use and which does not
      start a drag; a double click on a box opens its first inner board, and a box without one
      behaves as before.
- [x] The open board shows a breadcrumb of the boards above it, and a click opens that board. Where
      the tree is broken, by a cycle or an owner without a file or home, the breadcrumb ends there
      and cannot loop, and the board stays open and reachable through the list of boards.
- [x] The open board stays the one piece of state that decides what is shown: the board list marks
      the same board, and a link to a board that has gone shows the existing message.
- [x] Tests in Rust and Vitest cover the fields, a broken tree, the button, the double click, the
      breadcrumb, and keyboard use; an IPC test covers `open_board` on inner boards.
- [x] A reader of Canvas can tell how to move between boards: its table row on zooming in and its
      Scope proposal no longer say otherwise, and its section on projecting a board and its Security
      section name what `open_board` now returns. The Glossary defines the breadcrumb.
- [x] `cargo test --workspace`, `cargo clippy --workspace --all-targets`,
      `cargo fmt --all --check`, `pnpm --dir crates/abyme-canvas/ui check`, and
      `pnpm --dir crates/abyme-canvas/ui test` pass.

### Task 6: A marker on pinned boxes

A person sees which boxes are pinned.

Review path: Security-Hybrid

- [x] `open_board` says for each placement whether it is pinned, and a pinned box shows a marker
      that hides none of its text, in a form that does not rely on color alone.
- [x] A move in the canvas shows the marker after the reload that follows it.
- [x] Tests in Rust and Vitest cover the field and the marker.
- [x] A reader of Canvas can tell what the marker means, and the entry "At a glance" in
      `docs/Open Questions.md` keeps only what is still open.
- [x] `cargo test --workspace`, `cargo clippy --workspace --all-targets`,
      `cargo fmt --all --check`, `pnpm --dir crates/abyme-canvas/ui check`, and
      `pnpm --dir crates/abyme-canvas/ui test` pass.

### Task 7: Releasing a pin

A person releases a pin in the canvas with one click, and the box stays where it is.

Review path: Security-Hybrid

- [x] On a board that is not mirrored, the marker of a pinned box is a button that releases the pin.
      On a mirrored board it stays a plain marker. A release is `move_element` with `pinned: false`
      at the box's position, sent in order with the moves of the canvas.
- [x] The canvas admits `move_element` with `pinned: false` only when its position is the one in the
      board file, and refuses it otherwise by position and board, naming no other text; a request
      with such an operation writes nothing. `move_element` without `pinned` stays refused, and a
      release on a mirrored board is refused like every move there.
- [x] Rust tests cover each of these states through `apply` over IPC with the bytes of the board file,
      and the tests that pinned the refusal of `pinned: false` change to the new rule; Vitest covers
      the button and its absence on a mirrored board, and the typecheck file `admitted.typecheck.ts`
      covers the wider type.
- [x] A reader of Canvas, Placement and Layout, and the Glossary's entry on a pin can tell how a person
      releases a pin, and the comments in `admission.rs`, `app.rs`, and `client.ts` that say what the
      canvas may send, and which check needs the project, state the new rule.
- [x] `cargo test --workspace`, `cargo clippy --workspace --all-targets`,
      `cargo fmt --all --check`, `pnpm --dir crates/abyme-canvas/ui check`, and
      `pnpm --dir crates/abyme-canvas/ui test` pass.

### Task 8: Review flags on boxes

A box shows whether its element has a review flag, and of which level, when the board opens and
whenever a file watcher sees the project change.

Review path: Security-Hybrid

- [x] A new command returns each flagged element with `task` or `hint`, computed through the core,
      apart from `open_board`, so a slow Git holds up neither boxes nor arrows nor a drag. Opening
      another board does not ask Git again.
- [x] A box of a flagged element shows its level, in a form that does not rely on color alone and
      that hides none of its text, and shows no reason.
- [x] When Git cannot be asked, the board shows one status line that says flags are unavailable and
      why, and marks no box, so a missing mark never reads as clean. While the first answer is
      pending, it says so.
- [x] The capability grants the new command and no other, as the capability test checks.
- [x] Tests cover the command over IPC in a real repository and outside one, and in Vitest the marks,
      the status line, and a slow answer; `abyme-test-support` becomes a dev-dependency of the canvas.
- [x] A reader of Canvas can tell what a mark means and what the line means; its Commands section
      counts and lists the command, and its Security section says the canvas starts Git as the core
      does. The Glossary defines the mark. The components rows of the root
      `CLAUDE.md` for the canvas, with arrows, inner boards, pins, and flags, and for
      `abyme-test-support` state the plan's result.
- [x] `cargo test --workspace`, `cargo clippy --workspace --all-targets`,
      `cargo fmt --all --check`, `cargo deny --locked check`, `pnpm --dir crates/abyme-canvas/ui check`,
      and `pnpm --dir crates/abyme-canvas/ui test` pass.

### Task 9: Review flags stay current

The file watcher sees neither commits nor changes under an anchor, which is what raises flags. Ask
again when the window gets focus and every 30 s while it is visible.

Review path: Security-Hybrid

- [x] Flags are computed again when the window gets focus and every 30 s while the document is
      visible, and not while it is hidden. A slow run never starts a second one beside it.
- [x] The watcher does not change.
- [x] Vitest with fake timers covers focus, the interval, a hidden document, and a run still pending.
- [x] A reader of Canvas and of Checks can tell how current the marks on the canvas are, at most
      one interval behind.
- [x] `pnpm --dir crates/abyme-canvas/ui check` and `pnpm --dir crates/abyme-canvas/ui test` pass.

## Decisions

- **Two plans, display first:** sticky notes let the canvas write text into the repository for the
  first time, so they get their own review.
- **Labels are cut in the UI:** the full label stays in what the UI receives, like every other text.
- **Fields on `BoardView` for arrows and the breadcrumb, not new commands:** they come from one
  snapshot, so they cannot disagree, and need no new permission. A navigation stack for the
  breadcrumb was declined: it would be empty after a click in the list and wrong after a change in
  the files, so an element placed on several boards shows its home in the trail.
- **Review flags have their own command:** inside `open_board`, a slow Git would delay each reload and
  make a box jump back after a second drag.
- **Flags refresh on focus and every 30 s:** focus alone leaves a canvas beside the editor stale, and
  watching `.git` misses changes that are not committed, worktrees, and submodules.
- **A box shows only the level of a flag:** reasons would bring file names to the page, and the AI
  or `read_element` gives them.
- **An arrow carries only its label:** showing the kind clutters large boards.
- **A release stays a `move_element` with `pinned: false` and must keep the position:** a new
  operation would change the core, the schema, and the docs for a rare window; without the check, a
  release from a stale view would silently move a box back.
- **No release on a mirrored board:** there only reconciliation changes a board, and the person can ask
  the AI.
- **A breadcrumb in a broken tree simply ends:** the board stays reachable through the list, and the AI
  sees the warnings. A warning in the breadcrumb was declined.
- **New work gets Security-Hybrid:** the release is an access decision and flags start the canvas's
  first Git processes, both of which the risk rule would escalate; the user chose this path.

- **Arrows are checked in a simulated layout:** no display exists here to run WebKitGTK, so the tests
  simulate measuring and a drag in jsdom; a run in the real webview is for the user at the review.

## Non-Goals

- Sticky notes (separate plan after this one).
- Routing arrows around boxes, until dogfooding shows a need, and joining parallel relations into
  one arrow.
- Showing why a box is flagged, and a statement kind on the canvas.
- Showing relations to other boards, a tree-shaped board list, and changing what the watcher sees.
