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
- **The index is derived.** It is updated incrementally on every change and can be rebuilt from the data at any time. It is never the source of data and lives in the cache, not in Git.
- **What gets embedded** per element is its text plus context: type, path (e.g. “Shop › Payment › Stripe adapter”), and [[Boards and Zones#Statement Kind|statement kind]]. Coordinates are not part of it.
- **The statement kind is a filter**, so an idea is never found as a fact.

For a single project, structure and full text already go a long way. Embeddings pay off for fuzzy questions such as “Where did we have that rate-limiting thing?” and for many projects.

## LanceDB

> [!question] Proposal

The index lies as a LanceDB table in the cache, for example under `.abyme/cache/index.lance`. LanceDB has a Rust SDK (crate `lancedb`). The index is built by the crate `abyme-index` (see [[Hexagonal Architecture#Crates]]).

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

When an element file changes, its hash is compared with `source_hash`. Only changed rows get new text and a new embedding. If the cache is gone, it is rebuilt from `abyme/`. A second table for board summaries is conceivable.

> [!todo] Open
> Which embedding model? Local with a small model, for example via fastembed, or through an API? Local means: it works offline, and nothing leaves the machine.
