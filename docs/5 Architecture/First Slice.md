---
tags:
  - architecture
aliases:
  - Slice
  - MVP
  - Working Skeleton
---
# First Slice

> [!question] Proposal
> Not discussed yet. The Working Skeleton and the MVP get defined before a prototype is built.

On a real repo:

1. Through MCP, the AI builds a C4 model and a map that connects Markdown files and code.
2. The canvas shows both, and a person edits something in it.
3. A code change triggers review flags through `git diff`, and the AI updates the board.
4. A feature is planned as a [[Ideas#Specifications|specification]] in the target. The AI implements it, the diff shrinks, and the target elements become representations.
5. An export to Mermaid or Miro shows what a one-way output loses.

What it should show:

- **Whether the waist fits.** It has to work equally for a text access (MCP) and a visual one (canvas), see [[Hourglass#Risks]].
- **Whether detection through anchors holds up**, see [[Reconciliation#Detecting Changes]].
- **Whether the whole cycle holds up:** target, implementation, current, shrinking diff, draft becomes representation, see [[Current and Target]].
- **How good the AI is as the main input**, see [[Hourglass#Inputs]].
- **What exports lose**, see [[Hourglass#Outputs]].
- **Whether the canvas holds up:** a board with frames, a reference, a relation to another board, zooming in, and a change by the AI that appears live. Plus the performance under WebKitGTK (see [[Canvas]]).
