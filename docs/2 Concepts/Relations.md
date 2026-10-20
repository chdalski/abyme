---
tags:
  - concept
aliases:
  - Relation
  - Relation Types
---
# Relations

Relations are typed. Only certain types trigger [[Checks]]; otherwise every change would cause a flood of warnings. [[Extensions#Type Packs|Type packs]] can contribute their own types.

| Type | Meaning | Triggers a check |
|---|---|---|
| contains | hierarchy, e.g. system → container | yes |
| depends on | A works only with B | yes |
| uses | runtime relation, e.g. a call | yes |
| corresponds to | target element ↔ current element | yes, for the [[Current and Target#Diff\|diff]] |
| replaces | a new element supersedes an old one | yes |
| concerns | a note, idea, or question refers to an element | yes, with low priority |
| see also | loose reference | no |

> [!question] Proposal
> The list of types is a first draft.

- **Arrows are relations.** A board shows the relations between the elements that lie on it. Arrows are not stored.
- **“contains”** usually follows from the inner board: whatever has its home inside X is contained in X.
- **Across board boundaries**, relations appear as stubs or implied relations, see [[Board Graph#Relations Across Boards]].

How changes travel along these relations is shown by the relation chain in [[Reconciliation]].
