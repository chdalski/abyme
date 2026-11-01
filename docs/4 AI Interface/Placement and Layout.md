---
tags:
  - ai
aliases:
  - Layout
  - Auto-Layout
---
# Placement and Layout

## Three Levels

1. **Semantic:** “Put X on board B, in frame F, to the right of Z.” The layout engine turns this into coordinates. Not built yet, like the tool `place`: `place_element` looks only for free space.
2. **Auto-layout:** The AI supplies only elements and relations, and the operation `layout_board` of `apply` arranges them as a layered (Sugiyama) diagram, which suits box-and-arrow diagrams such as C4 well (see [[#Layered Layout]]).
3. **Coordinates:** for fine work, with `move_element`.

## Layered Layout

`{"op": "layout_board", "board": <ref>}` runs in order with the other operations of a request, so one request can build a board and arrange it. It writes only the board file, so no verification stamp changes, and it may lay out a [[Boards and Zones#Modes|mirrored]] board.

- **Ranks:** A relation of the kind `uses`, `depends_on`, `describes`, or `concerns` between two elements on the board puts its source above its target. Where relations form a cycle, a walk over the elements in ID order leaves out the one that would close it, and only for the ranks. `see_also` only influences the order within a rank. Relations to elements on other boards count for nothing.
- **Parts and the rest:** Each connected part of the board is laid out on its own, the parts side by side by their smallest element ID. Boxes without relations stand in a grid of four columns below them.
- **Every unpinned box moves,** whatever its old position, and gets the size of its text, or 180 × 90 when its element has no file. So the result depends only on IDs, sizes, relations, and pins, and a repeated layout changes no byte. `place_element` remains the way to add a box without moving others.
- **Pinned boxes stay.** They take part in the ranks, so a chain through one stays connected, but they keep their position and size, and an unpinned box that would overlap one moves right past it.
- **When it is refused:** Every unpinned box must lie within -1,000,000 to 1,000,000, the range of `move_element`; otherwise the request fails with “no free position left”. A pin in the way can be released, after asking the person. The layouts of one request also share a limit on their work: beyond it the request fails and names the board, and fewer `layout_board` in one request, or a board with fewer boxes and relations, get through.

## Rules

- **Pinned positions:** What a person placed by hand is moved neither by the AI nor by auto-layout without asking. Moving a box in the canvas pins it, and a click on its marker releases the pin and leaves the box where it is. A `move_element` of a pinned element is refused unless it says `pinned`: `true` keeps the pin and `false` releases it, which the AI sends only after the person has said so. Only such a move pins a box; a box placed any other way is not pinned.
- **Box sizes:** A box that `place_element` places gets the size of its element's type, name, and summary: the width stays 180, and the height grows by whole lines from 90 up to 300. This is an estimate by character width, not a font measurement, so text can still be cut off. A later change of the text leaves the size as it is until the next layout.
- **Transactional:** A package of changes succeeds completely or not at all.
- **Visual check:** The `render` tool returns an image of a board or a section. That way the AI sees what reaches the person, such as overlaps or cut-off text (see [[Canvas#Visual Check]]).
- **Part of the core:** The MCP server and the canvas both need layout and placement and both use the core, so layout lives there. It becomes a crate of its own when the canvas alone needs it (see [[Hexagonal Architecture#Crates]]).

## Prior Art

- **Miro** reads and writes boards via MCP as SVG (`canvas_read_as_svg`, `canvas_update_from_svg`). Every element has a stable `data-miro-id`. Diagrams are described as Mermaid inside the SVG and arranged by Miro. ([Miro MCP Tools](https://developers.miro.com/docs/miro-mcp-tools))
- **tldraw** gives the AI in its agent kit a screenshot plus a simplified list of shapes with type, text, position, and size, along with short IDs. ([tldraw Agent Starter Kit](https://tldraw.dev/starter-kits/agent))

The common pattern: a format language models already know, explicit coordinates, stable and short IDs, and visual feedback. Abyme adds a semantic layer on top.
