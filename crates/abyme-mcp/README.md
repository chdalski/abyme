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

- **`apply`** takes `operations`, a list of `create_board`, `update_board`, `create_element`, `place_element`, `move_element`, `layout_board`, `update_element`, `delete_element`, `create_relation`, and `delete_relation` operations, and applies all of them or none. `create_element` and `update_element` take an optional `anchor`, the path relative to the project root that the element stands for, such as `crates/payment-service`. Every `update_element`, `create_relation`, and `delete_relation` removes the verification stamp of the element it changes, which no request can set. `create_board` takes a key, a title, and optionally a `summary`, an `owner`, the element whose inside (inner board) the board is, and a `zone`; `update_board` changes only the fields it names among `title`, `summary` (`null` removes it), `owner` (`null` removes it), and `zone` (`null` removes it), and the board keeps its ID. A zone is `{"mode": …, "statement": …}` with the mode `free` or `mirrored`, where a person cannot move elements in the canvas, and the statement kind `current`, `target`, `decision`, `idea`, `knowledge`, or `question`; the presets are the pairs Current (`mirrored`, `current`), Target (`free`, `target`), and Brainstorm (`free`, `idea`), and the board file stores the pair, not the preset's name. An element owns at most one board, and no board may lie inside itself (the board above a board is its owner's home); a request is refused only for such a breach that it brings in. `move_element` sets the position of an element on a board where it lies, with x and y from -1,000,000 to 1,000,000. An element a person moved in the canvas is pinned: a `move_element` without `pinned` is refused for it, `"pinned": true` moves and keeps or sets the pin, and `"pinned": false` moves and releases it; the AI sends either only after the person has said so. Without `pinned`, an unpinned element stays unpinned. `layout_board` takes `board` and arranges it as a layered diagram, so the AI need not compute coordinates: `uses`, `depends_on`, `describes`, and `concerns` put the source above the target, every unpinned box moves and gets the size of its text, a pinned box stays, and only the board file changes. It fails when a box would lie outside the range of `move_element`, or when the layouts of one request are too much work; [Placement and Layout](../../docs/4%20AI%20Interface/Placement%20and%20Layout.md#layered-layout) describes it. Semantic placement and a tool `place` are not built yet. `update_element` changes only the fields it names among `name`, `type_ref`, `summary` (`null` removes it), `body`, `anchor` (`null` removes it), and `statement` (`null` removes it); the element keeps its ID. It stores text exactly as sent, so the escapes that `read_board` and `read_element` show are not decoded. An element normally takes the statement kind of its home board's zone; `create_element` and `update_element` take an optional `statement`, one of the kinds above, as the exception, such as a question on a board of current facts. `delete_element` removes an element's file and its placement on every board, and is refused while other elements still relate to it, or while it owns a board; the error names each of those elements or the board, so a request can delete those relations or unlink the board first. `create_relation` adds a relation of a kind, such as `uses`, from one element to another, with an optional label; `delete_relation` removes it again. It answers with the ID each key received, such as `pay: element payment-service`. [Data Model](../../docs/5%20Architecture/Data%20Model.md#operations) describes the operations, [Relations](../../docs/2%20Concepts/Relations.md) the kinds.
- **`overview`** takes no argument and answers with all boards as a tree, the place to start a session: one line per board, each board's inner boards below it and indented two spaces further, with its ID, title, zone as `mode · statement kind` (or `no zone`, in a column that appears when a board is a zone), summary, how many elements with a file have their home on it, and how many of those have a task or only a hint, such as `⚠ 2 tasks, 1 hint`. A title longer than 80 characters and a summary longer than 160 are cut and end in `…`; `read_board` shows them in full. A board that a merge or a hand edit took out of the tree, because it lies inside itself or its owner has no file or no home, stands at the top level with its `Inside of:` line as `read_board` shows it. It lists at most 50 boards and then how many more there are. Text from the repository is escaped onto one line. When the project lies in no Git repository, `git` cannot be run, or Git fails, it starts with the line `Review flags unavailable:` and the reason, and counts no flags.
- **`read_board`** takes `board`, a board ID, and answers with the board's outline: a header with the board's ID and title; for an element's inner board, a line `Inside of:` with the element and `Above:` with the board it lies on, marked `⚠ missing` for an element without a file, `⚠ no home` for one that no board places, and `⚠ cycle` for a board that lies inside itself; for a zone, a line `Zone:` with its mode and statement kind; for a board with a summary, a line `Summary:` with it. Then, after a blank line, one line per element with its ID, type, statement kind, name, summary, `▸` and the IDs of the boards it owns (its inner boards), and last `⚠ task` or `⚠ hint` when the element has review flags, which `read_element` names, then, after a blank line, one line per relation between two elements on the board, such as `checkout → orders-db  uses  writes orders`. Text from the repository, relation labels included, is escaped onto one line. The statement kind column appears on a zone and wherever an element has a kind; it shows the element's own kind, that of its home board's zone, and `none` means it has none, not the kind of the board shown. With `"coordinates": true`, each element line also shows the element's position and size right after its ID, such as `@400,200 180x90`, and `pinned` after it for a pinned element. When the project lies in no Git repository, `git` cannot be run, or Git fails, `read_board` and `search` start with the line `Review flags unavailable:` and the reason, and mark no line.
- **`read_element`** takes `element`, an element ID, and answers with the element's detail: its ID and name, type, `Statement kind:` with the kind of its home board's zone and that board, or `none`, anchor (`Anchor: none` without one), verification stamp as the commit and `by ai` or `by person` (`Verified: none` without one), `Review:` with `task`, `hint`, or `none` and one indented line per review flag, such as the files changed under the anchor since the stamp (at most 10 named) or a relation where what the target's anchor stands for may have changed since the stamp, summary, the boards that place it, `Inner board:` with the boards it owns (`none` without one), its outgoing relations (a target without a file marked `⚠ missing`), the relations of other elements to it, and last its description. The description keeps its line breaks and tabs; other control and invisible characters in it appear as escapes such as `\u{202e}`. Every other value is escaped onto one line as in an outline. When the project lies in no Git repository, `git` cannot be run, or Git fails, the answer starts with the line `Review flags unavailable:` and the reason, and has no `Review:` lines.
- **`search`** takes `query`, words or a description of what to find, and answers with the 10 elements that match best, best first, in the same lines as an outline, statement kinds, inner boards, and markers included. It finds elements by the words they contain and by what they are about, also without a shared word. With `statement`, one of `current`, `target`, `decision`, `idea`, `knowledge`, or `question`, it answers only with elements of that kind, the best among them; an element without a kind matches none.
- **`status`** takes no argument and answers with the open review flags: a line counting the elements to review by their highest priority, then one line per element as in `search`, marked `⚠ task` or `⚠ hint`, those with a task first, at most 50 and then how many more there are; or `No open review flags.` When the project lies in no Git repository, `git` cannot be run, or Git fails, it answers with an error that starts with `Review flags unavailable:` and names the reason.
- **`verify`** takes `element`, an element ID, and stamps the element as verified by `ai` against the commit HEAD names in the repository the element's anchor lies in, a nested one such as a submodule or the project's own, after the AI has checked and updated it; it answers with that commit. Only the stamp changes, under the same lock as `apply`. It refuses an element without a file, a repository without a commit, an element with files under its anchor that differ from HEAD or are untracked (they must be committed first), an element whose anchor lies in a nested repository that Abyme cannot read, and any call when Git cannot be asked; nothing is written then. No other tool sets a stamp.

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
