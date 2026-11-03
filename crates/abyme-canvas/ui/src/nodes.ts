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
 * boards its element owns, whether the board file pins it, and, for a new note no
 * file holds yet, the ID the write gave it.
 */
export type ElementData = {
  element: string;
  shown: Shown;
  home: XYPosition;
  innerBoards: BoardEntry[];
  pinned: boolean;
  draft: { createdAs: string | null } | null;
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
        draft: null,
      },
      draggable: view.mode !== "mirrored" || placement.shown.kind === "note",
      deletable: placement.shown.kind === "note",
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
 * The elements React Flow takes away in `changes`, each once: the selected notes
 * a key deleted. A new note no file holds is never among them.
 */
export function finishedRemovals(changes: NodeChange<ElementNode>[]): string[] {
  return [
    ...new Set(
      changes.flatMap((change) =>
        change.type === "remove" && !change.id.startsWith("+")
          ? [change.id]
          : [],
      ),
    ),
  ];
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
  const listed = new Set(fresh.map((node) => node.id));
  const kept = fresh.map((node) => {
    const was = held.get(node.id);
    if (was === undefined) {
      return node;
    }
    const selected = was.selected === true ? { selected: true } : {};
    return was.dragging === true
      ? { ...node, ...selected, position: was.position, dragging: true }
      : { ...node, ...selected };
  });
  // A new note stays until a view lists it, also when that view was asked for
  // before the note was written.
  const drafts = current.filter(
    ({ data: { draft } }) =>
      draft !== null &&
      (draft.createdAs === null || !listed.has(draft.createdAs)),
  );
  return [...kept, ...drafts];
}

/**
 * A new note at `position` that no file holds yet, under an ID no element can
 * have. It cannot be selected or dragged, so no move or update ever names it, but
 * it takes the mouse, which React Flow would turn off for such a node, or its
 * text field could not be clicked. It has the size the core gives a box without a
 * file: crates/abyme-core/src/layout/box_size.rs. The two change together.
 */
export function draftNote(id: string, position: XYPosition): ElementNode {
  return {
    id,
    type: "element",
    position,
    width: 180,
    height: 90,
    data: {
      element: id,
      shown: { kind: "note", name: "", body: "" },
      home: position,
      innerBoards: [],
      pinned: true,
      draft: { createdAs: null },
    },
    style: { pointerEvents: "all" },
    selectable: false,
    draggable: false,
    deletable: false,
    connectable: false,
    handles: [],
  };
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
