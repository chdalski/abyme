---
tags:
  - architecture
aliases:
  - Extension
  - Type Packs
  - Type Pack
  - Parser
  - Fetcher
  - Snapshot
---
# Extensions

The AI interprets any content in the repo (see [[Hourglass#Inputs]]), and changes are detected through anchors and Git. So for much of it, no extension is needed at all. Extensions come in where the AI alone is not enough:

| Kind | Purpose | Examples |
|---|---|---|
| **Type packs** | extend the vocabulary: semantic types, relation types, type rules, templates | C4, ADR, mind map |
| **Exports** | output boards to other systems, one-way only | Miro, Mermaid, SVG/PNG, Structurizr |
| **Parsers** (optional) | read common structures exactly without the AI, when interpretation by the AI is too expensive or too inaccurate | `Cargo.toml`, `package.json`, docker-compose |
| **Fetchers** (later) | fetch data from sources outside the repo | tickets, schema of a live database |

The core takes over everything shared, so extensions stay small: base snapshots, detecting changes, the diff, review flags, change requests, zones, modes, references, deleting, layout, and search.

## Type Packs

A type pack brings:

- **Semantic types**, e.g. person, system, container, and component for C4
- **Relation types**, e.g. “uses” with the technology
- **Type rules**, e.g. “a container belongs to a system”. These become structural [[Checks]].
- **Templates**, e.g. a C4 board whose elements have inner boards for the next level
- **Default presentations** per type, e.g. boxes in a particular color for containers

## Parsers

Parsers are optional. A structure is first interpreted by the AI and gets a parser only when that pays off. Parsers read known formats in the repo without the AI and deliver a normalized snapshot.

> [!question] Proposal
> The snapshot format is a draft.

```yaml
source: cargo://workspace
version: 3f2a9c1
things:
  - id: payment-service
    type: crate
    summary: Payment processing through Stripe
    attributes: { path: crates/payment-service, version: 0.4.0 }
  - id: legacy-mailer
    type: crate
relations:
  - from: order-service
    to: legacy-mailer
    type: depends_on
```

### Code Parser

> [!question] Proposal

A parser based on tree-sitter reads definitions and references in the code, that is which functions and types a file defines and which foreign ones it uses. Instead of one parser per language, it covers every language for which there is a tree-sitter grammar with tag queries. The Rust crates are `tree-sitter` and `tree-sitter-tags`, both under MIT.

This makes two things possible:

- **Dependencies without the AI:** Which module uses which other one follows from the references. If the board claims “A uses B” but nothing in the code refers from A to B, there is a review flag. If the code shows a relation that is missing on the board, there is a hint.
- **Finer review flags**, see [[Reconciliation#Finer Review Flags]]

**Limit:** References are matched by name, not through resolved imports. Symbols with the same name in different modules produce false matches. For dependencies between packages, parsers for `Cargo.toml` or `package.json` stay more accurate.

## Fetchers

Fetchers come later. They fetch data from sources outside the repo, such as tickets. Each fetcher defines an anchor scheme, such as `jira://PROJ-123`, reads the state of the source as a snapshot, and can carry out [[Change Requests]] itself, such as setting a ticket status.

## Examples

| Source | Route | Anchor | Typical mode |
|---|---|---|---|
| code, e.g. crates | AI, optionally a parser for `Cargo.toml` | `file://crates/` | synced |
| Markdown documents | AI | `file://docs/` | synced |
| images in the repo | AI, e.g. for an image description | `file://assets/logo.png` | mirrored |
| DB schema | AI through the migrations in the repo | `file://migrations/` | mirrored |
| website components | AI through the source code | `file://src/pages/` | synced |
| tickets | fetcher (later) | `jira://PROJ-123` | synced |

> [!todo] Open
> - **Runtime:** Do extensions run in the same process as the core or as separate processes?
> - **Security:** Extensions run code, and fetchers need credentials. What do permissions and isolation look like?
> - **Versioning** of the interfaces and the distribution of extensions
