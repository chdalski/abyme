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

Flags travel **one hop**: an element whose relation points to an element with an anchor is flagged when what that anchor stands for may have changed since its own stamp; [[Reconciliation#Detecting Changes]] says when Git shows that. A flag passed on this way is not passed on further, and an element without a stamp has no point in time to measure from, so it gets none. [[Relations]] lists what each kind passes on.

## Verification Stamp

Every representation carries “last verified against commit X, by the AI or a person”, and a native element can carry it too, as the base for the flags its relations pass on. That way the AI knows how fresh a statement is instead of trusting it blindly. A change of the element's content through `apply`, its relations included, removes the stamp, so a stamp never vouches for text nobody verified. Only the tool `verify` sets a stamp, with the commit HEAD names in the repository the element's anchor lies in, a [[Reconciliation#Nested Repositories|nested repository]] or the project's, and only while the files under the element's anchor are committed there. The verification stamp is also the starting point for [[Reconciliation#Detecting Changes|detecting changes]].

## Reports

Abyme does not contact the AI on its own, because MCP works on request. That is enough, because Abyme is used often as docs and memory. Reports come in two ways:

- **In every tool response** that concerns something, e.g.: “This change affects 3 dependent elements: …”
- **Through a status call** at the start of a session, e.g.: “3 open checks, 2 pending changes, target ‘Notifications’ 60 % reached.” `status` reports the review flags so far: how many elements have a task or only a hint, and their lines, tasks first; pending changes and targets come with their slices.

> [!todo] Open
> Do the structural checks belong to the format or to the core? That decides what the canvas alone can already check.
