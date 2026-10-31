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
2. **Auto-layout:** The AI supplies only elements and relations, and a layered (Sugiyama) layout arranges them, which suits box-and-arrow diagrams such as C4 well. The layout library must be MIT-compatible, which rules out the copyleft ones.
3. **Coordinates:** for fine work.

## Rules

- **Pinned positions:** What a person placed by hand is moved neither by the AI nor by auto-layout without asking. Moving a box in the canvas pins it. A `move_element` of a pinned element is refused unless it says `pinned`: `true` keeps the pin and `false` releases it, which the AI sends only after the person has said so. Boxes that lay on a board before pins existed are not pinned.
- **Box sizes:** A box that `place_element` places gets the size of its element's type, name, and summary: the width stays 180, and the height grows by whole lines from 90 up to 300. This is an estimate by character width, not a font measurement, so text can still be cut off. A later change of the text leaves the size as it is.
- **Transactional:** A package of changes succeeds completely or not at all.
- **Visual check:** The `render` tool returns an image of a board or a section. That way the AI sees what reaches the person, such as overlaps or cut-off text (see [[Canvas#Visual Check]]).
- **Part of the core:** The MCP server and the canvas both need layout and placement and both use the core, so layout lives there. It becomes a crate of its own when the canvas alone needs it (see [[Hexagonal Architecture#Crates]]).

## Prior Art

- **Miro** reads and writes boards via MCP as SVG (`canvas_read_as_svg`, `canvas_update_from_svg`). Every element has a stable `data-miro-id`. Diagrams are described as Mermaid inside the SVG and arranged by Miro. ([Miro MCP Tools](https://developers.miro.com/docs/miro-mcp-tools))
- **tldraw** gives the AI in its agent kit a screenshot plus a simplified list of shapes with type, text, position, and size, along with short IDs. ([tldraw Agent Starter Kit](https://tldraw.dev/starter-kits/agent))

The common pattern: a format language models already know, explicit coordinates, stable and short IDs, and visual feedback. Abyme adds a semantic layer on top.
