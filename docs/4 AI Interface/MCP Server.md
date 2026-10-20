---
tags:
  - ai
aliases:
  - MCP
  - Detail Levels
  - Detail Level
  - Board Outline
---
# MCP Server

Through the MCP server, the AI brings content into Abyme, and through it the AI reads Abyme as memory and context. It is the main input (see [[Hourglass#Inputs]]) and offers the same operations as the [[Canvas]] (see [[Data Model#Operations]]).

The AI describes *what* it wants, and Abyme decides *where* it goes. Instead of hundreds of single calls like “create shape” or “create arrow” with self-computed coordinates, there are a few higher-level calls (see [[Placement and Layout]] and [[Example C4 for a Repo]]).

## Tools

> [!question] Proposal
> The tool list is a first draft.

| Tool | Purpose |
|---|---|
| `status` | open checks, pending changes, state of the target zones |
| `overview` | overview of all boards |
| `search` | hybrid search, filterable by statement kind, type, and board |
| `read_board` | board outline |
| `read_element` | element detail |
| `apply` | a package of create, change, delete, and link, transactional |
| `create_from_template` | boards or frames from a template, e.g. C4 or Current/Target |
| `place` | place semantically or apply auto-layout |
| `render` | image of a board or a section |
| `checks` / `resolve_check` | fetch and resolve review flags and change requests |

Every writing response contains the consequences of the change, that is affected elements and new review flags (see [[Checks#Reports]]). For visual collaboration, tools like `get_selection` and `focus` would be added, see [[Ideas#Visual Collaboration with the AI]].

## Detail Levels

Abyme gives the AI three detail levels in a compact text format instead of JSON:

| Level | Content | Size |
|---|---|---|
| **Overview** | all boards with a one-line summary, zone preset, and open checks | a few hundred tokens, the entry point of every session |
| **Board outline** | elements and relations as lines, inner boards as links, notes shortened | depends on the board size |
| **Element detail** | full description, all relations, anchors, references, verification stamp | per element |

Example of a board outline:

```
Board: Container Level (current, verified @ abc123)
Above: Context · Below: Payment ▸, Checkout ▸

[1] Web Shop      container  React     storefront
[2] Checkout Svc  container  Kotlin    order completion
[3] Payment Svc   container  Go        Stripe payments   ⚠ review
[4] Orders DB     datastore  Postgres

1 → 2  REST
2 → 3  REST, synchronous
2 → 4  writes orders

Notes: "Bottleneck on Black Friday" (at 3)
```

As a rough estimate, this takes 5 to 10 times fewer tokens than the same board as JSON with geometry. A prototype still has to confirm this.

- **Short IDs** per response ([1], [2] …) instead of the real IDs. Abyme translates them back.
- **Coordinates** only on request, when the AI wants to arrange elements.
- **Geometry is translated into relations.** “Lies in frame X” is reliable and important. “Lies near Y” is only a hint.
- **Summaries** of boards are maintained automatically. When the content below them changes, they get a review flag themselves.

### Ranking

> [!question] Proposal

If the overview or a board outline does not fit into the token budget, a ranking decides what goes in. It is computed on every request as a personalized PageRank over the [[Board Graph]]:

- **Important** is whatever many relations and references point to.
- **Weighted higher** is whatever is close to the task: the selection in the canvas, the board the AI is working on, elements it has read or changed in this session, and open review flags.
- **Whatever does not fit** is summarized, such as “… and 14 more elements on 3 boards”. The AI loads more on purpose.

The AI states the budget in the call; otherwise a default applies.

Next: [[Search and Index]] · [[Project Knowledge and AI Memory]]
