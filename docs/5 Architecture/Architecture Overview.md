---
tags:
  - architecture
aliases:
  - Architecture
  - Overview
---
# Architecture Overview

Two levels, the way Abyme itself is meant to show C4 levels as inner boards: first Abyme in its surroundings, then zoomed into its building blocks. Dashed borders are proposals, dotted ones come later.

## Level 1 · Context

```mermaid
flowchart TB
    dev(["Developer"])
    ai(["AI agent<br>e.g. Claude Code"])
    abyme["<b>Abyme</b><br>linked boards as the shared<br>desk of developer and AI"]
    repo[("Project repo in Git<br>code · Markdown · abyme/")]
    export["Other systems<br>Miro · Mermaid · SVG"]
    ext["External sources<br>e.g. tickets"]

    dev <-->|"sees and edits boards"| abyme
    abyme <-->|"MCP: reads context,<br>writes knowledge"| ai
    ai -->|"reads and changes code"| repo
    abyme <-->|"stores boards,<br>detects changes"| repo
    abyme -->|"exports"| export
    abyme -.->|"later"| ext

    classDef later stroke-dasharray: 2 3
    class ext later
```

- The **developer** sees and edits the boards in the canvas.
- The **AI** reads Abyme through MCP as context and writes its interpretation of the repo back. It reads and changes the code with its own tools.
- Abyme **stores** the boards in the project repo and detects changes there.
- **Other systems** such as Miro get exports, in one direction only.
- **External sources** such as tickets come later.

## Level 2 · Building Blocks

```mermaid
flowchart TB
    dev(["Developer"])

    subgraph abyme["Abyme"]
        canvas["<b>Canvas</b><br>Tauri app with React Flow"]
        export["<b>Exports</b><br>one-way: Miro, Mermaid, SVG"]
        core["<b>Core</b><br>operations · reconciliation · checks<br>layout · type packs"]
        mcp["<b>MCP server</b><br>tools for the AI"]
        parser["<b>Parsers</b><br>optional, e.g. Cargo.toml"]
        fetcher["<b>Fetchers</b><br>later"]
        cache[("<b>Cache</b><br>search index · render cache<br>not in Git")]
    end

    ai(["AI agent<br>e.g. Claude Code"])
    ext[("External sources<br>e.g. tickets")]

    subgraph repo["Project repo in Git"]
        format[("<b>abyme/</b><br>Abyme format")]
        code[("Code · Markdown<br>migrations …")]
    end

    dev <-->|"sees, edits, selects"| canvas
    canvas <-->|"operations,<br>live changes"| core
    export -->|"reads boards"| core
    mcp -->|"derives search index"| cache
    core <-->|"operations"| mcp
    mcp <-->|"reads context,<br>writes interpretation"| ai
    ai -->|"reads with its own tools"| code
    core <---->|"reads, writes"| format
    core ---->|"git diff at anchors"| code
    core -.-> parser -..->|"reads"| code
    core -.-> fetcher -.-> ext

    classDef proposal stroke-dasharray: 6 4
    classDef later stroke-dasharray: 2 3
    class parser proposal
    class fetcher,ext later
```

| Building block | Job | More |
|---|---|---|
| Canvas | Tauri app in which the developer sees, edits, and selects boards | [[Canvas]] |
| Exports | output boards one-way to Miro, Mermaid, or SVG | [[Hourglass#Outputs]] |
| Core | operations on the format, reconciliation, checks, layout, type packs | [[Data Model]], [[Reconciliation]], [[Checks]], [[Placement and Layout]] |
| MCP server | tools for the AI | [[MCP Server]] |
| Parsers | read known formats without the AI, optional | [[Extensions#Parsers]] |
| Fetchers | connect external sources, later | [[Extensions#Fetchers]] |
| Cache | search index and render cache, not in Git | [[Search and Index]] |
| `abyme/` | the Abyme format in the project repo | [[Storage in Git]] |

**Hooks** for Git and Claude Code trigger reconciliation, see [[Project Knowledge and AI Memory]].

How content flows through Abyme is shown by the [[Hourglass]]. Who depends on whom, which crates exist, and how the parts work together at runtime is shown by the [[Hexagonal Architecture]].
