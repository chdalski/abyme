---
tags:
  - architecture
aliases:
  - Storage
  - File Format
  - File Structure
---
# Storage in Git

All Abyme data lies as files in Git, in the same repo as the code. Derived data such as the search index is not in Git.

The folders are `abyme/`, in Git, and `.abyme/`, which Git ignores. Elements are Markdown files with YAML frontmatter, boards are JSON files.

> [!question] Proposal
> The file names inside the folders, and the fields that come after the Working Skeleton, are a draft.

```
project/
├── abyme/                             Abyme format, in Git
│   ├── abyme.toml                     format version, enabled type packs
│   ├── elements/
│   │   ├── payment-service.md         one element per file
│   │   └── bottleneck-black-friday.md
│   ├── boards/
│   │   ├── context.board.json         one board per file: zones, frames, placements
│   │   └── container-level.board.json
│   └── requests/
│       └── delete-legacy-mailer.yaml  change requests
├── .abyme/                            not in Git
│   ├── .gitignore                     contains `*`, so Git ignores the folder
│   ├── lock                           lock for writing
│   ├── tmp/                           files being written, before their rename
│   └── cache/                         search index and render cache
├── src/ …
└── …
```

`.abyme/` ignores itself through its own `.gitignore`, so Abyme never edits the project's `.gitignore`.

## Consequences

