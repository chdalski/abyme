**Repository:** root
**Status:** InProgress
**Created:** 2026-10-23

## Goal

The Working Skeleton's canvas: a desktop app shows the boards of the
project it starts in, keeps them live, and lets the user drag an
element. An element that the AI writes with `apply` over MCP appears in
the open board without a reload, and a drag lands in the board file,
where `read_board` over MCP shows the new position. This shows that a
text access and a visual access work on the same files before MVP
features depend on it.

## Context

- The decided concept is in `docs/5 Architecture/Canvas.md` (Tauri app
  `crates/abyme-canvas`, UI in `crates/abyme-canvas/ui/`, `AbymeClient`
  over Tauri IPC, ts-rs types, projecting a board, editing, security),
  `docs/5 Architecture/First Slice.md#Working Skeleton`,
  `docs/5 Architecture/Hexagonal Architecture.md` (crates, core API,
  runtime stage A, Security Baseline), and
  `docs/5 Architecture/Storage in Git.md#Writing` (the canvas reloads on
  file-watcher events, its own writes included).
- The canvas runs inside the devcontainer (Ubuntu 24.04), which forwards
  the display through VS Code.
- Libraries, each pinned exactly by the task that adds it:
  - Rust: `tauri` 2.12.5 with `tauri-build` 2.7.4, `ts-rs` 12.0.1,
    `notify` 8.2.0;
  - npm through pnpm, lockfile committed: `@tauri-apps/api` 2.12.4,
    `@xyflow/react` 12.12.0, `react` and `react-dom` 19.3.1; for
    development `vite` 8.3.5, `@vitejs/plugin-react` 6.1.2,
    `typescript` 6.0.3, `vitest` 5.0.5, ESLint with typescript-eslint,
    and Prettier.
- `Project::init` creates `abyme/` with its project file only;
  `abyme/elements/` and `abyme/boards/` appear with the first write.
- Risk categories:
  - a new IPC boundary between a webview that renders repository text
    and the Rust side that writes the project;
  - a new accepted input form, `move_element`, through MCP `apply` and
    the canvas;
  - file-system events from a tree that a repository controls;
  - the Tauri capability set as an access decision;
  - code execution through the webview, WebKitGTK, and a new npm
    dependency tree;
  - system packages and a build tool added to the devcontainer.

## Steps

- [x] Extract loading a board into the request's draft
- [x] Move an element with an operation
- [x] Show coordinates in `read_board` on request
- [x] Provide the canvas toolchain in the devcontainer
- [x] List the project's boards in the canvas, live
- [x] Show a board's elements in the canvas, live
- [x] Move an element by dragging it

## Tasks

### Task 1: Extract loading a board into the request's draft

`move_element` will load an existing board into the request's draft the
way `place_element` does. Before that, the loading becomes one function
that both can use. Behavior is unchanged.

Review path: Security-Hybrid

- [x] `place_element` loads an existing board into the draft through a
      function that a second operation can call; no copy of that logic
      remains in `place_element`.
- [x] These tests pass with their assertions unchanged:
      `crates/abyme-core/src/apply/tests.rs` and
      `crates/abyme-core/tests/apply_properties.rs`.

### Task 2: Move an element with an operation

The core gets the operation `move_element`, which sets the position of
an element on a board where it lies. The canvas and the AI move
elements through it, so MCP `apply` accepts it as well.

Review path: Security-Hybrid

- [x] `{"op": "move_element", "element": …, "board": …, "x": …, "y": …}`
      changes only x and y of that element's placement on that board.
      Width, height, other placements, other boards, and element files
      stay unchanged. Tests show it.
- [x] A move applies to the board as the request has changed it so far:
      a request that creates an element, places it, and moves it writes
      the moved position, and of two moves of one element in a request,
      the later one holds. Tests show both.
- [x] A move is rejected, and the request writes nothing, when the
      element does not lie on that board, when the element or the board
      is unknown, or when a coordinate is not an integer from -1,000,000
      to 1,000,000 inclusive. Tests show each case through `apply`, with
      both bounds accepted and one value beyond each bound rejected, and
      one case through MCP `apply`.
- [x] In a request that moves an element and then places another
      element on the same board, the placed element does not overlap the
      moved one; a test shows it.
- [x] The message of a rejected move names only IDs and numbers.
- [x] The test of `apply`'s input schema covers `move_element`. From the
      description of the MCP tool `apply`, `crates/abyme-mcp/README.md`,
      and `docs/5 Architecture/Data Model.md#Operations`, a reader learns
      that `move_element` exists and what it takes; no sentence there
      still counts three operations.

### Task 3: Show coordinates in `read_board` on request

`read_board` takes an optional argument that adds each element's
position and size to its line, so the AI can see where an element lies,
for example after a drag in the canvas.

Review path: Security-Hybrid

