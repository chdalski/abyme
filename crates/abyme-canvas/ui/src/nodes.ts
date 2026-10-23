import type { Node } from "@xyflow/react";

import type { BoardView, Shown } from "./bindings";

/** What a box shows: its element's ID and what the projection says of it. */
export type ElementData = { element: string; shown: Shown };

/** A box on the canvas. */
export type ElementNode = Node<ElementData, "element">;

/**
 * The boxes of `view`, one per placement, at the rect from the board file.
 * Every box has the one node type `element`; nothing from the repository
 * chooses a node type, and the node's ID is the validated element ID.
 */
export function toNodes(view: BoardView): ElementNode[] {
  return view.placements.map((placement) => ({
    id: placement.element,
    type: "element",
    position: { x: placement.x, y: placement.y },
    width: placement.width,
    height: placement.height,
    data: { element: placement.element, shown: placement.shown },
    draggable: false,
    connectable: false,
  }));
}
