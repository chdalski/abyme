---
tags:
  - ai
aliases:
  - Layout
  - Auto-Layout
---
# Placement and Layout

## Three Levels

1. **Semantic (the normal case):** “Put X on board B, in frame F, to the right of Z.” The layout engine turns this into coordinates and looks for free space.
2. **Auto-layout:** The AI supplies only elements and relations, and an algorithm arranges them. ELK, for example, suits box-and-arrow diagrams such as C4 well.
3. **Coordinates:** for fine work.

## Rules

- **Pinned positions:** What a person placed by hand is moved neither by the AI nor by auto-layout without asking.
- **Transactional:** A package of changes succeeds completely or not at all.
- **Visual check:** The `render` tool returns an image of a board or a section. That way the AI sees what reaches the person, such as overlaps or cut-off text (see [[Canvas#Visual Check]]).
- **A building block of its own:** The MCP server and the canvas both need layout and placement, so they are a crate of their own (see [[Hexagonal Architecture#Crates]]).

## Prior Art

- **Miro** reads and writes boards via MCP as SVG (`canvas_read_as_svg`, `canvas_update_from_svg`). Every element has a stable `data-miro-id`. Diagrams are described as Mermaid inside the SVG and arranged by Miro. ([Miro MCP Tools](https://developers.miro.com/docs/miro-mcp-tools))
- **tldraw** gives the AI in its agent kit a screenshot plus a simplified list of shapes with type, text, position, and size, along with short IDs. ([tldraw Agent Starter Kit](https://tldraw.dev/starter-kits/agent))

The common pattern: a format language models already know, explicit coordinates, stable and short IDs, and visual feedback. Abyme adds a semantic layer on top.