- [x] With coordinates requested, each element line shows the element's
      x, y, width, and height, in one format that the tests assert.
      Without the argument, the outline stays byte-for-byte as it is: the
      existing tests in `crates/abyme-core/src/outline/tests.rs` and
      `crates/abyme-core/tests/outline.rs`, and the existing `read_board`
      and `search` calls in `crates/abyme-mcp/tests/server.rs` and
      `crates/abyme-mcp/src/server/tests.rs`, pass with their assertions
      unchanged.
- [x] Through MCP, a `move_element` followed by `read_board` with
      coordinates shows the new position; a test shows it.
- [x] The tool's input schema lists the new argument as an optional
      boolean, and arguments that do not fit the schema come back as tool
      errors.
- [x] From `docs/4 AI Interface/MCP Server.md` and
      `crates/abyme-mcp/README.md`, a reader learns how to ask
      `read_board` for coordinates.

### Task 4: Provide the canvas toolchain in the devcontainer

Tauri builds on Linux against WebKitGTK and GTK, and its CLI runs the app
during development. The devcontainer image does not install either yet.

Review path: Security-Hybrid

- [x] The Dockerfile installs Tauri's Linux prerequisites from Ubuntu's
      packages and the Tauri CLI at an exact version. The same install
      commands have run in the running container, where
      `pkg-config --modversion webkit2gtk-4.1` and `cargo tauri --version`
      succeed.
- [x] Each package name and the CLI version are checked against their
      source, and the handoff says how.
- [x] The root `CLAUDE.md` names the canvas toolchain among the
      toolchains the devcontainer provides.

### Task 5: List the project's boards in the canvas, live

The new crate `abyme-canvas` is the Tauri app. Started in a project, it
lists the project's boards, and a board that `apply` creates appears in
the list without a reload. The UI's TypeScript types are generated from
the Rust types with ts-rs.

Review path: Security-Hybrid

- [x] The app started in a project shows the titles of the project's
      boards, or a note that the project has no boards yet. Started
      outside a project, it shows the same hint as MCP: run
      `abyme-mcp init`. When the core cannot read the project, it shows
      the core's message as MCP does. Tests of the command and of the
      list view show each case.
- [x] A board created by `Project::apply` appears in the list without a
      reload, also in a project that `abyme-mcp init` just created and
      that has no `abyme/boards/` yet: Rust tests show that the write
      raises the change signal, and a Vitest test shows that the signal
      leads to the new list.
- [x] A symbolic link inside a watched directory is not followed, and a
      write under `.abyme/` raises no change signal; Rust tests show
      both.
- [x] The webview reaches the Rust side only through Abyme's own
      commands and listening to its events; the app includes no Tauri
      plugin; the UI renders repository text as text only. Tests show
      each.
- [x] The TypeScript types that describe Rust data are generated from
      the Rust types, and a test fails when the committed generated file
      differs from what the Rust types produce.
- [x] On a checkout without a UI build, `cargo build --workspace`,
      `cargo clippy --workspace --all-targets`, and
      `cargo test --workspace` pass, and none of them runs pnpm or npm.
- [x] Every npm and crate dependency the task adds is pinned exactly,
      the pnpm lockfile is committed, and the handoff lists the licenses
      of the new dependencies, all MIT-compatible.
- [x] The root `CLAUDE.md` lists `crates/abyme-canvas` among the
      components, gives the commands to install, check, test, and run the
      UI, and tells which runs need `--all-features` for the new `ts`
      feature, as it does for `json-schema`.
- [x] From `docs/5 Architecture/Canvas.md`, a reader learns the crate's
      layout and how the UI learns about changes; from
      `docs/5 Architecture/Data Model.md`, which functions the core
      provides and that the canvas itself watches for changes.

### Task 6: Show a board's elements in the canvas, live

A click on a board in the list opens it. Each element placed on the
board appears as a box at its position and size from the board file,
with its type, name, and summary. An element that `apply` places on the
open board appears without a reload.

Review path: Security-Hybrid

- [x] An opened board shows one box per placement, at the rect from the
      board file, with the element's type, name, and summary; a
      placement whose element file is missing shows as missing. Rust
      tests show the board's projection, and Vitest tests show its
      mapping to React Flow nodes.
- [x] An element that `Project::apply` places on the open board appears
      without a reload, also when it is the first element of the project
      and `abyme/elements/` did not exist before: Rust tests show that
      the write raises the change signal, and a Vitest test shows that
      the signal re-fetches the open board.
- [x] When the open board no longer exists or the core cannot read the
      project, the board view shows the core's message in its place, and
      the list stays reachable; a Vitest test shows it.
- [x] The projection sent to the UI holds no element body.
- [x] Every npm and crate dependency the task adds is pinned exactly and
      MIT-compatible, and the handoff lists their licenses.
- [x] From `docs/5 Architecture/Canvas.md#Projecting a Board`, a reader
      learns that the canvas computes a board's projection from the
      core's snapshot.

### Task 7: Move an element by dragging it

Releasing a dragged element writes its new position into the board file
through `move_element`. The UI sends operations through one IPC command,
and the Rust side admits only the operations the canvas may send: today
`move_element` of an existing element on an existing board.

Review path: Security-Hybrid

