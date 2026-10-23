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

- **`apply`** takes `operations`, a list of `create_board`, `create_element`, and `place_element` operations, and applies all of them or none. It answers with the ID each key received, such as `pay: element payment-service`. [Data Model](../../docs/5%20Architecture/Data%20Model.md#operations) describes the operations.
- **`read_board`** takes `board`, a board ID, and answers with the board's outline: one line per element with its ID, type, name, and summary.
- **`search`** takes `query`, words or a description of what to find, and answers with the 10 elements that match best, best first, in the same lines as an outline. It finds elements by the words they contain and by what they are about, also without a shared word.

A request that does not fit a tool, or that the project rejects, comes back as a tool error that says why.

## The embedding model

`search` needs an embedding model, which the server downloads from Hugging Face (about 70 MB) when it starts in a project, in the background. The first `search` waits for the download if it has not finished. If the download fails, `search` answers with the error and tries again at the next call.

The model and the search index live in your cache, `$XDG_CACHE_HOME/abyme/` or, without that, `~/.cache/abyme/`, never in the repository. Deleting that directory is safe: the server downloads the model and rebuilds the index again. Without `XDG_CACHE_HOME` and `HOME`, `search` answers with an error, and the other tools keep working. [Search and Index](../../docs/4%20AI%20Interface/Search%20and%20Index.md) describes how the model is checked and which network settings apply.
