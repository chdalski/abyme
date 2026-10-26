---
tags:
  - concept
aliases:
  - Relation
  - Relation Types
---
# Relations

Relations are typed. Only certain types trigger [[Checks]]; otherwise every change would cause a flood of warnings. [[Extensions#Type Packs|Type packs]] can contribute their own types.

| Type | Name in files and requests | Meaning | Triggers a check |
|---|---|---|---|
| contains | `contains` | hierarchy, e.g. system → container | yes |
| depends on | `depends_on` | A works only with B | yes |
| uses | `uses` | runtime relation, e.g. a call | yes |
| describes | `describes` | a document describes an element, e.g. in the [[First Slice#MVP\|docs map]] | yes |
| corresponds to | `corresponds_to` | target element ↔ current element | yes, for the [[Current and Target#Diff\|diff]] |
| replaces | `replaces` | a new element supersedes an old one | yes |
| concerns | `concerns` | a note, idea, or question refers to an element | yes, with low priority |
| see also | `see_also` | loose reference | no |

- **Stored today:** `concerns`, `depends_on`, `describes`, `see_also`, and `uses`. Element files and requests know no other kind. `contains` comes with inner boards, which it usually follows from; `corresponds_to` and `replaces` come with [[Current and Target|current and target]].
- **A relation goes out from an element** and has a kind, a target, and an optional label, such as “REST”. An element has at most one relation of a kind to a target, and none to itself. How relations are stored is described in [[Storage in Git#Element File]].
- **Arrows are relations.** A board shows the relations between the elements that lie on it. Arrows are not stored.
- **“contains”** usually follows from the inner board: whatever has its home inside X is contained in X.
- **Across board boundaries**, relations appear as stubs or implied relations, see [[Board Graph#Relations Across Boards]].

How changes travel along these relations is shown by the relation chain in [[Reconciliation]].

> [!todo] Open
> Does `describes` trigger a task or only a hint?
