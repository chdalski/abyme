**Repository:** root
**Status:** NotStarted
**Created:** 2026-10-20

## Goal

The Working Skeleton, the MVP, and the architecture decisions
the skeleton rests on are settled, but the docs still present
them as open questions or show the earlier proposals. Record
them in the concept vault so that the docs describe the
decided state the skeleton and MVP plans build on.

## Context

- The concept vault is `docs/`. The notes this plan centers on
  are `docs/5 Architecture/First Slice.md` (still an
  undiscussed proposal), `docs/Decisions.md`, and
  `docs/Open Questions.md`.
- Render check for Mermaid diagrams: `npx -y
  @mermaid-js/mermaid-cli@12.0.0 -q -i "<note>.md" -o
  "<scratch dir>/<note>.md"` renders every diagram in a note
  and exits non-zero when one is broken. It needs headless
  Chrome's system libraries (`libnss3` and others), which the
  devcontainer image does not install yet.

## Steps

- [x] Settle the MVP, the Working Skeleton, and the skeleton
      design with the user, with the design advisor and the
      security engineer consulted
- [ ] Record the MVP and the Working Skeleton
- [ ] Record the architecture decisions for the Working
      Skeleton

## Tasks

### Task 1: Record the MVP and the Working Skeleton

Turn the First Slice proposal into the decided definition of
the Working Skeleton and the MVP, and remove what this settles
from the open lists. Decided content:

- **Order:** the Working Skeleton comes first, then the MVP in
  feature slices.
- **Working Skeleton:** a thin path through all layers without
  feature value. An `apply` over MCP writes an element; it
  appears live in the canvas, and `search` over MCP finds it.
  A drag in the canvas lands in the board file, and
  `read_board` over MCP shows it.
- **MVP:** through MCP, the AI builds a C4 model and a docs map
  (Markdown documents as document representations with
  relations to the code) with anchors, inner boards, and
  auto-layout. It reads through `status`, `overview`,
  `read_board`, and `read_element`. Review flags come from
  anchors and `git diff` and propagate along relations;
  `verify` sets a new verification stamp. `search` is a hybrid
  search in LanceDB, filterable by statement kind. The canvas
  navigates inner boards with a breadcrumb, shows review flags
  and live changes, and lets the developer move and pin
  elements and create and attach sticky notes. Zones use the
  modes free and mirrored; statement kinds come from zone
  presets.
- **Not in the MVP:** current and target, synced zones and
  change requests, specifications, `render` and the visual
  check, exports, parsers, type packs as an extension
  mechanism (C4 is built in), references, stubs, and implied
  relations.
- **After the MVP** (a proposal, not decided): current and
  target, then synced zones with change requests, then
  specifications, then render, exports, and parsers.
- **What the MVP must show:** whether the waist fits both a
  text and a visual access, whether detection through anchors
  holds up over real history, how good the AI is as the main
  input, whether search finds by meaning and keeps statement
  kinds apart, and whether the canvas holds up, including its
  performance under WebKitGTK.
