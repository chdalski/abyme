---
tags:
  - ai
aliases:
  - Search
  - Search Index
  - LanceDB
---
# Search and Index

- **Hybrid search** from full text and vectors. LanceDB can do both, locally and embedded.
- **The index is derived.** It is brought up to date before each search and can be rebuilt from the data at any time. It is never the source of data and lives in the cache, not in Git.
- **Embeddings are computed locally** with fastembed. The model is downloaded on first use into a cache outside the repository. After that, search works offline, and no text leaves the machine.
- **What gets embedded** per element is its text plus context: type, path (e.g. “Shop › Payment › Stripe adapter”), and [[Boards and Zones#Statement Kind|statement kind]]. Coordinates are not part of it.
- **The statement kind is a filter**, so an idea is never found as a fact.

For a single project, structure and full text already go a long way. Embeddings pay off for fuzzy questions such as “Where did we have that rate-limiting thing?” and for many projects.

## LanceDB

The index lies as a LanceDB table in `.abyme/cache/`. LanceDB has a Rust SDK (crate `lancedb`). The index is built by the crate `abyme-index` (see [[Hexagonal Architecture#Crates]]).

**Only the MCP process owns the index.** Neither `apply` nor the core touches it. Before each `search`, the MCP process compares the hash of every element file with `source_hash` and re-embeds only what changed. Edits in the canvas, checkouts, and edits by hand can happen while no MCP process is running, so this catch-up is needed anyway.

> [!question] Proposal
> The table below is a draft.

Table `elements`:

| Column | Type | Purpose |
|---|---|---|
| `id` | string | element ID from the [[Data Model]] |
| `board_path` | string | context, e.g. “Shop › Payment” |
| `type_ref` | string | filter by type |
| `statement` | string | effective statement kind, filter |
| `text` | string | name, summary, description, and context, for full text and embedding |
| `vector` | list of float32 | embedding of `text` |
| `source_hash` | string | hash of the element file, so only what changed is recomputed |

Only changed rows get new text and a new embedding. If the cache is gone, it is rebuilt from `abyme/`. A second table for board summaries is conceivable.

> [!todo] Open
> Which embedding model does fastembed load?
