---
id: "abyme"
type: "c4/system"
name: "Abyme"
summary: "A tool for developers to work visually with an AI on linked canvas boards stored in the project's own Git repository"
anchor: "Cargo.toml"
verified: "139a2cb2de465a30cc907ee26fc2d3993101a97b"
verified_by: "ai"
relations:
  - {kind: "uses", to: "hugging-face-hub"}
  - {kind: "uses", to: "project-repository"}
---
Abyme keeps project knowledge on boards in the project's Git repository: the developer works on them in the canvas app, the AI agent through the MCP server, both on the same files. It flags the elements whose code changed since they were last checked.

## How this model is cut

This is Abyme's own C4 model, which the AI builds and keeps current through Abyme's MCP server.

- **Context:** Abyme as one system, anchored at the workspace's `Cargo.toml`, with the developer, the AI agent, the project's Git repository, and the Hugging Face Hub, from which the search downloads its embedding model.
- **Containers:** the inside of Abyme. Each crate under `crates/` that ships in a binary is one container, anchored at the crate's directory; between containers there is one relation per direct non-dev dependency between the crates. The data stores are the project's files and the search cache.
- **Components:** the insides of `abyme-core` and `abyme-canvas`. Each module with a responsibility of its own is one component, anchored at its file or directory.
- **Relations** are all `uses`, so a task flag means that the element's own code changed, and a change passes only hints to what uses it.
- Elements that are not Abyme's own code, namely the developer, the AI agent, the outside systems, and the data stores, have no anchor and stay unstamped.

## What it leaves out

- `abyme-test-support`, which only tests use.
- The components of `abyme-model`, `abyme-index`, and `abyme-mcp`: their containers' anchors cover their code.
- Helper and entry-point modules, such as `slug.rs`, `lib.rs`, or `main.rs`: they stay with their container, whose anchor covers them.
- The half of a module that its component's anchor does not name, where a module has both `<name>.rs` and `<name>/`: an anchor is one path, and the container's anchor covers the rest.
- Tests, the docs in `docs/`, the plans, and the development environment.