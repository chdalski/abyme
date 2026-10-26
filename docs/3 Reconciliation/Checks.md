---
tags:
  - reconciliation
aliases:
  - Review Flag
  - Review Flags
  - Verification Stamp
  - Reports
---
# Checks

## Structural (Deterministic)

- dead links and references
- deleted targets of relations
- orphaned elements
- elements without a home
- type rules, e.g. “a container belongs to a system”
- duplicate anchors, e.g. “there already is an element for `crates/payment`”
- impact analysis: who depends on X, directly or indirectly?
- *Proposal:* relations that the code does not bear out, through the [[Extensions#Code Parser|code parser]]

## Content (Flag, Then Review)

When an element changes, the system sets review flags along the [[Relations]] that trigger checks. This works like the dirty flag in build systems. In classic AI, such a thing is called a truth maintenance system. Whether a statement still holds is then judged by the AI or a person.

Review flags are not stored. They are computed whenever they are needed, from the anchors, the [[#Verification Stamp|verification stamps]], Git, and the relations, so they cannot go stale.

Two things help against a flood of warnings:

- Only certain relation types trigger checks.
- There are priorities: a “concerns” is a hint, a “depends on” is a task.

## Verification Stamp

Every representation carries “last verified against commit X, by the AI or a person”, and a native element can carry it too, as the base for the flags its relations pass on. That way the AI knows how fresh a statement is instead of trusting it blindly. A change of the element's content through `apply`, its relations included, removes the stamp, so a stamp never vouches for text nobody verified. The verification stamp is also the starting point for [[Reconciliation#Detecting Changes|detecting changes]].

## Reports

Abyme does not contact the AI on its own, because MCP works on request. That is enough, because Abyme is used often as docs and memory. Reports come in two ways:

- **In every tool response** that concerns something, e.g.: “This change affects 3 dependent elements: …”
- **Through a status call** at the start of a session, e.g.: “3 open checks, 2 pending changes, target ‘Notifications’ 60 % reached.”

> [!todo] Open
> Do the structural checks belong to the format or to the core? That decides what the canvas alone can already check.
