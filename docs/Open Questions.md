---
tags:
  - meta
---
# Open Questions

Only what is still open, with a link to the note where the question lives. Once a question is settled, it goes into the [[Decisions]] and leaves this list.

## Before the Working Skeleton

- **Confirm the core API, crates, and runtime**, see [[Hexagonal Architecture]].
- **Discuss the data model and file format**, see [[Data Model]] and [[Storage in Git]]. Open there: IDs as slugs or ULIDs, boards as JSON or YAML, folder names.
- **Review flags:** store or compute them? And do structural checks belong to the format or to the core? See [[Checks]].
- **Canvas:** desktop first? Which library for text layout? How are arrows routed? See [[Canvas]].

## Before the MVP

- **Measuring the results:** How are the MVP's results measured, for example the share of justified review flags in the replay? See [[First Slice]].

## Later

- **Visual collaboration:** How exactly do developer and AI work together in the canvas? See [[Ideas#Visual Collaboration with the AI]].
- **Team collaboration:** Does it stay with Git, or do sessions as participants, terminals, and real time come in? See [[Ideas#Team Collaboration]].
- **Protocol** for `abyme serve` and **concurrent editing**, see [[Hexagonal Architecture#Runtime]]
- **Board graph:** stubs, implied relations, zooming out, see [[Board Graph]]
- **Sticky notes:** references, colors, Markdown, see [[Presentation#Sticky Notes]]
- **Current and target:** a reached target, showing the diff, see [[Current and Target]]
- **Change requests:** display and format, see [[Change Requests]]
- **Exports** and **reliability of the AI**, see [[Hourglass]]
- **Embedding model**, see [[Search and Index]]
- **Provenance** of elements, see [[Ideas#Through Git]]
- **Extensions:** runtime, security, versioning, see [[Extensions]]
- **Several repos**, see [[Storage in Git]]
- **Several projects:** one data set per project, or links across project boundaries?
- **Roles and permissions**, when several people are involved
- **At a glance:** How do you recognize references and pinned positions?