- **Board and code in the same commit:** Changes to boards land in the same commit and pull request as the matching code change. Reviews see both. That is why the folder is visible and not hidden.
- **Every branch has its own current state.** On a feature branch, the Current board shows the state of that branch.
- **Delete instead of archive.** What is no longer true is removed. History comes from Git.
- **The base snapshot** for [[Reconciliation#Direction of Changes|detecting the direction]] comes from the Git history.
- **Collaboration runs through commits, branches, and pull requests**, not in real time. Merge conflicts in boards are resolved as in code.
- **The data is readable without Abyme too**, for people and for an AI without MCP.

## Format Requirements

- **Text and human-readable**
- **Diff-friendly:** stable IDs, a stable order, integer coordinates
- **Content and layout separated:** Moving a box does not change the file with the content.
- **Merge-friendly:** many small files instead of one large one, so conflicts stay local
- **Versioned:** The format has a version number, and there are migrations.

## Project

A directory becomes an Abyme project through `abyme-mcp init`, run in the directory that is to become the project's root. Init creates `abyme/abyme.toml` and `.abyme/`; inside an existing project it changes nothing and names the project's root. Writing never creates a project.

- **Finding the root:** Abyme uses the nearest directory, from the working directory upwards, that holds `abyme/abyme.toml`. Git repositories nested in the project, such as clones and submodules, do not stop the search.
- **Owner:** The project is used only when its root directory, `abyme/`, and `abyme/abyme.toml` belong to the user running Abyme. Otherwise every call fails, and Abyme does not look further up, so a project file that another user places above the working directory, for example in `/tmp`, is never used. The check exists for Unix only, so Abyme builds only there for now.
- **Fresh clone:** Git does not carry `.abyme/`. Reading does not need it, and the first write creates it with its `.gitignore`. A missing `abyme/elements/` or `abyme/boards/` counts as empty.

## Reading

- **Only Abyme files:** In `abyme/elements/` and `abyme/boards/`, Abyme reads only regular files named `<id>.md` or `<id>.board.json` with a valid ID. Everything else, such as editor backups, folders, and symbolic links, is ignored.
- **No symbolic links at fixed paths:** If `abyme/`, `abyme/abyme.toml`, `abyme/elements/`, `abyme/boards/`, `.abyme/`, `.abyme/tmp/`, or `.abyme/lock` is a symbolic link, every call that uses it fails, so a cloned repository cannot redirect reads, writes, or deletions outside the project.
- **Limits:** Element and board files are at most 1 MiB, the project file at most 64 KiB. A larger file makes every call fail, and a request that would write one is rejected.

## Writing

In [[Hexagonal Architecture#Runtime|stage A]], the canvas and the MCP server write the same files from two processes.

- **Under a lock:** Every [[Data Model#Operations|operation]] is applied to the current files while the writer holds an advisory lock on `.abyme/lock`. A change therefore never overwrites one it has not seen.
- **Atomically:** Every file is written to a temporary file in `.abyme/tmp/` and then renamed, so a file watcher never sees a half-written file. Before it writes, a writer removes the files that a crashed writer left there; it holds the lock, so no other writer's file is among them.
- **In order:** New element files come first, then each new or changed board, so a board never places an element whose file is missing. A write that fails between two renames can leave an element without a home, which the [[Data Model#Rules|data model]] allows.
- **No model in memory:** The MCP server reads the files on every call. The canvas holds only the board it shows and reloads it on file-watcher events, its own writes included.
- **Outside the lock:** An editor or `git checkout` does not take the lock. The file watchers pick up such changes as well.

## Element File

An element is a Markdown file with a YAML frontmatter. The description follows below as Markdown. A change to the description is a normal text diff in the pull request. Since the vault lies in the project root, Obsidian shows the elements as notes.

```markdown
---
id: "payment-service"
type: "c4/container"
name: "Payment Service"
summary: "Payment processing through Stripe"
---
Handles payments through Stripe. Refunds run asynchronously
through the refund-worker.
```

- **Fields:** `id`, which equals the file name, `type`, which is never empty, `name`, and the optional `summary`. Other keys are rejected.
- **Writing:** Abyme writes every value double-quoted. It escapes backslash, double quote, tab, line feed, carriage return, U+0085, U+2028, U+2029, and U+FEFF and keeps every other character as it is. Values written by hand may use any YAML scalar style.
- **Limits:** `type`, `name`, and `summary` hold only characters YAML counts as printable: no control characters other than tab, line feed, carriage return, and U+0085, and neither U+FFFE nor U+FFFF. The frontmatter is at most 64 KiB, counted as Abyme writes it.
- **Strict YAML:** The frontmatter is one flat mapping. Aliases, anchors, tags, directives, and document markers are rejected, so the file means the same to every reader.
- **The body** below the closing `---` line is kept byte for byte.
- **Later fields:** `attributes`, `anchor`, `statement`, `verified`, `inner_board`, `expected_anchor`, and `relations` from the [[Data Model#Types|data model]] come with the slices that need them.

## Board File

A board is a JSON file. Its structure follows JSON Canvas, so an export to `.canvas` for Obsidian would be easy.

```json
{
  "format": 1,
  "id": "container-level",
  "title": "Container Level",
  "placements": [
    {"element": "checkout-service", "x": 80, "y": 200, "width": 180, "height": 90},
    {"element": "payment-service", "x": 400, "y": 200, "width": 180, "height": 90}
  ]
}
```

- **Fields:** `format`, which is 1, `id`, which equals the file name, `title`, and the placements with `element`, `x`, `y`, `width`, and `height`. Positions are integers, sizes positive integers. The file must be a JSON object; other fields and an element placed twice are rejected.
- **One placement per line**, sorted by element ID. Moving a box changes exactly one line.
- **Arrows are not in the file.** They follow from the relations of the elements.
- **Later fields:** `owner`, `zone`, and `frames` on the board; `frame`, `presentation`, `pinned`, and `attached_to` on a placement. An attached sticky note's coordinates will be relative to the element it hangs on (see [[Presentation#Sticky Notes]]).

## Project File

```toml
# abyme/abyme.toml
format = 1
```

`type_packs`, the enabled [[Extensions#Type Packs|type packs]], comes later.

## What Is Not in Git

- **Derived data** that can be regenerated at any time: search index and render caches
- **Credentials**, in case fetchers come later
- **Values that change continuously.** Such sources are [[Ideas#Deferred|deferred]].

> [!todo] Open
> - Where do hints for arrow routing go, in case someone bends an arrow by hand?
> - **Several repos:** How do projects work that consist of several repos? One option is a separate Abyme repo that points to the others.
