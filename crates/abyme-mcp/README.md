# abyme-mcp

The MCP server of Abyme. Through it, an AI such as Claude Code builds boards and elements in your repository and reads them back.

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

Claude Code starts the server in the directory it runs in, and the server serves the project that directory lies in. Without `--scope`, the registration holds for this directory only. With `--scope user`, it holds in every directory. In a directory outside any project, both tools answer that the repository needs `abyme-mcp init` first; the server never creates a project itself.

## Tools

- **`apply`** takes `operations`, a list of `create_board`, `create_element`, and `place_element` operations, and applies all of them or none. It answers with the ID each key received, such as `pay: element payment-service`. [Data Model](../../docs/5%20Architecture/Data%20Model.md#operations) describes the operations.
- **`read_board`** takes `board`, a board ID, and answers with the board's outline: one line per element with its ID, type, name, and summary.

A request that does not fit a tool, or that the project rejects, comes back as a tool error that says why.
