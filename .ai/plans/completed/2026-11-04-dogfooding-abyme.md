**Repository:** root
**Status:** Completed (2026-11-04)
**Created:** 2026-11-04

## Goal

The MVP is tested first on Abyme's own repository: Abyme is developed with Abyme, so its boards
follow every real change and gaps show up in daily work. Make this repository an Abyme project
whose MCP server Claude Code starts, have the AI build Abyme's C4 model through that server and
commit it with the code, and set the rule by which later code changes keep the model current.

## Context

- The model is built only through the MCP tools `apply` and `verify`, never by editing files under
  `abyme/` by hand; each board task's handoff carries the `overview`, `read_board`, and `status`
  outputs of the state it commits. The board tasks change nothing under an anchor, so each
  verifies its elements before its own commit.
- The tools of the server reach the AI only in a session that starts after Task 1's commit and
  after the user has approved the server, so the lead stops after Task 1 until the user has
  started that session.
- The C4 model, every board a zone `mirrored · current` with a summary:

  | Board | Owner | Elements: type (anchor) |
  |---|---|---|
  | `context` | none | `abyme`: `c4/system` (`Cargo.toml`); `developer`: `c4/person`; `ai-agent`, `project-repository`, `hugging-face-hub`: `c4/system` |
  | `containers` | `abyme` | `abyme-model`, `abyme-core`, `abyme-index`, `abyme-mcp`, `abyme-canvas`: `c4/container` (each `crates/<id>`); `project-files`, `search-cache`: `c4/datastore` |
  | `core-components` | `abyme-core` | `c4/component`, anchors under `crates/abyme-core/`: `operations` (`src/operation.rs`), `apply` (`src/apply.rs`), `layout` (`src/layout`), `project-store` (`src/project`), `review-flags` (`src/review.rs`), `outline` (`src/outline.rs`) |
  | `canvas-components` | `abyme-canvas` | `c4/component`, anchors under `crates/abyme-canvas/`: `canvas-ui` (`ui`), `canvas-commands` (`src/app.rs`), `admission` (`src/admission.rs`), `projection` (`src/projection.rs`), `file-watcher` (`src/watcher.rs`) |

- The relations of the context and the containers, all `uses`, besides one per direct non-dev
  dependency between the workspace's crates: `developer` to `abyme`, to `ai-agent`, and to
  `abyme-canvas`; `ai-agent` to `abyme`, to `abyme-mcp`, and to `project-repository`; `abyme` to
  `project-repository` and to `hugging-face-hub`; `abyme-core` to `project-files` and to
  `project-repository`; `abyme-index` to `search-cache` and to `hugging-face-hub`.
- Risk categories: Task 1 adds configuration that lets the AI agent start a local server which
  writes files of this repository.

## Steps

- [x] The repository is an Abyme project whose server Claude Code starts
- [x] The context and the containers are on boards
- [x] The components of abyme-core are on a board
- [x] The components of the canvas are on a board
- [x] Later code changes keep the model current

## Tasks

### Task 1: The repository is an Abyme project whose server Claude Code starts

`abyme-mcp init` makes the repository a project, and a committed `.mcp.json` lets every Claude Code
session in it start the server from the debug build.

Review path: Security-Hybrid

- [x] `/workspace` holds `abyme/abyme.toml`, made by `abyme-mcp init`, and `git status` shows no
      other new file of the project.
- [x] A committed `.mcp.json` at the repository root starts `./target/debug/abyme-mcp` in the
      format Claude Code's documentation gives; no setting pre-approves the server.
- [x] The Conventions of the root `CLAUDE.md` tell a reader what the server needs before it starts
      and when a rebuilt binary takes effect.
- [x] No sentence of the root `CLAUDE.md` says that a crate's directory lies in no project.
- [x] Obsidian leaves the files under `abyme/` out of the vault.

### Task 2: The context and the containers are on boards

The boards `context` and `containers` hold the elements, types, anchors, and relations of the model
in Context, laid out.

Review path: Reviewer-only

- [x] `overview` lists `context` with `containers` inside it, both `mirrored · current` with a
      summary, and the element counts match the files under `abyme/elements/`.
- [x] Every directory under `crates/` except `abyme-test-support` is the anchor of exactly one
      `c4/container` element, which `containers` places; every element has the type Context gives.
- [x] The elements relate exactly as Context lists, and the relations between container elements
      are the direct non-dev dependencies between the workspace's crates.
- [x] Every anchor names a path `git ls-files` lists.
- [x] Every anchored element carries a stamp by the AI on the commit HEAD named before this task's
      commit, and no element without an anchor carries one.
