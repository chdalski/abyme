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
> The file names inside the folders and the fields in the examples below are a draft.

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

## Writing

In [[Hexagonal Architecture#Runtime|stage A]], the canvas and the MCP server write the same files from two processes.

- **Under a lock:** Every [[Data Model#Operations|operation]] is applied to the current files while the writer holds an advisory lock on `.abyme/lock`. A change therefore never overwrites one it has not seen.
- **Atomically:** Every file is written to a temporary file and then renamed, so a file watcher never sees a half-written file.
- **No model in memory:** The MCP server reads the files on every call. The canvas holds only the board it shows and reloads it on file-watcher events, its own writes included.
- **Outside the lock:** An editor or `git checkout` does not take the lock. The file watchers pick up such changes as well.

## Element File

An element is a Markdown file with frontmatter. The description follows below as Markdown. A change to the description is a normal text diff in the pull request. Since the vault lies in the project root, Obsidian shows the elements as notes.

```markdown
---
id: payment-service
type: c4/container
name: Payment Service
summary: Payment processing through Stripe
attributes:
  technology: Go
anchor: file://crates/payment-service/
verified:
  commit: 3f2a9c1
  by: ai
inner_board: payment-components
relations:
  - { kind: uses, to: stripe-adapter, label: "HTTPS" }
  - { kind: depends_on, to: orders-db }
---
Handles payments through Stripe. Refunds run asynchronously
through the refund-worker.
```

> [!todo] Open
> **YAML library:** Which library reads and writes the frontmatter?

## Board File

A board is a JSON file. Its structure follows JSON Canvas, so an export to `.canvas` for Obsidian would be easy.

```json
{
  "format": 1,
  "id": "container-level",
  "title": "Container Level",
  "owner": "shop-system",
  "zone": { "preset": "current", "mode": "mirrored" },
  "frames": [
    { "id": "current", "title": "Current", "x": 0, "y": 0, "width": 900, "height": 600 }
  ],
  "placements": [
    { "element": "bottleneck-black-friday", "x": 420, "y": 320, "width": 160, "height": 80, "presentation": "note" },
    { "element": "checkout-service", "frame": "current", "x": 80, "y": 200, "width": 180, "height": 90, "presentation": "box" },
    { "element": "payment-service", "frame": "current", "x": 400, "y": 200, "width": 180, "height": 90, "presentation": "box", "pinned": true },
    { "element": "question-retries", "frame": "current", "attached_to": "payment-service", "x": 120, "y": -40, "width": 140, "height": 70, "presentation": "note" }
  ]
}
```

- **One placement per line**, sorted by element ID. Moving a box changes exactly one line.
- **Attached:** The note `question-retries` hangs on `payment-service`; its coordinates are relative to that element (see [[Presentation#Sticky Notes]]).
- **Arrows are not in the file.** They follow from the relations of the elements.

## Project File

```toml
# abyme/abyme.toml
format = 1
type_packs = ["c4", "adr"]
```

## What Is Not in Git

- **Derived data** that can be regenerated at any time: search index and render caches
- **Credentials**, in case fetchers come later
- **Values that change continuously.** Such sources are [[Ideas#Deferred|deferred]].

> [!todo] Open
> - Where do hints for arrow routing go, in case someone bends an arrow by hand?
> - **Several repos:** How do projects work that consist of several repos? One option is a separate Abyme repo that points to the others.
