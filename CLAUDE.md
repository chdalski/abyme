# Abyme

Abyme is a tool for developers to work visually with an AI. Project
knowledge lives on linked canvas boards stored in the project's own Git
repository; the developer works in a canvas app, the AI through an MCP
server, both on the same data. Abyme keeps that knowledge honest: it
detects when boards no longer match the code and flags dependent content.
The concept lives in `docs/`. The Working Skeleton is being built: its
core comes first, then search and the canvas.

## Build and Test

```sh
cargo build --workspace
cargo test --workspace
cargo test --workspace <filter>          # only the tests whose name contains <filter>; see Conventions
cargo clippy --workspace --all-targets   # zero warnings: the workspace lints deny them
cargo fmt --all                          # --check to verify only
cargo clean
```

## Components

| Path | Purpose |
|---|---|
| `crates/abyme-model` | The Abyme format: identifiers and the files that hold elements and boards |
| `crates/abyme-core` | Operations on elements and boards: `apply` turns a request into the files to write, `Project` finds a project, initializes it, and reads and writes its files, and `outline` renders a board as text for the AI |
| `crates/abyme-index` | The search index: keeps a LanceDB table per project and model in the user's cache and ranks a project's elements for a query by full text and vectors; downloads a built-in embedding model's pinned files into that cache, checks them against the sizes and hashes in the source, and loads them with fastembed |
| `crates/abyme-mcp` | The `abyme-mcp` binary: `abyme-mcp init` makes a directory an Abyme project; without arguments it serves the tools `apply`, `read_board`, and `search` over MCP on stdio and downloads the project's embedding model in the background |

## Language

Project language: English. Everything written to the
repository — code, comments, docs, tests, commit messages,
plans — is in English, regardless of the language the
user chats in. Chat replies may use the user's language.

Format tokens stay verbatim in every language: Conventional
Commit types (`feat`, `fix`, ...), plan header fields,
status values, and section headings, and programming-language
keywords.

## Conventions

<!-- Agents: add non-obvious project conventions discovered during work — things a future agent would need to know to avoid mistakes. One line each. Remove when no longer true. -->

- Write code only when explicitly asked. The Working Skeleton is being built as defined in `docs/5 Architecture/First Slice.md`: its core first, then search and the canvas, each in its own plan under `.ai/plans/`.
- `docs/` is an Obsidian vault whose root is the project root; its rules are in `docs/README.md` ("Rules"): docs describe the current state, not its history; proposals go in `> [!question] Proposal`, open points in `> [!todo] Open`; decisions are a table in `docs/Decisions.md`; links are wikilinks.
- Render-check every new Mermaid diagram before adding it to `docs/`: `npx -y @mermaid-js/mermaid-cli@12.0.0 -q -i "<note>.md" -o "<scratch dir>/<note>.md"` exits non-zero on a broken diagram; it needs headless Chrome's libraries (`libnss3` and others), which the devcontainer image does not install yet.
- The dev profile, which tests use too, keeps line tables for the workspace's crates and no debug info for dependencies (root `Cargo.toml`), so linking the test binaries stays light. For one build with full debug info, run `cargo --config 'profile.dev.debug=true' --config 'profile.dev.package."*".debug=true' build`, which rebuilds everything.
- Builds and tests stay on the workspace selection, whose artifacts stay cached; narrow a test run by test, not by package: `cargo test --workspace <filter>`, or `--test <file>` for one integration test file. A run with `-p` resolves features for its own packages and compiles the dependencies a second time: without `abyme-mcp`, `serde` lacks `rc`, `indexmap` lacks `serde`, and the `json-schema` feature of `abyme-model` and `abyme-core`, off by default and turned on only by `abyme-mcp`, goes unchecked.
- Crates inherit the workspace lint set with `[lints] workspace = true`; the set comes from `.claude/skills/project-init/rust-init.md`, and `clippy.toml` relaxes it only for tests.
- Comparisons with other tools stay in the chat; `docs/` takes only the adopted ideas, without naming the tool.
- `docs/Glossary.md` defines the terms that docs, UI, and code share; add new terms there.
- Decided stack: a Rust Cargo workspace with crates under `crates/`; the canvas is a Tauri app with React Flow, its UI in TypeScript.
- License MIT: every dependency must be MIT-compatible.
- `.devcontainer/` provides the toolchains: Rust (linked with mold), protoc with the protobuf well-known types for LanceDB's build, WebKitGTK and the Tauri CLI (`cargo tauri`) for the canvas, Node with pnpm, uv, and Go.

## References

<!-- Agents: add authoritative sources used to make implementation decisions. One line each. -->

- React Flow, the canvas library: https://reactflow.dev/
- Tauri v2, the canvas app shell: https://v2.tauri.app/
- Model Context Protocol specification, the AI interface: https://modelcontextprotocol.io/specification/latest
- JSON Canvas 1.0, model for the board file format: https://jsoncanvas.org/spec/1.0/
- C4 model, the first use case and its nested levels: https://c4model.com/
- Structurizr implied relationships, model for relations raised to parent boards: https://docs.structurizr.com/dsl/implied-relationships
- Miro MCP tools, model for AI-side placement on a canvas: https://developers.miro.com/docs/miro-mcp-tools
