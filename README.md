# Abyme

> *Mise en abyme*: the picture within the picture. Boards you can zoom into, and inside them more boards.

Abyme is a tool for developers to work visually with an AI. Project knowledge lives on linked canvas boards stored in the project's own Git repository. The developer works in a canvas app, the AI through an MCP server, both on the same data. Abyme keeps that knowledge honest: it detects when boards no longer match the code and flags dependent content.

## Status

Concept phase: there is no code yet. The concept lives in `docs/`; start at [docs/README.md](docs/README.md). To browse it with working wikilinks, open the project root as an Obsidian vault.

## Stack

- Core: Rust Cargo workspace
- Canvas: Tauri app with React Flow, UI in TypeScript
- AI interface: MCP server

The dev container in `.devcontainer/` provides the toolchains.

## License

[MIT](LICENSE)
