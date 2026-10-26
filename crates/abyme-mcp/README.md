# abyme-mcp

The MCP server of Abyme. Through it, an AI such as Claude Code builds boards and elements in your repository, reads them back, and searches them.

It runs on Linux and macOS.

## Install

From the root of the Abyme repository:

```sh
cargo install --locked --path crates/abyme-mcp
```

This puts `abyme-mcp` into `~/.cargo/bin`, which must be on your `PATH`. `--locked` builds with the dependency versions in `Cargo.lock`, the ones the project has checked.

## Make a repository an Abyme project

In the repository's root directory:

```sh
abyme-mcp init
```

This creates `abyme/abyme.toml`, which marks the project, and `.abyme/`, which Git ignores. Commit `abyme/`. Running `init` again, or anywhere inside the project, changes nothing and names the project's root.

## Register the server with Claude Code

In the repository's root directory:

```sh
claude mcp add abyme -- abyme-mcp
```

Claude Code starts the server in the directory it runs in, and the server serves the project that directory lies in. Without `--scope`, the registration holds for this directory only. With `--scope user`, it holds in every directory. In a directory outside any project, every tool answers that the repository needs `abyme-mcp init` first; the server never creates a project itself.

## Tools

- **`apply`** takes `operations`, a list of `create_board`, `create_element`, `place_element`, `move_element`, `update_element`, `delete_element`, `create_relation`, and `delete_relation` operations, and applies all of them or none. `create_element` and `update_element` take an optional `anchor`, the path relative to the project root that the element stands for, such as `crates/payment-service`. Every `update_element`, `create_relation`, and `delete_relation` removes the verification stamp of the element it changes, which no request can set. `move_element` sets the position of an element on a board where it lies, with x and y from -1,000,000 to 1,000,000. `update_element` changes only the fields it names among `name`, `type_ref`, `summary` (`null` removes it), `body`, and `anchor` (`null` removes it); the element keeps its ID. It stores text exactly as sent, so the escapes that `read_board` and `read_element` show are not decoded. `delete_element` removes an element's file and its placement on every board, and is refused while other elements still relate to it; the error names each of them, so a request can delete those relations first. `create_relation` adds a relation of a kind, such as `uses`, from one element to another, with an optional label; `delete_relation` removes it again. It answers with the ID each key received, such as `pay: element payment-service`. [Data Model](../../docs/5%20Architecture/Data%20Model.md#operations) describes the operations, [Relations](../../docs/2%20Concepts/Relations.md) the kinds.
- **`read_board`** takes `board`, a board ID, and answers with the board's outline: one line per element with its ID, type, name, and summary, then, after a blank line, one line per relation between two elements on the board, such as `checkout → orders-db  uses  writes orders`. Text from the repository, relation labels included, is escaped onto one line. With `"coordinates": true`, each element line also shows the element's position and size right after its ID, such as `@400,200 180x90`.
- **`read_element`** takes `element`, an element ID, and answers with the element's detail: its ID and name, type, anchor (`Anchor: none` without one), verification stamp as the commit and `by ai` or `by person` (`Verified: none` without one), summary, the boards that place it, its outgoing relations (a target without a file marked `⚠ missing`), the relations of other elements to it, and last its description. The description keeps its line breaks and tabs; other control and invisible characters in it appear as escapes such as `\u{202e}`. Every other value is escaped onto one line as in an outline.
- **`search`** takes `query`, words or a description of what to find, and answers with the 10 elements that match best, best first, in the same lines as an outline. It finds elements by the words they contain and by what they are about, also without a shared word.

A request that does not fit a tool, or that the project rejects, comes back as a tool error that says why.

## The embedding model

`search` needs an embedding model, which the server downloads from Hugging Face when it starts in a project, in the background. The first `search` waits for the download if it has not finished. If the download fails, `search` answers with the error and tries again at the next call.

Each project chooses its model with the key `embedding_model` in `abyme/abyme.toml`:

```toml
format = 1
embedding_model = "paraphrase-multilingual-minilm-l12-v2"
```

| `embedding_model` | For | Download |
|---|---|---|
| `bge-small-en-v1.5` (default, also without the key) | English | about 70 MB |
| `paraphrase-multilingual-minilm-l12-v2` | 50+ languages; finds English text through a German query and the other way round | about 250 MB |

Any other value makes every tool fail with an error that names `abyme/abyme.toml`. A changed key takes effect at the next `search`, which then loads the other model and builds its own index.

The model and the search index live in your cache, `$XDG_CACHE_HOME/abyme/` or, without that, `~/.cache/abyme/`, never in the repository. Deleting that directory is safe: the server downloads the model and rebuilds the index again. Without `XDG_CACHE_HOME` and `HOME`, `search` answers with an error, and the other tools keep working. [Search and Index](../../docs/4%20AI%20Interface/Search%20and%20Index.md) describes how the model is checked and which network settings apply.
