---
tags:
  - concept
aliases:
  - Board
  - Frame
  - Zone
  - Zone Presets
  - Modes
  - Mode
  - Statement Kind
---
# Boards and Zones

- **Board:** an infinite surface with elements, arrows, and frames.
- **Frame:** a named area on a board.
- **Zone:** a board or a frame that carries settings: a mode and a statement kind. Zones can also have an [[Elements#Anchors|anchor]], such as a Current frame anchored on `crates`.

## Modes

A mode answers two questions: Who may change things on the board? And is it reconciled with an external resource?

| Mode | Who changes the board | Reconciliation with the outside | Example |
|---|---|---|---|
| **free** | people and the AI | none | brainstorm, target draft |
| **synced** | people and the AI | in both directions | current architecture that you also want to rework on the board |
| **mirrored** | content only reconciliation, i.e. the AI; its arrangement also people | only from outside in | crate overview that must always be right |
| **locked** | nobody | none | approved target picture, state of release 1.0 |

Abyme knows the modes free and mirrored so far. What a mirrored board shows follows the code, but people may arrange it: in the [[Canvas]], a person moves, pins, and releases boxes there as on a free board, so everyone sees the same arrangement, and the AI's layout leaves pinned boxes where they are. The AI and edits by hand still change the board.

[[Presentation#Sticky Notes|Sticky notes]] are allowed as comments in every mode. They do not change the content of a zone. How reconciliation works is described in [[Reconciliation]].

## Statement Kind

So that the AI weighs content correctly, content carries a statement kind, normally from the zone it has its home in (see [[#Inheritance]]). It says what the content claims, not that it was checked. It does not say how finished something is, but what kind of statement it is.

| Kind | Meaning | How the AI treats it |
|---|---|---|
| **current** | describes today's reality | counts as fact, must match reality; review flags check it where it has an anchor |
| **target** | agreed goal | goal of the implementation, not yet reality |
| **decision** | was made, with reasons | holds until it is superseded |
| **idea** | non-binding | only as a suggestion, never quoted as fact |
| **knowledge** | experience and quirks, e.g. “tests need Docker” | like today's memory notes |
| **question** | open | get an answer |
| **historic** | frozen state | as a look back, no deviation check |

An example: asked “How does auth work?”, the AI answers from the current state. At most it adds: “On the brainstorm board there is the idea of switching to OAuth.” In [[Search and Index|search]], the statement kind is a filter, so an idea is never found as a fact.

## Zone Presets

Nobody should have to maintain settings per element. For a board or a frame, you choose a preset:

| Preset | Mode | Statement kind |
|---|---|---|
| Current | mirrored | current |
| Target | free | target |
| Brainstorm | free | idea |
| Archive / snapshot | locked | historic |

A zone stores its mode and its statement kind, never the preset's name: a preset only names a usual pair. The AI gives a board the Current preset as `"zone": {"mode": "mirrored", "statement": "current"}` (see [[Storage in Git#Board File]]). Abyme knows the modes free and mirrored and every kind but historic so far, so Archive comes with locked, and a Current zone may also be synced once synced zones exist.

The AI sets the zone itself when it creates a board, because it knows what the board holds.

## Inheritance

An element takes the statement kind of the zone it has its home in, its home board. Without a home, or when its home board is no zone, it has none: no other board that shows it and no board above stands in, so nothing becomes a fact by accident. As the exception, an element can hold a kind of its own, such as a question on a board of current facts, which takes precedence over the zone and needs no home; that is a deliberate claim, visible in the diff, not a check. The AI sees each element's kind and, in `read_element`, where it comes from in the [[MCP Server#Detail Levels|detail levels]].

> [!question] Proposal
> Frames get settings of their own, and some element types bring their own kind: question, ADR (= decision), and knowledge note (= knowledge). Frame settings take precedence over board settings.
