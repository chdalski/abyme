---
tags:
  - meta
aliases:
  - Start
---
# Abyme

> *Mise en abyme*: the picture within the picture. Boards you can zoom into, and inside them more boards.

Abyme is a tool for developers to work visually with an AI. Project knowledge lives on linked canvas boards in the repo. Developers and the AI document, discuss, and update it there together. The developer works in the canvas, the AI through an MCP server, both on the same data.

**Status:** The Working Skeleton is being built, as defined in [[First Slice]]: its core comes first, then search and the canvas. The MVP follows.

**Visual entry point:** [[Architecture Overview]]

## Map

### 1 Vision
- [[Vision]]: problem, goal, audience, comparison
- [[Guiding Principles]]

### 2 Concepts
- [[Elements]]: view and data, native or representation, anchors
- [[Boards and Zones]]: frames, modes, statement kinds, presets
- [[Board Graph]]: home, references, inner boards, deleting
- [[Relations]]
- [[Presentation]]: including sticky notes

### 3 Reconciliation
- [[Reconciliation]]: relation chain, detecting changes and their direction
- [[Checks]]
- [[Change Requests]]
- [[Current and Target]]

### 4 AI Interface
- [[MCP Server]]: tools and detail levels
- [[Placement and Layout]]
- [[Search and Index]]
- [[Project Knowledge and AI Memory]]
- [[Example C4 for a Repo]]

### 5 Architecture
- [[Architecture Overview]]
- [[Hourglass]]: inputs, waist, outputs
- [[Hexagonal Architecture]]: dependencies, crates, runtime
- [[Data Model]]: the Abyme format
- [[Storage in Git]]: layout and file format
- [[Canvas]]: Tauri and React Flow
- [[Extensions]]
- [[First Slice]]

### Cross-Cutting
- [[Decisions]]: what is settled
- [[Open Questions]]: what still needs answering
- [[Ideas]]: collected, not yet discussed
- [[Glossary]]

## Rules

- **The docs describe the current state, not how it came about.** Rejected options, earlier drafts, and research logs stay out. For a choice, one sentence on why it is so and what was rejected is enough.
- **Decided content carries no marker.** What is still a proposal goes in a callout `> [!question] Proposal`, what is open goes in `> [!todo] Open`.
- **Decisions** are a compact table: decision, why, link. When one changes, its row is replaced.
- **Open Questions** lists only what is open, with a link to the note. Whatever is settled leaves the list.
- **New terms** go into the [[Glossary]], so the docs and the later code use the same names.
- **Vault:** The project root is opened as an Obsidian vault. New notes go in `docs/`, attachments in `docs/Attachments/`. Links are wikilinks.
- **Diagrams** are in Mermaid. A solid border means decided, a dashed one a proposal, a dotted one later.
