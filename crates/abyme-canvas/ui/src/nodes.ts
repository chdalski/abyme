import {
  type Node,
  type NodeChange,
  Position,
  type XYPosition,
} from "@xyflow/react";

import type { BoardEntry, BoardView, Shown } from "./bindings";

/**
 * What a box shows: its element's ID and what the projection says of it,
 * its position in the board file, where it returns when a move fails, the
 * boards its element owns, and whether the board file pins it.
 */
export type ElementData = {
  element: string;
  shown: Shown;
  home: XYPosition;
  innerBoards: BoardEntry[];
  pinned: boolean;
};

/** A box on the canvas. */
export type ElementNode = Node<ElementData, "element">;

/** A box the user put somewhere else: by dragging it, alone or with others, or with the arrow keys. */
export type Move = { element: string; position: XYPosition };

/**
 * The boxes of `view`, one per placement, at the rect from the board file.
 * Every box has the one node type `element`; nothing from the repository
 * chooses a node type, and the node's ID is the validated element ID. On a
 * mirrored board only a note can be dragged: React Flow goes by this value on
 * the node, also for the arrow keys. It offers what Rust's admission decides.
 * Arrows leave at the bottom and arrive at the top; the handles come with the
 * node, since a fresh node after a reload has no measurements and its arrows
 * would vanish until it is measured again.
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
      data: {
        element: placement.element,
        shown: placement.shown,
        home,
        innerBoards: placement.inner_boards,
        pinned: placement.pinned,
      },
      draggable: view.mode !== "mirrored" || placement.shown.kind === "note",
      connectable: false,
      handles: [
        {
          type: "target",
          position: Position.Top,
          x: placement.width / 2,
          y: 0,
          width: 1,
          height: 1,
        },
        {
          type: "source",
          position: Position.Bottom,
          x: placement.width / 2,
          y: placement.height,
          width: 1,
          height: 1,
        },
      ],
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
