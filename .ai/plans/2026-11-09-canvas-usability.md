**Repository:** root
**Status:** InProgress
**Created:** 2026-11-09

## Goal

The canvas lacks basic navigation: no visible zoom control, a fixed sidebar with a flat board
list, and no way to see which boxes a selected box is connected to. The canvas gets a zoom bar, a
resizable and collapsible sidebar whose board list shows the board hierarchy as a tree, and
highlighting of the selected boxes' relations; the larger ideas found while trying the canvas are
recorded in `docs/Ideas.md`.

## Context

- The canvas is `crates/abyme-canvas`: Rust side in `src/` (IPC commands in `app.rs`, the board
  list in `boards.rs`, a board's projection in `projection.rs`), UI in `ui/src/` (layout in
  `App.tsx`, the list in `BoardList.tsx`, the board in `BoardCanvas.tsx`, arrows in `edges.ts`,
  boxes in `ElementBox.tsx`). `docs/5 Architecture/Canvas.md` describes what the canvas does; each
  task brings it in line with the behavior it changes.
- **Board hierarchy:** a board lies below the home board of the element that owns it, the order
  `overview` and the canvas's breadcrumb already use (`abyme_core::BoardTree`); a board whose
  chain is broken, by a cycle or an owner without an element file or without a home, stands at the
  top level.

## Steps

- [x] The ideas from trying the canvas are recorded
- [x] A zoom bar shows and changes the zoom
- [ ] The relations of the selected boxes stand out
- [ ] One helper builds a board's list entry
- [ ] The board list shows the hierarchy
- [ ] The board tree opens along the open board
- [ ] The sidebar can be resized and collapsed

## Tasks

### Task 1: The ideas from trying the canvas are recorded

`docs/Ideas.md` gets the three ideas that are not part of this plan, so a later plan finds them.

Review path: Reviewer-only

- [x] A reader of `docs/Ideas.md` finds three ideas, each with what it would give the user: folders
      for boards that need no element, as in Obsidian; a graph view of the boards with their
      hierarchy and the relations between elements on different boards, summed up per pair of
      boards; and animated data flows on the arrows.
- [x] No other note changes.

### Task 2: A zoom bar shows and changes the zoom

At the bottom right of the board, a bar shows the zoom level and offers zooming in, zooming out,
100 %, and fitting the board into view; React Flow's attribution link moves to the bottom left.

Review path: Security-Hybrid

- [x] The bar shows the current zoom as a whole percentage and follows every change of the zoom,
      also by the trackpad or the wheel.
- [x] A button `−` zooms out, a button `+` zooms in, a click on the percentage sets 100 %, and a
      fourth button fits the whole board into view.
- [x] The keys `+`, `-`, and `0` zoom in, zoom out, and fit the board, with Shift where the
      keyboard needs it for the character; they do nothing while a text field has the focus or
      with Ctrl, Meta, or Alt held.
- [x] The zoom reaches from 10 % to 200 %, React Flow's default maximum, and fitting a board uses
      that range.
- [x] The attribution link shows at the bottom left, and the bar covers nothing else of the board's
      controls.
- [x] Tests through `BoardCanvas` show each button and key acting on the zoom and the percentage
      following it, including a fit against a pane of known size.
- [x] `docs/5 Architecture/Canvas.md` tells a reader how to zoom, and its Scope section and its
      Editing table agree with that.

### Task 3: The relations of the selected boxes stand out

With one or more boxes or notes selected, the arrows that leave and enter them, the boxes at their
other ends, and the selection stand out, and the rest of the board is dimmed.

Review path: Security-Hybrid

- [ ] An arrow drawn from a selected element shows in one color and thicker, an arrow drawn to a
      selected element in a second color and thicker, its arrowhead in the same color; an arrow
      between two selected elements counts as leaving.
- [ ] A box at the other end of a highlighted arrow gets a border in that arrow's color, the
      leaving color when it is at both kinds of ends.
- [ ] Every other arrow and box is dimmed, except the box a selected note hangs on.
- [ ] With nothing selected, the board looks as it does today; a click on an empty spot ends the
      highlighting.
- [ ] Moving, pinning, creating, editing, and deleting notes and boxes behave as before, with the
      existing tests passing unchanged.
- [ ] Tests through `BoardCanvas` show the colors, borders, and dimming for a single selection, a
      multiple selection, a selected note, and an empty selection.
- [ ] `docs/5 Architecture/Canvas.md` tells a reader what a selection highlights, and its Scope
      section agrees with that.

### Task 4: One helper builds a board's list entry

The code that turns a board into the ID and title the UI receives exists in `boards.rs` and in
`projection.rs`; it moves into one function both use, ahead of the tree, which needs it too.
Behavior is unchanged.

Review path: Reviewer-only

- [ ] One function builds the entry of a board for the list, the breadcrumb, and the inner-board
      buttons.
- [ ] The tests of the board list, the breadcrumb, the inner-board buttons, and the command
      `list_boards` pass with their assertions unchanged.

### Task 5: The board list shows the hierarchy

The command `list_boards` returns the boards in the order `overview` lists them, each with its
depth, and the sidebar indents each board by its depth, every branch open. The list's readers
are the UI's `boards.ts` and `BoardList.tsx` and the test client `fakeClient.ts`.

