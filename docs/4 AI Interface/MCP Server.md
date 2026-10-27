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

The server offers `apply`, `read_board`, `read_element`, `search`, `status`, and `verify` so far; the other tools come with the slices that need them. `status` counts the elements to review by their highest priority and lists them as in `search`, those with a task first, at most 50; when Git cannot be asked, it answers with an error that names the reason. `verify` takes an element ID and stamps the element as verified by the AI against the commit HEAD names in the repository its anchor lies in, nested or the project's; it refuses while files under the element's anchor differ from HEAD or are untracked, so the AI asks for a commit first, and it refuses an element whose anchor lies in a [[Reconciliation#Nested Repositories|nested repository]] that Abyme cannot read.

Every writing response contains the consequences of the change, that is affected elements and new review flags (see [[Checks#Reports]]). For visual collaboration, tools like `get_selection` and `focus` would be added, see [[Ideas#Visual Collaboration with the AI]].

## Detail Levels

Abyme gives the AI three detail levels in a compact text format instead of JSON:

| Level | Content | Size |
|---|---|---|
| **Overview** | all boards with a one-line summary, zone preset, and open checks | a few hundred tokens, the entry point of every session |
| **Board outline** | elements and relations as lines, inner boards as links, notes shortened | depends on the board size |
| **Element detail** | name, type, anchor, verification stamp, review flags, summary, the boards that place it, its inner boards, outgoing and incoming relations, and the full description; references come with their slice | per element |

Example of a board outline:

```
Board: container-level · Container Level
Inside of: web-shop-system · Above: context
Summary: The containers of the web shop and how they talk

checkout-service  c4/container  Checkout Service  order completion
orders-db         c4/datastore  Orders DB
payment-service   c4/container  Payment Service   Stripe payments  ▸ payment-components  ⚠ task
web-shop          c4/container  Web Shop          storefront

checkout-service → orders-db        uses  writes orders
checkout-service → payment-service  uses  REST, synchronous
web-shop → checkout-service         uses  REST
```

As a rough estimate, this takes 5 to 10 times fewer tokens than the same board as JSON with geometry. A prototype still has to confirm this.

- **Header:** The first line names the board's ID and title. A board that is an element's [[Board Graph|inner board]] names that element in a second line, `Inside of:`, and with `Above:` the board the element lies on. Merges and hand edits can break the hierarchy, so the line marks an element without a file `⚠ missing`, one that no board places `⚠ no home`, and a board that lies inside itself `⚠ cycle`. A board with a summary shows it in a line `Summary:`.
- **Inner boards:** An element line shows `▸` and the IDs of the boards the element owns, which `read_board` opens; `search` and `status` show them the same way.
- **Real IDs:** The outline shows the element IDs themselves, which are short, readable slugs (see [[Data Model#Rules]]). The server needs no state between calls, and an ID from an older response never points to the wrong element.
- **One line per element:** Text from boards and elements, relation labels included, stays on its line. Backslashes, line breaks, control characters, and invisible format characters are written as escapes such as `\\`, `\n`, and `\u{202e}`, so content can neither break the outline nor hide text from a reader.
- **Relations as arrows:** After the elements and a blank line, one line per relation between two elements on the board: source → target, kind, and label. They come by source, then by kind and target. Relations to elements on other boards are left out.
- **Missing elements:** A placement whose element file does not exist, for example after a merge, shows its ID and `⚠ missing`.
- **Markers:** An element line ends with `⚠ task` or `⚠ hint` when the element has [[Checks|review flags]], for their highest priority; `read_element` names them, and `search` marks its lines the same way. When the project lies in no Git repository, `git` cannot be run, or Git fails, the answer starts with `Review flags unavailable:` and the reason, and no line is marked.
- **Coordinates** only on request, when the AI wants to arrange elements: `read_board` with `"coordinates": true` shows each element's position and size right after its ID, such as `@400,200 180x90`.
- **Geometry is translated into relations.** “Lies in frame X” is reliable and important. “Lies near Y” is only a hint.
- **Board summaries** come from the AI, which writes them with `create_board` and `update_board`; nothing updates or flags them when the boards below change.

Example of an element detail, as `read_element` returns it:

```
Element: payment-service · Payment Service
Type: c4/container
Anchor: crates/payment-service
Verified: 4fc3a1e0b8d27c95e61f0a3d8b7c2e19f5a6d403 by ai
Review: task
  changed since the stamp: crates/payment-service/src/refund.rs
Summary: Stripe payments
Boards: container-level
Inner board: payment-components

Outgoing relations:
payment-service → stripe ⚠ missing  depends_on
payment-service → orders-db         uses  SQL

Incoming relations:
checkout-service → payment-service  uses  REST, synchronous

Description:
Handles payments through Stripe. Refunds run asynchronously
through the refund-worker.
```

- **Fixed order:** the sections always come in this order, with `none` where there is nothing; only a missing summary leaves out its line. The description comes last.
- **Anchor and stamp:** `Anchor:` shows the element's [[Elements#Anchors|anchor]], `Verified:` the full commit ID of its [[Checks#Verification Stamp|verification stamp]] and whether the AI or a person verified it.
- **Review flags:** `Review:` shows the highest priority of the element's [[Checks|review flags]], `task`, `hint`, or `none`, and one indented line per flag; a flag for changed files names at most 10 of them and how many more there are, and a flag passed on along a relation names the relation's kind and target and says that what the target's anchor stands for may have changed since the element's stamp. When the project lies in no Git repository, `git` cannot be run, or Git fails, the answer starts with `Review flags unavailable:` and the reason instead.
- **Boards:** `Boards:` names every board that places the element, `Inner board:` every board that is its inside.
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