- **Test setup:** a local clone of the developer's repository
  rlsp (https://github.com/chdalski/rlsp), never pushed. For a
  history replay, a branch at a commit from early April 2026
  receives the boards; then `main` is merged into it commit by
  commit, and each merge is a code change the review flags
  must catch.
- **First use case:** living architecture docs based on C4, as
  the MVP builds them. Planning a feature as a specification
  in the target is part of the after-MVP proposal, not an open
  question.
- **Open before the MVP:** how the MVP's results are measured,
  for example the share of justified review flags in the
  replay.

Review path: Reviewer-only

- [ ] A reader of `First Slice.md` learns what the Working
      Skeleton demonstrates, what the MVP contains and leaves
      out, what it must show, and how it is tested against
      rlsp; the note no longer says the slice is undiscussed
- [ ] `Decisions.md` holds the decisions on order, Working
      Skeleton, MVP scope, and test setup, each with its
      reason
- [ ] `Open Questions.md` no longer asks to define the Working
      Skeleton, the MVP, or the first use case, and lists how
      the MVP's results are measured as a question to answer
      before the MVP
- [ ] `Glossary.md` defines the term docs map
- [ ] Neither the status line in `docs/README.md` nor the
      concept-phase convention in the root `CLAUDE.md` says
      that the Working Skeleton and the MVP are still to be
      defined
- [ ] Every Mermaid diagram the task adds or changes passes
      the render check

### Task 2: Record the architecture decisions for the Working Skeleton

Bring the architecture and AI-interface notes in line with the
design decided for the skeleton, and update the decision and
question lists. Decided content:

- **IDs** of elements and boards are readable slugs. The core
  assigns them on creation and never changes them; a collision
  gets the suffix `-2`, `-3`, and so on. A sticky note's slug
  comes from its first words, or is `note` without text. Frame
  IDs are unique within their board. Board outlines over MCP
  show these slugs; per-response short IDs are dropped.
- **Files:** boards are JSON with one placement per line;
  element files stay Markdown with YAML frontmatter. The
  folders are `abyme/` in Git and `.abyme/` for the cache and
  the lock; `.abyme/` ignores itself through its own
  `.gitignore` containing `*`, so Abyme never edits the
  project's `.gitignore`.
- **Operations** are deltas held as data, such as creating,
  placing, moving, or pinning an element and attaching a note.
  Applying one is a pure function that returns the file
  writes. Neither the MCP server nor the canvas keeps a model
  in memory: the MCP server reads the files on every call, and
  the canvas holds only the board it shows and reloads it on
  file-watcher events.
- **Concurrent writing in stage A:** every operation is applied
  to the current files under an advisory lock in `.abyme/` and
  written atomically through a temporary file and a rename.
- **Crates of the skeleton:** `abyme-model`; `abyme-core` with
  the operations, the file access with lock and atomic writes,
  and layout as a module; `abyme-index` with LanceDB and
  fastembed, a crate of its own so that building the canvas
  never builds LanceDB; `abyme-mcp`; `abyme-canvas`. There is
  no core API trait yet; because operations are data, it can
  be extracted when stage B needs it. `abyme-layout`,
  `abyme-render`, `abyme-store-git`, `abyme-remote`, and
  `abyme-miro` come when something needs them.
- **Canvas:** it runs as a desktop app first; a canvas in the
  browser needs `abyme serve` from stage B. TypeScript types
  are generated from the Rust types with ts-rs; the few IPC commands are typed by hand in
  `AbymeClient`. Text measurement stays in Rust and comes with
  the MVP; the skeleton uses fixed sizes. Fixed sizes for the
  MVP were rejected, because layout without a screen must know
  how large a box with a given text becomes.
- **Search index:** only the MCP process owns it. Before each
  search it re-embeds the elements whose `source_hash`
  changed. Embeddings are computed locally with fastembed; the
  model is downloaded on first use into a cache outside the
  repository.
- **Review flags** are computed from anchors, verification
  stamps, Git, and relations; they are not stored.
- **Layout:** auto-layout is a layered (Sugiyama) layout. The
  library `Placement and Layout.md` names today is excluded,
  because its JavaScript port is licensed EPL-2.0 OR
  GPL-3.0-or-later.
- **Security baseline:** the core reads and writes only inside
  `abyme/` and `.abyme/` and treats input from the AI and from
  files in the repository as untrusted. The canvas renders no
  raw HTML, and its webview reaches only Abyme's own commands.
  Accepted risks: content in Abyme can carry a stored prompt
  injection that only Git and pull-request review catch; the
  local Git configuration and hooks are trusted; the AI
  client's own shell access is outside Abyme's scope.
- **Open before the Working Skeleton:** the YAML library for
  the frontmatter, which the skeleton already reads and
  writes, and the embedding model, which its `search` needs.
- **Open before the MVP:** Git access (the git CLI or a Rust
  Git library), the text layout library, and arrow routing.
- **Open later, as before:** whether structural checks belong
  to the format or to the core.

Review path: Reviewer-only

- [ ] Each decision above is stated as decided in the note it
      concerns: `Data Model.md`, `Storage in Git.md`,
      `Hexagonal Architecture.md`, `MCP Server.md`,
      `Search and Index.md`, `Placement and Layout.md`,
      `Canvas.md`, or `Checks.md`; the security baseline's
      file confinement and accepted risks are in
      `Hexagonal Architecture.md`, its canvas part in
      `Canvas.md`
- [ ] No note presents a settled alternative as current or
      open: per-response short IDs, slugs or ULIDs, boards as
      JSON or YAML, desktop first, stored or computed review
      flags, a core
      API trait with `embedded` and `remote` features for the
      skeleton, local or API embeddings, or the excluded
      layout library
- [ ] The crate diagram and table in
      `Hexagonal Architecture.md` show the skeleton's crates
      and mark the others as later
- [ ] `Decisions.md` holds each decision with its reason, and
      its architecture rows no longer mark the core API, the
      crates, and the runtime as a proposal
- [ ] `Open Questions.md` lists the questions named open above
      and none that this task settles, under headings that say
      when each must be answered: before the Working
      Skeleton, before the MVP, or later
- [ ] `Glossary.md` defines the term operation
- [ ] Every Mermaid diagram the task adds or changes passes
      the render check

## Decisions

- **Security baseline as a decision, not as controls:** the
  docs record the boundaries and accepted risks; the concrete
  controls come from the security engineer at each code
  task's gate.
- **Render-check tooling in a later plan:** installing headless
  Chrome's libraries in the devcontainer image is a
  configuration change, so it goes with the skeleton plan that
  also adds the skeleton's toolchain to the devcontainer.

## Non-Goals

- Code, the Cargo workspace, and devcontainer changes for the
  skeleton (Working Skeleton plans)
- Deciding the questions this plan lists as open