- [x] `status` reports no open review flags, and no `read_board` output of the two boards shows `⚠`.
- [x] A reader of the element `abyme` learns how the model is cut and what it leaves out, helper
      and entry-point modules included.
- [x] The board files are byte-identical before and after a second `layout_board` of each board,
      by a checksum listing of `abyme/boards/` from before and after in the handoff.

### Task 3: The components of abyme-core are on a board

The board `core-components`, the inside of `abyme-core`, holds its components and anchors as
Context gives them, related as their modules use each other, laid out.

Review path: Reviewer-only

- [x] `overview` shows `core-components` inside `containers`, `mirrored · current` with a summary.
- [x] Every component of the board has its type and anchor from Context, a path `git ls-files`
      lists, and a stamp by the AI on the commit HEAD named before this task's commit.
- [x] The relations between the components follow the `use crate::` imports between their modules.
- [x] `status` reports no open review flags, and no `read_board` output of the board shows `⚠`.
- [x] The board files are byte-identical before and after a second `layout_board`, by a checksum
      listing of `abyme/boards/` from before and after in the handoff.

### Task 4: The components of the canvas are on a board

The board `canvas-components`, the inside of `abyme-canvas`, holds its components and anchors as
Context gives them, related as their modules call each other, laid out.

Review path: Reviewer-only

- [x] `overview` shows `canvas-components` inside `containers`, `mirrored · current` with a
      summary.
- [x] Every component of the board has its type and anchor from Context, a path `git ls-files`
      lists, and a stamp by the AI on the commit HEAD named before this task's commit.
- [x] The relations between the components follow the calls between their modules and the Tauri
      commands the UI invokes.
- [x] `status` reports no open review flags, and no `read_board` output of the board shows `⚠`.
- [x] The board files are byte-identical before and after a second `layout_board`, by a checksum
      listing of `abyme/boards/` from before and after in the handoff.

### Task 5: Later code changes keep the model current

After a code change, the elements its anchors touch are flagged; the rule makes the AI bring them
up to date before the commit and verify them once the change is committed.

Review path: Reviewer-only

- [x] The Conventions of the root `CLAUDE.md` tell an AI agent that the model in `abyme/` describes
      this repository, that before a commit that changes code it calls `status` and brings each
      flagged element up to date with `apply`, that a new crate gets a container and a new module
      of `abyme-core` or `abyme-canvas` that carries a responsibility of its own a component, and
      that the element `abyme` says how the model is cut.
- [x] `.claude/CLAUDE.md` tells the lead that each task first verifies the elements the previous
      task's commit left flagged, with those stamps in its own commit and its handoff naming each
      flagged element and what was checked or changed, and that the stamps of a plan's last task go
      into the commit that marks the plan complete, which then changes nothing under `abyme/` but
      `verified` lines; Implementing a Task step 8, Completion step 2, and Skill-Output Commits agree
      with it.
- [x] The entry of `.ai/backlog.md` that names the remaining MVP plans no longer names the
      dogfooding on Abyme.

## Decisions

- **Relations are `uses` throughout:** a task flag then means that the element's own code changed,
  and a change passes only hints to what uses it.
- **Elements without an anchor stay unstamped:** Git cannot see the outside world, and a stamp on
  them would only collect hints.
- **The stamps of a code change travel in the next task's commit, and those of a plan's last task
  in the commit that marks it complete:** `verify` needs the change committed. Only that last commit
  carries stamps without a Reviewer; it changes nothing under `abyme/` but `verified` lines, whose
  `git diff` the completion report shows.
- **The Reviewer checks the model through its files and the pasted tool outputs:** it gets no MCP
  tools, and the criteria are checkable with Git and the files.
- **Components are the modules with a responsibility of their own:** helper and entry-point modules
  such as `slug.rs` or `main.rs` stay with their container, whose anchor covers them.
- **The element files stay out of the Obsidian vault:** the vault holds the docs, and the canvas
  shows the boards.
- **The canvas UI is a component of `abyme-canvas`, not a container:** it is bundled into the app,
  and one anchor cannot separate it from the crate.

## Non-Goals

- The docs map: the notes in `docs/` as elements with relations to the code (separate plan after
  this one).
- Several anchors per element, so that a module's `<name>.rs` and `<name>/` are one component
  (separate plan after this one).
- Stamps that survive a squash merge or a rebase.
- The history replay on a local clone of rlsp, which tests Abyme at size (a later MVP plan).
