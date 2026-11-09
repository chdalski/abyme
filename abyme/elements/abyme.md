---
id: "abyme"
type: "c4/system"
name: "Abyme"
summary: "A tool for developers to work visually with an AI on linked canvas boards stored in the project's own Git repository"
anchor: "Cargo.toml"
verified: "c1c7662c5b5d1e5cfa42b94b737e9d5f3c7461df"
verified_by: "ai"
verified_content:
  - {path: "Cargo.toml", object: "a6b1a9a191148b516509f10d9552f71523b41b99"}
relations:
  - {kind: "uses", to: "hugging-face-hub"}
  - {kind: "uses", to: "project-repository"}
---
Abyme keeps project knowledge on boards in the project's Git repository: the developer works on them in the canvas app, the AI agent through the MCP server, both on the same files. It flags the elements whose code changed since they were last checked.

## How this model is cut

This is Abyme's own model, which the AI builds and keeps current through Abyme's MCP server: a C4 model of the code, and a docs map of the notes in `docs/` that relates each note to the code it describes.

### C4 model

- **Context:** Abyme as one system, anchored at the workspace's `Cargo.toml`, with the developer, the AI agent, the project's Git repository, and the Hugging Face Hub, from which the search downloads its embedding model.
- **Containers:** the inside of Abyme. Each crate under `crates/` that ships in a binary is one container, anchored at the crate's directory; between containers there is one relation per direct non-dev dependency between the crates. The data stores are the project's files and the search cache.
- **Components:** the insides of `abyme-core` and `abyme-canvas`. Each module with a responsibility of its own is one component, anchored at its file or directory.
- **Relations** between C4 elements are all `uses`, so a task flag on a C4 element means that its own code changed, and a change passes only hints to what uses it.
- Elements that are not Abyme's own code, namely the developer, the AI agent, the outside systems, and the data stores, have no anchor and stay unstamped.

### Docs map

- **Notes:** each Markdown note in the numbered folders of `docs/` is one element of type `docs/note`, anchored at its file, on the board of its folder. Each folder is one element of type `docs/folder`, anchored at the folder, on the board `docs`, and owns its folder's board.
- **Relations** `describes` go from a note to the finest C4 element that covers built code the note describes: a component of `abyme-core` or `abyme-canvas` where one covers that code, otherwise the container, such as for a crate without components or a module that is no component, and never `abyme`. A note about the set of crates relates to each container it covers. A sentence about built code is enough for a relation, also in a note that otherwise describes unbuilt parts; a note about nothing built or about no code has none.
- A relation `describes` passes a task, so a code change flags each note that describes that code. Notes have no relations to each other, and folders have none.

## What it leaves out

- `abyme-test-support`, which only tests use.
- The components of `abyme-model`, `abyme-index`, and `abyme-mcp`: their containers' anchors cover their code.
- Helper and entry-point modules, such as `slug.rs`, `lib.rs`, or `main.rs`: they stay with their container, whose anchor covers them.
- The half of a module that its component's anchor does not name, where a module has both `<name>.rs` and `<name>/`: an anchor is one path, and the container's anchor covers the rest.
- The Markdown at the root of `docs/`, such as `README.md`, `Decisions.md`, and `Glossary.md`, which describes no code, and Markdown outside `docs/`.
- Tests, the plans, and the development environment.