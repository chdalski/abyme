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
 * boards its element owns, whether the board file pins it, the box it hangs on,
 * and, for a new note no file holds yet, the ID the write gave it.
 */
export type ElementData = {
  element: string;
  shown: Shown;
  home: XYPosition;
  innerBoards: BoardEntry[];
  pinned: boolean;
  attachedTo: string | null;
  draft: { createdAs: string | null; writtenAt: number | null } | null;
};

/** A box on the canvas. */
export type ElementNode = Node<ElementData, "element">;

/**
 * A box the user put somewhere else, at its position on the board: by
 * dragging it, alone or with others, or with the arrow keys. A note's move may
 * also hang it on another box, or take it off the one it hung on.
 */
export type Move = {
  element: string;
  position: XYPosition;
  hang?: { attachTo: string } | "detach";
};

/**
 * The boxes of `view`, one per placement, at the rect the projection gives. A
 * note that hangs on a box is that box's child node, at its position relative
 * to the box, so it follows a drag of the box; parents come first, as React
 * Flow requires.
 * Every box has the one node type `element`; nothing from the repository
 * chooses a node type, and the node's ID is the validated element ID. A note
 * can be dragged on every board, a box whose element file exists only on a
 * board that is not mirrored, and a box whose file is missing nowhere: React
 * Flow goes by this value on the node, also for the arrow keys. It offers what
 * Rust's admission and the core take.
 * Arrows leave at the bottom and arrive at the top; the handles come with the
 * node, since a fresh node after a reload has no measurements and its arrows
 * would vanish until it is measured again.
 */
export function toNodes(view: BoardView): ElementNode[] {
  const placed = new Map(
    view.placements.map((placement) => [placement.element, placement]),
  );
  const nodes = view.placements.map((placement): ElementNode => {
    const parent =
      placement.attached_to === null
        ? undefined
        : placed.get(placement.attached_to);
    const home =
      parent === undefined
        ? { x: placement.x, y: placement.y }
        : { x: placement.x - parent.x, y: placement.y - parent.y };
    return {
      id: placement.element,
      type: "element",
      position: home,
      ...(parent === undefined ? {} : { parentId: parent.element }),
      width: placement.width,
      height: placement.height,
      data: {
        element: placement.element,
        shown: placement.shown,
        home,
        innerBoards: placement.inner_boards,
        pinned: placement.pinned,
        attachedTo: parent === undefined ? null : parent.element,
        draft: null,
      },
      draggable:
        placement.shown.kind === "note" ||
        (placement.shown.kind === "element" && view.mode !== "mirrored"),
      deletable: placement.shown.kind === "note",
      connectable: false,
      handles: [
        // No size, as the handles in canvas.css: the two change together.
        {
          type: "target",
          position: Position.Top,
          x: placement.width / 2,
          y: 0,
          width: 0,
          height: 0,
        },
        {
          type: "source",
          position: Position.Bottom,
          x: placement.width / 2,
          y: placement.height,
          width: 0,
          height: 0,
        },
      ],
    };
  });
  return [
    ...nodes.filter((node) => node.parentId === undefined),
    ...nodes.filter((node) => node.parentId !== undefined),
  ];
}

/** Where `node` lies on the board: its position, plus its parent's for a child node. */
function onTheBoard(
  node: ElementNode,
  byId: Map<string, ElementNode>,
): XYPosition {
  const parent =
    node.parentId === undefined ? undefined : byId.get(node.parentId);
  return parent === undefined
    ? node.position
    : {
        x: parent.position.x + node.position.x,
        y: parent.position.y + node.position.y,
      };
}

/**
 * `moves`, as React Flow reports them for `nodes` as they stand after the
 * moves, each at its position on the board, and a note's with where it now
 * hangs: on the box drawn on top whose rect holds the note's centre, being no
 * note, no box whose file is missing, and not the note itself; else on the box
 * it hangs on while that box holds the centre, whatever it shows now; and off
 * that box otherwise.
 */
export function onTheBoardAndHung(moves: Move[], nodes: ElementNode[]): Move[] {
  const byId = new Map(nodes.map((node) => [node.id, node]));
  return moves.map((move) => {
    const node = byId.get(move.element);
    if (node === undefined) {
      return move;
    }
    const position = onTheBoard({ ...node, position: move.position }, byId);
    if (node.data.shown.kind !== "note" || node.data.draft !== null) {
      return { element: move.element, position };
    }
    const centre = {
      x: position.x + (node.width ?? 0) / 2,
      y: position.y + (node.height ?? 0) / 2,
    };
    const holdsCentre = (box: ElementNode) => {
      const at = onTheBoard(box, byId);
      return (
        centre.x >= at.x &&
        centre.x <= at.x + (box.width ?? 0) &&
        centre.y >= at.y &&
        centre.y <= at.y + (box.height ?? 0)
      );
    };
    const current = node.data.attachedTo;
    const below = nodes.filter(
      (box) =>
        box.id !== node.id &&
        box.data.draft === null &&
        box.data.shown.kind !== "note" &&
        box.data.shown.kind !== "missing" &&
        holdsCentre(box),
    );
    const target = below.at(-1)?.id;
    if (target !== undefined && target !== current) {
      return { element: move.element, position, hang: { attachTo: target } };
    }
    // A note stays on its box, whatever the box shows now, while its centre
    // lies on it.
    const host = current === null ? undefined : byId.get(current);
    if (target === undefined && host !== undefined && !holdsCentre(host)) {
      return { element: move.element, position, hang: "detach" };
    }
    return { element: move.element, position };
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
 * it, and a box `current` is dragging stays where the drag holds it. A new
 * note stays until `fresh` lists it, and a written one also leaves once a view
 * asked for after its write answered (`askedAt`) leaves it out.
 */
export function keepUiState(
  fresh: ElementNode[],
  current: ElementNode[],
  askedAt: number | null,
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
  // before the note's write answered; a view asked for after it that leaves the
  // note out says another writer deleted it.
  const drafts = current.filter(({ data: { draft } }) => {
    if (draft === null) {
      return false;
    }
    if (draft.createdAs === null) {
      return true;
    }
    const deleted =
      askedAt !== null && draft.writtenAt !== null && askedAt > draft.writtenAt;
    return !listed.has(draft.createdAs) && !deleted;
  });
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
      attachedTo: null,
      draft: { createdAs: null, writtenAt: null },
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
