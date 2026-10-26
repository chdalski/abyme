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
| **mirrored** | only reconciliation, i.e. the AI | only from outside in | crate overview that must always be right |
| **locked** | nobody | none | approved target picture, state of release 1.0 |

[[Presentation#Sticky Notes|Sticky notes]] are allowed as comments in every mode. They do not change the content of a zone. How reconciliation works is described in [[Reconciliation]].

## Statement Kind

So that the AI weighs content correctly, every piece of content has a statement kind. It does not say how finished something is, but what kind of statement it is.

| Kind | Meaning | How the AI treats it |
|---|---|---|
| **current** | describes today's reality | counts as fact, must match reality, gets checked |
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
| Current | mirrored or synced | current |
| Target | free | target |
| Brainstorm | free | idea |
| Archive / snapshot | locked | historic |

Some element types bring their own kind: question, ADR (= decision), and knowledge note (= knowledge). The AI sets the settings itself when it creates something, because it knows which zone it writes to. Overriding them by hand is rarely necessary.

## Inheritance

Elements inherit the settings of the zone they lie in. Frame settings take precedence over board settings, element settings over both.
