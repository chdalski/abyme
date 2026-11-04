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
cargo deny --locked check                # advisories, licenses, sources, and bans of the Rust dependencies; see Conventions
pnpm --dir crates/abyme-canvas/ui install --frozen-lockfile   # the canvas UI's packages; none runs a build script
pnpm --dir crates/abyme-canvas/ui check   # TypeScript, ESLint, and Prettier; `format` rewrites
pnpm --dir crates/abyme-canvas/ui test    # Vitest
pnpm --dir crates/abyme-canvas/ui dev     # the UI's dev server, which the canvas loads; see Conventions to run it
```

## Components

| Path | Purpose |
|---|---|
| `crates/abyme-model` | The Abyme format: identifiers and the files that hold elements and boards |
| `crates/abyme-core` | Operations on elements and boards: `apply` turns a request into the files to write, placing boxes by free space or laying a board out as a layered diagram, `Project` finds a project, initializes it, reads and writes its files, and asks Git through the `git` command line what changed under each anchor, from which `review` computes the review flags, and `overview`, `outline`, and `element_detail` render all boards as a tree, a board, and an element as text for the AI |
| `crates/abyme-index` | The search index: keeps a LanceDB table per project and model in the user's cache and ranks a project's elements for a query by full text and vectors; downloads a built-in embedding model's pinned files into that cache, checks them against the sizes and hashes in the source, and loads them with fastembed |
| `crates/abyme-canvas` | The `abyme-canvas` binary, a Tauri app: started in a project, it lists the project's boards and shows the one the user opens as boxes on a React Flow canvas, with arrows for its relations, a button per inner board, a breadcrumb of the boards above it, a marker on pinned boxes, and a mark for the level of an element's review flag, computed through the core's Git when the canvas opens, when the files change, when its window gets focus or turns visible, and at a fixed interval while it is visible; it writes a box the user drags, the release of a pin, and creating, editing, deleting, attaching, and detaching a sticky note through `apply`, where `src/admission.rs` decides what it may write, on a mirrored board only what concerns a note on which nothing hangs; an attached note is a child node of its box, and it asks again whenever a file watcher sees the project's files change; its UI, React with Vite, lies in `ui/` |
| `crates/abyme-mcp` | The `abyme-mcp` binary: `abyme-mcp init` makes a directory an Abyme project; without arguments it serves the tools `apply`, `overview`, `read_board`, `read_element`, `search`, `status`, and `verify` over MCP on stdio and downloads the project's embedding model in the background |
| `crates/abyme-test-support` | Helpers for tests that run Git in temporary repositories, shared by the tests of `abyme-core`, `abyme-mcp`, and `abyme-canvas`; it depends on no crate of the workspace |

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
- Builds and tests stay on the workspace selection, whose artifacts stay cached; narrow a test run by test, not by package: `cargo test --workspace <filter>`, or `--test <file>` for one integration test file. A run with `-p` resolves features for its own packages and compiles the dependencies a second time: without `abyme-mcp`, `serde` lacks `rc`, `indexmap` lacks `serde`, and the `json-schema` feature of `abyme-model` and `abyme-core`, off by default and turned on only by `abyme-mcp`, goes unchecked. So do the `ts` feature of `abyme-model`, which only the tests of `abyme-canvas` turn on, and its `fixtures` feature, a builder of elements that only the tests of other crates turn on: `cargo clippy --workspace --all-targets` and `cargo test --workspace` check them, `cargo build --workspace` compiles neither.
- In a devcontainer with a memory limit (`DEVCONTAINER_MEMORY`, which leaves no swap), cargo takes its number of parallel jobs from the environment variable `CARGO_BUILD_JOBS`, set in `.devcontainer/.env` and in effect after Rebuild Container (see `.devcontainer/README.md`, "Resource limits"); one job per CPU links enough test binaries at once to restart the container. If `printenv CARGO_BUILD_JOBS` prints nothing, pass `-j 3` to every cargo command that compiles (`build`, `test`, `clippy`) and ask the user to set the variable in `.devcontainer/.env` and rebuild the container.
- Run the canvas in a project: start `pnpm --dir crates/abyme-canvas/ui dev`, then run this repository's `target/debug/abyme-canvas` from the project's directory, after `cargo build --workspace`. A debug build loads its UI from that dev server; `cargo tauri dev` in `crates/abyme-canvas` starts both but runs the app in the crate's directory, which lies in no project.
- `crates/abyme-canvas/ui/src/bindings.ts` holds the TypeScript types that ts-rs generates from the Rust types the UI receives. Do not edit it: when the test `committed_bindings_match_the_rust_types` fails, replace the file with the text the test prints.
- Crates inherit the workspace lint set with `[lints] workspace = true`; the set comes from `.claude/skills/project-init/rust-init.md`, and `clippy.toml` relaxes it for tests and disallows reading a placement's stored `rect` and `attached_to` outside `abyme-model`: positions are read through `Board::rect_of` or `Board::placed` and written through `Board::set_rect`, and a test module that reads the stored fields carries `#![expect(clippy::disallowed_fields, reason = "...")]`.
- Comparisons with other tools stay in the chat; `docs/` takes only the adopted ideas, without naming the tool.
- `docs/Glossary.md` defines the terms that docs, UI, and code share; add new terms there.
- Decided stack: a Rust Cargo workspace with crates under `crates/`; the canvas is a Tauri app with React Flow, its UI in TypeScript.
- License MIT: every dependency must be MIT-compatible.
- `cargo deny --locked check` checks the Rust dependencies against `deny.toml`, from the repository root: RustSec advisories (it fetches the advisory database on each run, so it needs network), licenses, sources, and banned crates and features. A finding names its check, crate, and advisory; entries in `deny.toml` that let a dependency pass are added only when the user decides.
- `.devcontainer/` provides the toolchains: Rust (linked with mold), protoc with the protobuf well-known types for LanceDB's build, WebKitGTK and the Tauri CLI (`cargo tauri`) for the canvas, cargo-deny for the dependency check, Node with pnpm, uv, and Go.
- Bash tool calls run in zsh (`/usr/bin/zsh`), although the session's environment names fish, the devcontainer's login shell (`ENV SHELL` in `.devcontainer/Dockerfile`): write POSIX shell syntax, such as `S=dir`, never fish's `set S dir`.

## References

<!-- Agents: add authoritative sources used to make implementation decisions. One line each. -->

- React Flow, the canvas library: https://reactflow.dev/
- Tauri v2, the canvas app shell: https://v2.tauri.app/
- Model Context Protocol specification, the AI interface: https://modelcontextprotocol.io/specification/latest
- JSON Canvas 1.0, model for the board file format: https://jsoncanvas.org/spec/1.0/
- C4 model, the first use case and its nested levels: https://c4model.com/
- Structurizr implied relationships, model for relations raised to parent boards: https://docs.structurizr.com/dsl/implied-relationships
- Miro MCP tools, model for AI-side placement on a canvas: https://developers.miro.com/docs/miro-mcp-tools
