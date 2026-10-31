import type { Node, NodeChange, XYPosition } from "@xyflow/react";

import type { BoardView, Shown } from "./bindings";

/**
 * What a box shows: its element's ID and what the projection says of it,
 * and its position in the board file, where it returns when a move fails.
 */
export type ElementData = { element: string; shown: Shown; home: XYPosition };

/** A box on the canvas. */
export type ElementNode = Node<ElementData, "element">;

/** A box the user put somewhere else: by dragging it, alone or with others, or with the arrow keys. */
export type Move = { element: string; position: XYPosition };

/**
 * The boxes of `view`, one per placement, at the rect from the board file.
 * Every box has the one node type `element`; nothing from the repository
 * chooses a node type, and the node's ID is the validated element ID. A box
 * on a mirrored board cannot be dragged: React Flow goes by this value on the
 * node, also for the arrow keys.
 */
export function toNodes(view: BoardView): ElementNode[] {
  return view.placements.map((placement) => {
    const home = { x: placement.x, y: placement.y };
    return {
      id: placement.element,
      type: "element",
      position: home,
      width: placement.width,
      height: placement.height,
      data: { element: placement.element, shown: placement.shown, home },
      draggable: view.mode !== "mirrored",
      connectable: false,
    };
  });
}

/**
 * The moves that `changes` finish. React Flow ends every move with a
 * position change that is no longer dragging: the end of a drag, of one box
 * or of several, and every press of an arrow key. A click without movement
 * reports none.
 */
export function finishedMoves(changes: NodeChange<ElementNode>[]): Move[] {
  return changes.flatMap((change) =>
    change.type === "position" &&
    change.dragging === false &&
    change.position !== undefined
      ? [{ element: change.id, position: change.position }]
      : [],
  );
}

/**
 * `fresh`, with the state of the canvas that a reload must not reset: a
 * box `current` has selected stays selected, so the arrow keys keep moving
 * it, and a box `current` is dragging stays where the drag holds it.
 */
export function keepUiState(
  fresh: ElementNode[],
  current: ElementNode[],
): ElementNode[] {
  const held = new Map(current.map((node) => [node.id, node]));
  return fresh.map((node) => {
    const was = held.get(node.id);
    if (was === undefined) {
      return node;
    }
    const selected = was.selected === true ? { selected: true } : {};
    return was.dragging === true
      ? { ...node, ...selected, position: was.position, dragging: true }
      : { ...node, ...selected };
  });
}

/** `nodes`, with the boxes of `elements` back at their positions in the board file. */
export function returnHome(
  nodes: ElementNode[],
  elements: string[],
): ElementNode[] {
  return nodes.map((node) =>
    elements.includes(node.id)
      ? { ...node, position: node.data.home, dragging: false }
      : node,
  );
}
