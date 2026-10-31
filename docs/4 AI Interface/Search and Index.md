---
tags:
  - ai
aliases:
  - Search
  - Search Index
  - LanceDB
  - Embedding Model
---
# Search and Index

- **Hybrid search** from full text and vectors. LanceDB can do both, locally and embedded.
- **The index is derived.** It is brought up to date before each search and can be rebuilt from the data at any time. It is never the source of data and lives in the user's cache, not in the repository.
- **Embeddings are computed locally** with fastembed, from a model that Abyme downloads once and checks itself (see [[#Embedding Model]]). After that, search works offline, and no text leaves the machine.
- **What gets embedded** per element is its type, name, summary, and description. Coordinates are not part of it.
- **The [[Boards and Zones#Statement Kind|statement kind]] is a filter:** `search` with a `statement` ranks only among the elements of that kind, so an idea is never found as a fact. An element without a kind matches no filter, and without a `statement` search finds every element.

For a single project, structure and full text already go a long way. Embeddings pay off for fuzzy questions such as “Where did we have that rate-limiting thing?” and for many projects.

## Where Search Keeps Its Files

Search keeps its files in the user's cache: in `$XDG_CACHE_HOME/abyme/` when `XDG_CACHE_HOME` is an absolute path, otherwise in `$HOME/.cache/abyme/` when `HOME` is an absolute path. Without either, search fails, and the other tools keep working.

```
~/.cache/abyme/
├── models/<model>/<revision>/           model files, checked before every load
└── index/<hash of the project root>/
    ├── lock                             lock for bringing the index up to date
    └── <model>-<revision>-r<recipe>/    LanceDB table of one project and model
```

- **Never in the repository:** the paths are built only from a hash of the project's root directory and from built-in model names, never from repository content. So a repository cannot plant index files that LanceDB would read.
- **Resetting is safe:** after a project's directory under `index/` is deleted, search rebuilds that index; after `~/.cache/abyme/` is deleted, it also downloads the model again.
- **No automatic cleanup:** indexes of moved or deleted projects and of models no longer used stay until they are deleted. A moved project builds a new index.

## LanceDB

The index is a LanceDB table per project and model. The crate `abyme-index` builds it (see [[Hexagonal Architecture#Crates]]).

**Only the MCP process owns the index.** Neither `apply` nor the core touches it. Before each `search`, the MCP process computes the text it would embed for every element and its [[Boards and Zones#Inheritance|statement kind]], and compares them with what is stored. It embeds only what changed and removes what is gone. Edits in the canvas, checkouts, and edits by hand can happen while no MCP process is running, so this catch-up is needed anyway. Comparing these rather than the element file also catches what does not live in the file, such as the statement kind, which comes from a board.

Table `elements`:

| Column | Type | Purpose |
|---|---|---|
| `id` | string | element ID from the [[Data Model]] |
| `text` | string | type, name, summary, and description: searched in full text, embedded, and compared to find changes |
| `statement` | string, or null | the element's statement kind: what a filtered search selects by, and compared to find changes |
| `vector` | list of float32 | embedding of `text` |

**Results come from the files.** The index only ranks. `search` shows the elements it found from the current element files, in the same lines as a [[MCP Server#Detail Levels|board outline]], never text from the index. A filtered search also checks the kind of each result against the files, and each line shows the kind from the files.

> [!question] Proposal
> The board path (e.g. “Shop › Payment › Stripe adapter”) becomes a column and a filter. A second table for board summaries is conceivable.

## Embedding Model

Each project chooses its model with the key `embedding_model` in `abyme/abyme.toml` (see [[Storage in Git#Project File]]). The models are built into Abyme:

| `embedding_model` | Model | Languages |
|---|---|---|
| `bge-small-en-v1.5` (default) | BGE small en v1.5, quantized | English |
| `paraphrase-multilingual-minilm-l12-v2` | paraphrase-multilingual-MiniLM-L12-v2, quantized | 50+ languages |

- **When it is downloaded:** when the MCP server starts in a project, it downloads the project's model in the background, unless the model is already in the cache. `search` waits for it. A changed key takes effect at the next search.
- **How it is checked:** Abyme downloads the files of a revision fixed in its source, from Hugging Face over HTTPS. Every file the model loads, the tokenizer files included, has a size and a SHA-256 built into Abyme. Abyme checks each file after the download and before every load, and never loads a file that does not match.
- **Network:** this download is the only network access at runtime. It sends no project content and no credentials; Hugging Face settings such as `HF_TOKEN` and `HF_HOME` do not apply. It trusts the system's root certificates and goes through the proxy named in `ALL_PROXY`, `HTTPS_PROXY`, or `HTTP_PROXY`, except for hosts in `NO_PROXY`.
- **ONNX Runtime**, which runs the model, comes prebuilt when Abyme is built, checked against the SHA-256 that the pinned `ort-sys` crate carries.