- [x] Releasing a dragged element sends one request with one
      `move_element` at integer coordinates, and the board file holds the
      new position afterwards. A Vitest test shows the translation, and a
      Rust test shows the command from the request to the board file.
- [x] When a move fails, the box returns to the position in the board
      file, and the UI shows the error; a Vitest test shows it.
- [x] Every operation that the canvas may not send is refused before any
      access to the project, and a request with any refused operation
      writes nothing. Tests cover each kind of operation and a request
      that mixes admitted and refused ones.
- [x] A refusal names the operation's position and kind and nothing of
      its content.
- [x] The UI's typed client accepts only the operations that the Rust
      side admits.
- [x] A reload that arrives during a drag does not move the dragged
      element back; a Vitest test shows it.
- [x] Every npm and crate dependency the task adds is pinned exactly and
      MIT-compatible, and the handoff lists their licenses.
- [x] From `docs/5 Architecture/Canvas.md`, a reader learns which IPC
      commands the canvas has, which writes the Rust side admits and how
      that set grows, and what the webview can reach.

## Decisions

- **`move_element` is a core operation,** not a write of the canvas
  alone: edits go back to the core as operations. It takes `Ref`s like
  `place_element` and sets x and y only. Resizing and placing stay
  separate operations, so an element that does not lie on the board is
  an error, never a new placement. A placement whose element file is
  missing cannot be moved.
- **Coordinates of a move are bounded,** checked in the core for every
  caller: a position near the `i32` limit would make later placements on
  that board fail. Board files with larger coordinates are still read,
  as today.
- **`read_board` shows coordinates only on request,** as
  `MCP Server.md#Detail Levels` describes; `search` lines stay as they
  are.
- **One IPC command writes, with a list of admitted operations:**
  `apply(Operations)` in MCP's format. A pure function checks every
  operation with an exhaustive match before any access to the project,
  so a new core operation breaks the canvas build until it is admitted
  or refused. Each admission is its own reviewed change. Rejected:
  - one command per edit, because the IPC surface grows with every
    edit;
  - `apply` without a list, because a compromised webview could write
    any text into the repository;
  - a type of canvas edits, because it repeats every admitted operation
    and adds a second reader of untrusted input.
- **The projection lives in `abyme-canvas`,** computed from the snapshot
  by pure functions.
- **The watcher signals, and the UI re-fetches.** It watches `abyme/`,
  `abyme/elements/`, and `abyme/boards/`, never `.abyme/`, does not
  follow symbolic links, and debounces bursts of writes into one signal.
  Its event carries no payload. The canvas's own writes are not
  suppressed, since the reload confirms them. It lives in `abyme-canvas`
  and takes a closure, without a trait until a second implementation
  exists. Rejected: a recursive watch of `abyme/`, because a symbolic
  link in a cloned repository could make it watch the whole file system.
- **The watcher debounces itself,** without `notify-debouncer-mini`:
  inotify reports opening a file, and that debouncer passes every event
  on, so the reads that answer a signal would raise the next one. The
  watcher drops read events and signals once changes have been quiet for
  100 ms.
- **ts-rs is behind a feature `ts`,** like `json-schema`, that only the
  tests of `abyme-canvas` enable: the binary needs no TypeScript. So far
  only `abyme-model` has it, since only its IDs cross IPC; `abyme-core`
  gets it when one of its types does. The generated file is committed,
  and a test compares it, so a stale file fails instead of being
  rewritten.
- **The canvas is a workspace member,** and workspace builds compile it
  without a UI build. If Tauri needs the frontend at compile time, a
  committed placeholder serves it.
- **TypeScript stays below 6.1,** since typescript-eslint supports
  TypeScript only below 6.1.
- **The Tauri CLI comes from cargo,** installed in the image, not as an
  npm package: fewer native npm binaries.
- **The user rebuilds the image at their own pace,** outside any task
  gate; the review compares the Dockerfile with the commands that ran in
  the container.
- **The page freezes `Object.prototype` itself,** in `ui/src/main.tsx`
  once its modules have loaded, not through Tauri's `freezePrototype`:
  d3-color, which React Flow loads, sets `constructor` on its own
  prototypes while it loads, and a prototype frozen before that leaves
  the window blank. Repository data arrives only after the first render.
  Rejected (the user's choice):
  - no freeze, because it drops a defense against prototype pollution;
  - a patch of d3-color, because it must be maintained and the next
    library with the same pattern breaks the window again.
- **Boxes have the sizes in the board file;** text measurement comes
  with the MVP.
- **No end-to-end tests in a webview:** Rust tests for the commands and
  the watcher, Vitest tests for the UI logic with a fake `AbymeClient`,
  and the user's own trial of the running app.

## Non-Goals

- Text measurement and layout in Rust (MVP)
- Editing in the canvas beyond moving: resizing, renaming, creating,
  connecting, deleting, undo (later plans; each admits its operations as
  a reviewed change)
- Relations and edges, inner boards and the breadcrumb, sticky notes,
  review flags in the canvas (MVP)
- Bundles and installers for the canvas
