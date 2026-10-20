# Abyme

Abyme is a tool for developers to work visually with an AI. Project
knowledge lives on linked canvas boards stored in the project's own Git
repository; the developer works in a canvas app, the AI through an MCP
server, both on the same data. Abyme keeps that knowledge honest: it
detects when boards no longer match the code and flags dependent content.
The project is in its concept phase — the concept lives in `docs/`, and
there is no code yet.

## Build and Test

```sh
# No code yet — no build, test, lint, or format commands.
# Re-run /project-init once the first Cargo.toml or package.json exists.
```

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

- Concept phase: there is no code yet; write prototypes or code only when explicitly asked — the Working Skeleton and MVP get defined first.
- `docs/` is an Obsidian vault whose root is the project root; its rules are in `docs/README.md` ("Regeln"): docs describe the current state, not its history; proposals go in `> [!question] Vorschlag`, open points in `> [!todo] Offen`; decisions are a table in `docs/Entscheidungen.md`; links are wikilinks.
- Render-check every new Mermaid diagram before adding it to `docs/`.
- Comparisons with other tools stay in the chat; `docs/` takes only the adopted ideas, without naming the tool.
- `docs/Glossar.md` maps each German term (docs, UI) to its English term (code); add new terms there.
- Decided stack: a Rust Cargo workspace with crates under `crates/`; the canvas is a Tauri app with React Flow, its UI in TypeScript.
- License MIT: every dependency must be MIT-compatible.
- `.devcontainer/` provides the toolchains: Rust (linked with mold), Node with pnpm, uv, and Go.

## References

<!-- Agents: add authoritative sources used to make implementation decisions. One line each. -->

- React Flow, the canvas library: https://reactflow.dev/
- Tauri v2, the canvas app shell: https://v2.tauri.app/
- Model Context Protocol specification, the AI interface: https://modelcontextprotocol.io/specification/latest
- JSON Canvas 1.0, model for the board file format: https://jsoncanvas.org/spec/1.0/
- C4 model, the first use case and its nested levels: https://c4model.com/
- Structurizr implied relationships, model for relations raised to parent boards: https://docs.structurizr.com/dsl/implied-relationships
- Miro MCP tools, model for AI-side placement on a canvas: https://developers.miro.com/docs/miro-mcp-tools