Review path: Security-Hybrid

- [ ] `list_boards` returns every board once, in the depth-first order of `overview`, each with its
      ID, title, and depth; a board out of the tree has depth 0, as `overview` places it.
- [ ] The entries of the breadcrumb and of the inner-board buttons keep the shape they have.
- [ ] The sidebar shows each board indented by its depth, below the board it lies under.
- [ ] Rust tests of `list_boards` cover a nested hierarchy and an owner without an element file;
      UI tests show the indentation.
- [ ] `ui/src/bindings.ts` matches the Rust types, `docs/5 Architecture/Canvas.md` and the
      `abyme-canvas` row of the root `CLAUDE.md` describe the list as a tree, and no comment still
      gives the list's old order.

### Task 6: The board tree opens along the open board

A board with boards below it gets a toggle; branches are closed except those leading to the open
board.

Review path: Security-Hybrid

- [ ] A board with boards below it shows a toggle that opens and closes its branch; a board
      without shows none.
- [ ] When the canvas starts, only top-level boards show; when a board opens, by the list, a `▸`
      button, or the breadcrumb, every branch on the way to it opens, also one the user closed.
- [ ] A branch the user opened or closed stays so until the canvas closes or opening a board
      reopens it, also when the list reloads after a change of the project's files.
- [ ] Tests through `App` cover the start, opening a board deep in the tree, the user's toggles,
      and a reload.
- [ ] `docs/5 Architecture/Canvas.md` describes the tree's branches.

### Task 7: The sidebar can be resized and collapsed

The sidebar gets a handle that changes its width and a button that collapses it; both stay as the
user left them across starts of the canvas. The webview keeps its storage across starts: on Linux,
Tauri gives it the app's local data directory as a persistent data directory unless one is
configured (`tauri` 2.12.5, `src/manager/webview.rs`, and `wry` 0.57.1,
`src/webkitgtk/web_context.rs`), and the canvas configures none.

Review path: Security-Hybrid

- [ ] Dragging the handle on the sidebar's edge changes its width between 10rem and 40rem; a
      double-click on the handle sets the default width, 16rem, the sidebar's width today.
- [ ] A button collapses the sidebar so the board takes the whole width, and expands it again.
- [ ] Width and collapsed state are kept in the webview's storage and restored at the next start. A
      stored width that is a finite number is clamped to 10rem–40rem, and any other stored width,
      or none, gives 16rem; the sidebar starts collapsed only when the stored state is exactly the
      value the canvas writes for collapsed.
- [ ] A title too long for the sidebar is cut with an ellipsis and shown in full as a tooltip,
      still as text only.
- [ ] Tests through `App` cover resizing, the reset, collapsing, restoring, and invalid stored
      values, and no test depends on what an earlier test stored.
- [ ] `docs/5 Architecture/Canvas.md` describes the sidebar.

## Decisions

- **Board tree computed in Rust, sent as a flat list in depth-first order with each board's
  depth:** `abyme_core::BoardTree::depth_first` already orders the boards for `overview`, so the
  canvas list and `overview` agree by construction; the UI derives branches from the depths.
- **Zoom bar, sidebar, and highlighting each in a module of their own; highlighting derived from
  the selection and never stored in the boxes:** the node state in `nodes.ts` and `boardNodes.ts`
  (drafts, held notes, reload merging) stays untouched.
- **The helper that turns a board into a list entry moves to one place before the tree:** it exists
  twice today, in `projection.rs` and `boards.rs`, and the tree would need it a third time.
- **No core helper for the rule that an owner without an element file cuts the tree:** the rule is a
  one-line closure at each reader, cheaper than a new core API.
- **Zoom bar at the bottom right, React Flow's attribution link moved to the bottom left:** the
  link stays visible, as the library's authors ask.
- **Minimum zoom 10 %:** fitting a board into view then works for boards up to ten times the
  window, and trackpad zoom reaches as far.
- **Highlighting:** an arrow between two selected elements counts as leaving, a box at both kinds
  of ends gets the leaving border, and the box a selected note hangs on is not dimmed, since the
  note hangs on it visibly although no arrow is drawn.
- **Board tree branches:** closed except those on the path to the open board; opening a board
  reveals it in the list, also through a branch the user closed; the user's toggles last for the
  session only.
- **The board tree in two tasks:** the list's new shape and its fixtures apart from the branches'
  behavior, so each diff can be judged on its own.
- **Zoom keys and a click on the percentage for 100 %:** they come with the zoom bar as the usual
  companions of its buttons, and they act only where the buttons act.
- **Sidebar widths 10rem to 40rem, default 16rem:** the default keeps today's layout, the bounds keep
  titles readable and leave the board most of the window; a stored width out of range is clamped,
  not dropped, so a width stored under other bounds still comes close.

## Non-Goals

- Moving boxes on mirrored boards and a note mode with speech bubbles that attach where they are
  created: they widen what the canvas writes (separate plan after this one).
- A minimap, and folders for boards that need no element: the first was not asked for, the second
  is an idea for `docs/Ideas.md`.
