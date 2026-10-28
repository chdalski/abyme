---
tags:
  - architecture
aliases:
  - Slice
  - MVP
  - Working Skeleton
  - Docs Map
---
# First Slice

The first slice has two stages. The **Working Skeleton** comes first and shows that the parts fit together before any feature depends on them. Then the **MVP** follows in feature slices and tests whether boards stay honest when the code changes.

## Working Skeleton

A thin path through all layers, without feature value of its own:

1. Through MCP, an `apply` writes an element. It appears live in the [[Canvas]], and `search` through MCP finds it.
2. A drag in the canvas lands in the board file, and `read_board` through MCP shows it.

This shows that a text access and a visual access work on the same files: two processes with the core embedded, file watchers for the live view, and TypeScript types generated from Rust. Because the skeleton already contains the search index, the build cost of LanceDB shows early. The crates are listed in [[Hexagonal Architecture#Crates]].

## MVP

The MVP tests the core loop of [[Reconciliation]]: the AI builds the boards, a code change flags what it touches, and the AI brings the boards up to date. The first use case is living architecture docs based on C4.

**In the MVP:**

- **Building:** Through the [[MCP Server]], the AI builds a C4 model and a **docs map** of a repo: the Markdown documents as document representations, with relations to the code they describe. Both come with anchors, inner boards, and auto-layout.
- **Reading:** `status`, `overview`, `read_board`, and `read_element`, in the [[MCP Server#Detail Levels|detail levels]].
- **Review flags:** They come from anchors and `git diff` and propagate along the relations (see [[Checks]]). `verify` resolves a flag and sets a new [[Checks#Verification Stamp|verification stamp]].
- **Search:** `search` is a hybrid search in LanceDB, filterable by statement kind (see [[Search and Index]]).
- **Canvas:** navigating inner boards with a breadcrumb, showing review flags and live changes, moving and pinning elements, creating and attaching [[Presentation#Sticky Notes|sticky notes]].
- **Zones:** boards with the modes free and mirrored and a statement kind, set as the pairs that [[Boards and Zones#Zone Presets|zone presets]] name.

**Not in the MVP:** current and target, synced zones and change requests, specifications, `render` and the visual check, exports, parsers, type packs as an extension mechanism (C4 is built in), references, stubs, and implied relations.

**What the MVP must show:**

- **Whether the waist fits** a text access (MCP) and a visual one (canvas) alike, see [[Hourglass#Risks]].
- **Whether detection through anchors holds up** over real history, see [[Reconciliation#Detecting Changes]].
- **How good the AI is as the main input**, see [[Hourglass#Inputs]].
- **Whether search finds by meaning** and keeps statement kinds apart, see [[Search and Index]].
- **Whether the canvas holds up**, including its performance under WebKitGTK, see [[Canvas#Platforms and Risks]].

> [!question] Proposal
> **After the MVP:** current and target, then synced zones with change requests, then [[Ideas#Specifications|specifications]], then render, exports, and parsers.

> [!todo] Open
> How are the MVP's results measured, for example the share of justified review flags in the replay?

## Test Setup

The MVP is tested first on Abyme's own repository: the AI builds the C4 model and the docs map of Abyme, and the boards are committed in the Abyme repository together with the code they describe. Abyme is developed with Abyme, so its boards follow every real change, and gaps show up in daily work.

[rlsp](https://github.com/chdalski/rlsp), the developer's own Rust repo, whose Markdown docs describe its code, is the second test project. It carries two tests that Abyme's young repository cannot carry yet. Its boards live only in a local clone that is never pushed, and the clone stays only as long as these two tests need it.

- **History replay:** A branch at a commit from early April 2026 receives the boards. Then `main` is merged into it commit by commit. Each merge is a code change, and the review flags must catch it. This makes the test repeatable even while the code itself is quiet.
- **Size test:** rlsp's code base is larger than Abyme's and shows whether Abyme holds up at that size.
