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
| `read_element` | element detail: description, boards, and relations in both directions |
| `apply` | a package of create, change, delete, and link, transactional |
| `create_from_template` | boards or frames from a template, e.g. C4 or Current/Target |
| `place` | place semantically or apply auto-layout |
| `render` | image of a board or a section |
| `checks` / `resolve_check` | fetch review flags and change requests, resolve change requests |
| `verify` | resolve a review flag and set a new verification stamp |

The server offers `apply`, `read_board`, `read_element`, and `search` so far; the other tools come with the slices that need them.

Every writing response contains the consequences of the change, that is affected elements and new review flags (see [[Checks#Reports]]). For visual collaboration, tools like `get_selection` and `focus` would be added, see [[Ideas#Visual Collaboration with the AI]].

## Detail Levels

Abyme gives the AI three detail levels in a compact text format instead of JSON:

| Level | Content | Size |
|---|---|---|
| **Overview** | all boards with a one-line summary, zone preset, and open checks | a few hundred tokens, the entry point of every session |
| **Board outline** | elements and relations as lines, inner boards as links, notes shortened | depends on the board size |
| **Element detail** | name, type, summary, the boards that place it, outgoing and incoming relations, and the full description; anchors, references, and the verification stamp come with their slices | per element |

Example of a board outline:

```
Board: container-level · Container Level (current, verified @ abc123)
Above: context · Below: payment-components ▸, checkout-components ▸

web-shop          container  React     storefront
checkout-service  container  Kotlin    order completion
payment-service   container  Go        Stripe payments   ⚠ review
orders-db         datastore  Postgres

checkout-service → orders-db        uses  writes orders
checkout-service → payment-service  uses  REST, synchronous
web-shop → checkout-service         uses  REST

Notes: "Bottleneck on Black Friday" (at payment-service)
```

As a rough estimate, this takes 5 to 10 times fewer tokens than the same board as JSON with geometry. A prototype still has to confirm this.

- **Real IDs:** The outline shows the element IDs themselves, which are short, readable slugs (see [[Data Model#Rules]]). The server needs no state between calls, and an ID from an older response never points to the wrong element.
- **One line per element:** Text from boards and elements, relation labels included, stays on its line. Backslashes, line breaks, control characters, and invisible format characters are written as escapes such as `\\`, `\n`, and `\u{202e}`, so content can neither break the outline nor hide text from a reader.
- **Relations as arrows:** After the elements and a blank line, one line per relation between two elements on the board: source → target, kind, and label. They come by source, then by kind and target. Relations to elements on other boards are left out.
- **Missing elements:** A placement whose element file does not exist, for example after a merge, shows its ID and `⚠ missing`.
- **Coordinates** only on request, when the AI wants to arrange elements: `read_board` with `"coordinates": true` shows each element's position and size right after its ID, such as `@400,200 180x90`.
- **Geometry is translated into relations.** “Lies in frame X” is reliable and important. “Lies near Y” is only a hint.
- **Summaries** of boards are maintained automatically. When the content below them changes, they get a review flag themselves.

Example of an element detail, as `read_element` returns it:

```
Element: payment-service · Payment Service
Type: c4/container
Summary: Stripe payments
Boards: container-level

Outgoing relations:
payment-service → stripe ⚠ missing  depends_on
payment-service → orders-db         uses  SQL

Incoming relations:
checkout-service → payment-service  uses  REST, synchronous

Description:
Handles payments through Stripe. Refunds run asynchronously
through the refund-worker.
```

- **Fixed order:** the sections always come in this order, with `none` where there is nothing; the description comes last.
- **The description keeps its lines and tabs.** Other control and invisible characters in it are written as escapes such as `\u{202e}`, so it can hide or reorder nothing; a carriage return before a line break is left out. Every other value is escaped onto its line as in the board outline.
- **Missing targets:** a relation whose target has no element file shows the target with `⚠ missing`.

### Ranking

> [!question] Proposal

If the overview or a board outline does not fit into the token budget, a ranking decides what goes in. It is computed on every request as a personalized PageRank over the [[Board Graph]]:

- **Important** is whatever many relations and references point to.
- **Weighted higher** is whatever is close to the task: the selection in the canvas, the board the AI is working on, elements it has read or changed in this session, and open review flags.
- **Whatever does not fit** is summarized, such as “… and 14 more elements on 3 boards”. The AI loads more on purpose.

The AI states the budget in the call; otherwise a default applies.

Next: [[Search and Index]] · [[Project Knowledge and AI Memory]]
