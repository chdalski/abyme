---
tags:
  - meta
---
# Decisions

What is settled, briefly and with a reason. Details are in the linked note. A changed decision is replaced here, not appended.

## Concept

| Decision | Why | More |
|---|---|---|
| The board is a view, not the source of data. Elements have a stable identity that does not depend on their position. | Copies on several boards drift apart. Queries and dependencies need elements, not drawings. | [[Elements]] |
| Elements are native or a representation with an anchor. | Reconciliation and the chain of dependencies depend on it. | [[Elements#Native or Representation]] |
| Every element has a home and appears elsewhere as a reference. Each one can have an inner board. | No copies, no drift, unambiguous deletion. C4 levels come from zooming in. | [[Board Graph]] |
| Removing something from the board means deleting it, namely what you click. For representations in synced zones, this becomes a change request. | There is no hidden hiding. | [[Board Graph#Deleting]] |
| Zones have a mode: free, synced, mirrored, or locked. Sticky notes are allowed in every mode. | The mode determines who may change things and whether reconciliation happens. | [[Boards and Zones#Modes]] |
| The statement kind is inherited through zone presets. The AI sets it itself when creating content. | Nobody should have to maintain it per element. | [[Boards and Zones#Statement Kind]] |
| Relations are typed. Only certain types trigger checks. | Otherwise every change would cause a flood of warnings. | [[Relations]] |
| How something is presented is part of the board's content. | All outputs show the same thing, as faithfully as they can. | [[Presentation]] |

## Reconciliation

| Decision | Why | More |
|---|---|---|
| Structural checks run deterministically, content checks by flagging and reviewing. | Whether a statement still holds can only be judged by the AI or a person. | [[Checks]] |
| Reports come on request: in the tool responses and through a status call. | As docs and memory, Abyme is queried often enough. | [[Checks#Reports]] |
| Current and target lie as frames on one board, connected through correspondence links. The plan does not have to live in Abyme. | Looking at several boards in parallel is cumbersome. What matters is discussing current and target well. | [[Current and Target]] |
| Changes in the repo are detected through anchors and Git, read through the `git` command line. Parsers are optional. | No code is needed for each individual source; the command line adds no dependency, and Git documents its plumbing output. | [[Reconciliation#Detecting Changes]] |
| An element anchored in a repository nested in the project is measured in that repository. Abyme finds nested repositories only through the outer repository's Git output and reads one only when an anchor lies in it; an anchor that contains one is measured in its own repository, and relations in the source's. Clones in ignored folders are not read. | A stamp pins one commit of one repository; Git output names only real nested repositories, and listing every ignored file on every read costs too much. | [[Reconciliation#Nested Repositories]] |
| An anchor in the project is a path relative to the project root, such as `crates/payment-service`, and covers everything below it. It is rejected, never normalized, when it could name something other than it seems to. | A `file://` URI reads the first segment as a host; a trailing-slash convention would let a folder anchor silently miss changes. | [[Data Model#Rules]] |
| Only `verify` sets a verification stamp, with the commit the server reads itself; every change of an element's content through `apply`, its anchor and relations included, removes it. | A request that could set a stamp could hide a change; a stamp must not vouch for content nobody verified. | [[Checks#Verification Stamp]] |

## AI

| Decision | Why | More |
|---|---|---|
| Project knowledge lives in Abyme, the way of working in the AI's memory. Shipped with it: MCP server instructions, an example for the CLAUDE.md, and hooks. | The AI's memory and the documentation should be the same thing. | [[Project Knowledge and AI Memory]] |
| The AI places semantically, uses auto-layout, and coordinates when needed. What people pinned stays. An image shows it what reaches the person. | The AI says what it wants, and Abyme decides where it goes. | [[Placement and Layout]] |
| The AI reads in three detail levels in compact text. Search is hybrid, the index derived. To be tested on the prototype. | saves tokens, and the index is never the source of data | [[MCP Server#Detail Levels]], [[Search and Index]] |

## Architecture

| Decision | Why | More |
|---|---|---|
| Hourglass: inputs and outputs are connected through a narrow waist, the Abyme format. The main input is the AI via MCP. | N + M instead of N × M implementations | [[Hourglass]] |
| The own canvas is the only full-featured interface. Miro and other systems get one-way exports. | Systems without modes, references, and inner boards would constrain the waist. | [[Hourglass#Outputs]] |
| The data lives in Git, in the same repo as the code. What is no longer true is deleted, not archived. | History and base snapshots come from Git. Boards land in the same pull request as the code. | [[Storage in Git]] |
| Hexagonal architecture: canvas, core, and MCP server can be used independently, dependencies point inward. | The visual medium is a projection of the data from the core. | [[Hexagonal Architecture]] |
| Runtime stage A first: everything runs embedded, and the files are the shared state. The core API is the public API of `abyme-core`; the trait `AbymeApi` comes with stage B. | A trait with one implementation adds nothing; because operations are data, it can be extracted later. | [[Hexagonal Architecture#Core API]] |
| The Working Skeleton has the crates `abyme-model`, `abyme-core`, `abyme-index`, `abyme-mcp`, and `abyme-canvas`. Layout is a module of the core; the other crates come when something needs them. | fewest elements; `abyme-index` is a crate of its own so that building the canvas never builds LanceDB | [[Hexagonal Architecture#Crates]] |
| IDs are readable slugs that the core assigns and never changes; collisions get a suffix. Board outlines show the slugs. | Relations, placements, diffs, and outlines stay readable, and the MCP server needs no state between calls. | [[Data Model#Rules]] |
| Elements are Markdown with YAML frontmatter, boards JSON with one placement per line. The folders are `abyme/` and `.abyme/`, which ignores itself. | diff-friendly; Abyme never edits the project's `.gitignore` | [[Storage in Git]] |
| The frontmatter is read with `rlsp-yaml-parser` and written by Abyme's own emitter, which double-quotes every value. `type`, `name`, `summary`, and relation labels hold only YAML-printable characters, and the frontmatter is at most 64 KiB. | A spec-faithful YAML 1.2 parser with limits against hostile input; it has no emitter, rejects non-printable characters even as escapes, and parses escapes slowly. | [[Storage in Git#Element File]] |
| An element's outgoing relations are a list in its frontmatter, one line per relation with kind, target, and optional label, sorted; without relations the key is left out. | One changed relation is one changed line, and existing files stay valid. A separate file would split what belongs to the element, the board file would mix content into layout, and a key per kind would leave no room for the label. | [[Storage in Git#Element File]] |
| The relation kinds are a closed set: `concerns`, `depends_on`, `describes`, `see_also`, and `uses`. `contains` comes with inner boards, `corresponds_to` and `replaces` with current and target. | Every rule that depends on the kind, such as which kinds trigger checks, names each kind. `contains` usually follows from the inner board. | [[Relations]] |
| Operations are deltas held as data; applying them is a pure function that returns the file writes. Neither process keeps a model in memory. | testable without a file system; no stale copies | [[Data Model#Operations]] |
| Every operation is applied to the current files under an advisory lock and written atomically. | Two processes write the same files; a writer never overwrites a change it has not seen. | [[Storage in Git#Writing]] |
| A repository becomes an Abyme project only through `abyme-mcp init`. Writing recreates `.abyme/` but never creates a project. | A server registered for all projects would otherwise let one `apply`, possibly steered by content the AI read, create `abyme/` in any repository. | [[Storage in Git#Project]] |
| The project root is the nearest directory upwards with `abyme/abyme.toml`, also from Git repositories nested in the project, and it is used only when it belongs to the user running Abyme. | Repositories nested in a project belong to it; a project file that another local user places above the working directory must never be used. | [[Storage in Git#Project]] |
| Review flags are computed, not stored. | They cannot go stale. | [[Checks]] |
| Auto-layout is a layered (Sugiyama) layout. | suits box-and-arrow diagrams such as C4; copyleft layout libraries are excluded by the MIT rule | [[Placement and Layout]] |
| The canvas runs as a desktop app first. Types come from Rust through ts-rs. Text measurement stays in Rust and comes with the MVP. | Tauri has access to files and Git; the format is defined once; layout without a screen must know box sizes. | [[Canvas]] |
| Only the MCP process owns the search index and brings it up to date before each search by comparing the text it would embed with the stored text. Embeddings are computed locally with fastembed. | Search is the index's only consumer; offline after the model download, and no text leaves the machine. Comparing the text also catches context that does not live in the element file. | [[Search and Index]] |
| The search index and the model files live in the user's cache, never in the repository. | A repository can commit files into `.abyme/` that LanceDB would read. | [[Search and Index#Where Search Keeps Its Files]] |
| Abyme downloads the embedding model itself when the MCP server starts in a project, from a revision fixed in its source, and checks every model file against a size and a SHA-256 before every load. ONNX Runtime comes prebuilt, checked against the hash the pinned `ort-sys` crate carries. | Native code parses the model, so it must get exactly the files that were checked; fastembed's own download follows the latest revision and checks nothing. Starting at server start keeps a long download out of the first search's request. | [[Search and Index#Embedding Model]] |
| Each project picks its embedding model in `abyme/abyme.toml` from models built into Abyme: `bge-small-en-v1.5` (default, English) or `paraphrase-multilingual-minilm-l12-v2`. | The language of a project's text decides which model finds by meaning; built-in names keep repository content out of download URLs and paths. | [[Search and Index#Embedding Model]] |
| Security baseline: the core writes only inside `abyme/` and `.abyme/`, reads outside them only through Git with fixed arguments, never opens a path built from an anchor, and treats input from the AI and the repository as untrusted; the canvas renders no raw HTML and reaches only Abyme's own commands. Git runs in a nested repository only after checks of where it lies, with its own filters and hooks turned off, and never uses a transport. Accepted: stored prompt injection, also through file names in review flags, the project repository's Git configuration, hooks, and filters and the `git` on `PATH` as trusted, in nested repositories the user's global and system filters and what their Git directories hold, the AI client's shell out of scope, the prebuilt ONNX Runtime and the pinned model files once their hashes match, and the user's cache. | The AI can be steered by content it reads, and files arrive through clones and pull requests. | [[Hexagonal Architecture#Security Baseline]] |

## Project and Technology

| Decision | Why | More |
|---|---|---|
| Abyme is for developers who work visually with an AI. Live data is deferred. | focus | [[Vision#Audience]] |
| Rust with a Cargo workspace; the crates live under `crates/`. | – | [[Hexagonal Architecture#Crates]] |
| Open source under MIT. Dependencies must be compatible with it. | – | – |
| The canvas is a Tauri app with React Flow; the UI is TypeScript. | ready-made mechanics, MIT, controlled from outside | [[Canvas#Why Tauri and React Flow]] |
| The repository, including the docs, is written in English. | – | – |

## First Slice

| Decision | Why | More |
|---|---|---|
| The Working Skeleton comes first, then the MVP in feature slices. | Two processes, file watchers, generated types, and the LanceDB build show their risks before features depend on them. | [[First Slice#Working Skeleton]] |
| The Working Skeleton is a thin path through all layers without feature value: an element written by `apply` over MCP appears live in the canvas, and `search` finds it; a drag in the canvas lands in the board file, and `read_board` shows it. | It proves that a text access and a visual access work on the same files. | [[First Slice#Working Skeleton]] |
| The MVP tests the core loop of reconciliation on a C4 model and a docs map, with search, a canvas for viewing and light editing, and the modes free and mirrored. | Current and target, change requests, and specifications build on boards that stay honest; if that fails, nothing built on it holds. | [[First Slice#MVP]] |
| Current and target, synced zones and change requests, specifications, `render` and the visual check, exports, parsers, type packs as an extension mechanism (C4 is built in), references, stubs, and implied relations are not in the MVP. | focus on the core loop | [[First Slice#MVP]] |
| The MVP is tested first on Abyme's own repository, with its boards committed there. A local clone of rlsp serves the history replay and the size test, as long as they need it. | Abyme is developed with Abyme, so its boards follow real changes every day. rlsp brings the long history and the size that Abyme's repository does not have yet; the clone keeps that experiment out of the published repo. | [[First Slice#Test Setup]] |
