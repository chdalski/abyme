---
tags:
  - vision
aliases:
  - Audience
  - Focus
  - Comparison
---
# Vision

## Problem

When people and an AI work on a project together today, the knowledge about it is scattered:

- documentation in `docs/`
- the AI's memory notes
- plans that often exist only locally
- diagrams in separate tools such as Miro

That something was implemented does not mean the documentation was updated. An example: a new microservice is introduced. Afterwards there is a memory note and a plan, but the architecture docs do not show it.

## Goal

Abyme is **one** place for project knowledge that people and the AI share:

- **For people** it is a visual working medium, a canvas like Miro. They use it to document projects, make them understandable, and discuss them with the AI.
- **For AIs** it is memory, reference work, and the basis for context, connected through the [[MCP Server]].
- **For both** it is the shared truth. What the AI remembers about the project, it writes where the person sees it too (see [[Project Knowledge and AI Memory]]).

The content stays current. Abyme detects when representations no longer match reality and reports dependencies that changes invalidate (see [[Reconciliation]]). Content is not only added but also removed.

## Audience

Abyme is a tool for **developers** who work with an AI. The focus is *visual* collaboration: developer and AI look at architecture, relationships, and the current and target state together on boards, discuss them, and change them.

- The core scenario is one developer working with an AI on the same board. Teams collaborate through Git, not in real time.
- The content comes from the software project and lives in the same repo (see [[Storage in Git]]).
- Live data with continuously changing values is [[Ideas#Deferred|deferred]].

What collaboration in the canvas could look like is collected in [[Ideas#Visual Collaboration with the AI]]; ideas for teams are in [[Ideas#Team Collaboration]].

## Pillars

| Pillar | Model | Core |
|---|---|---|
| Spatial | Miro | Free surface with elements, connections, and frames |
| Linked | Obsidian | Any number of boards; links to boards and to single elements; drill-down |
| Semantic | LanceDB / memory palace | Semantic search, memory for the AI |
| For both sides | MCP | People (canvas) and the AI (MCP) work on the same data |
| Reliable | build systems, referential integrity | Reconciliation with reality, dependencies, checks |

What is new above all is the combination of the last two pillars: a shared knowledge base for people and the AI that checks itself.

## Use Cases

- **C4 architecture:** Each level is the inside of an element on the level above. You zoom from the system context down to the components (see [[Board Graph]] and [[Example C4 for a Repo]]).
- **Discussing current and target:** On one board, a frame shows the current state, reconciled with the code. A second frame shows the target. The diff is visible and shrinks on its own as the work gets done (see [[Current and Target]]).
- **Context for the AI:** The AI starts from an overview, reads the relevant boards at a fitting detail level, and writes new knowledge back (see [[MCP Server#Detail Levels]]).
- **Everything in the repo:** database schemas, website components, docs, dependencies — anything the AI can interpret (see [[Hourglass#Inputs]]).

## Comparison

Not examined in detail:

| Tool | What it does | What Abyme wants beyond that |
|---|---|---|
| Miro | canvas; MCP access for AIs (read and write boards as SVG) | reconciliation with the code, dependency checks, role as AI memory |
| Heptabase | cards on several whiteboards, nested whiteboards | the AI as an equal user, reconciliation with reality |
| IcePanel / Structurizr | C4 with separate model and views, drill-down | not limited to architecture, current and target, AI interface |
| Obsidian Canvas / JSON Canvas | open canvas format, notes on the canvas | reconciliation, dependencies, checks |
| Graphiti (Zep) | time-aware knowledge graph as AI memory, invalidates facts instead of deleting them | a visual working medium for people |

As far as we know, none of them combines a canvas with equal access for people and the AI and with integrity checks. How Miro and tldraw give the AI access to their canvas is described in [[Placement and Layout#Prior Art]].
