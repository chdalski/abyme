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
| Changes in the repo are detected through anchors and `git diff`. Parsers are optional. | No code is needed for each individual source. | [[Reconciliation#Detecting Changes]] |

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
| Hexagonal architecture: canvas, core, and MCP server can be used independently, dependencies point inward. *Proposal:* core API as a trait, crates, and runtime in stages. | The visual medium is a projection of the data from the core. | [[Hexagonal Architecture]] |

## Project and Technology

| Decision | Why | More |
|---|---|---|
| Abyme is for developers who work visually with an AI. Live data is deferred. | focus | [[Vision#Audience]] |
| Rust with a Cargo workspace; the crates live under `crates/`. | – | [[Hexagonal Architecture#Crates]] |
| Open source under MIT. Dependencies must be compatible with it. | – | – |
| The canvas is a Tauri app with React Flow; the UI is TypeScript. | ready-made mechanics, MIT, controlled from outside | [[Canvas#Why Tauri and React Flow]] |
| The repository, including the docs, is written in English. | – | – |
