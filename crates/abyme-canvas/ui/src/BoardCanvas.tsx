import "@xyflow/react/dist/style.css";

import {
  type NodeMouseHandler,
  ReactFlow,
  type XYPosition,
} from "@xyflow/react";
import { useCallback, useMemo } from "react";

import type { BoardView } from "./bindings";
import { useBoardNodes } from "./boardNodes";
import { toEdges } from "./edges";
import { ElementBox, OpenBoard, ReleasePin } from "./ElementBox";
import type { ElementNode, Move } from "./nodes";
import { WriteNote } from "./NoteContent";
import type { NoteParts } from "./notes";

/** The one node type the canvas knows. */
const NODE_TYPES = { element: ElementBox };

/**
 * A board's boxes and the arrows between them on the canvas. Moving boxes
 * moves their elements; the canvas cannot delete a box, since no delete
 * reaches the board file. Arrows follow from the view and hold no state. A
 * mirrored board offers no release of a pin, and a move only of a note.
 */
export function BoardCanvas({
  view,
  move,
  release,
  writeNote,
  onMoveFailed,
  onOpen,
}: {
  view: BoardView;
  move: (moves: Move[]) => Promise<void>;
  release: (element: string, position: XYPosition) => void;
  writeNote: (element: string, parts: NoteParts) => Promise<boolean>;
  onMoveFailed: (message: string) => void;
  onOpen: (board: string) => void;
}) {
  const { nodes, onNodesChange } = useBoardNodes(view, move, onMoveFailed);
  const edges = useMemo(() => toEdges(view), [view]);
  const openFirstInnerBoard = useCallback<NodeMouseHandler<ElementNode>>(
    (_event, node) => {
      const [first] = node.data.innerBoards;
      // On a note, a double-click opens its text.
      if (first !== undefined && node.data.shown.kind !== "note") {
        onOpen(first.id);
      }
    },
    [onOpen],
  );

  return (
    <OpenBoard.Provider value={onOpen}>
      <ReleasePin.Provider value={view.mode === "mirrored" ? null : release}>
        <WriteNote.Provider value={writeNote}>
          <ReactFlow
            nodes={nodes}
            edges={edges}
            nodeTypes={NODE_TYPES}
            onNodesChange={onNodesChange}
            onNodeDoubleClick={openFirstInnerBoard}
            nodesConnectable={false}
            deleteKeyCode={null}
            fitView
          />
        </WriteNote.Provider>
      </ReleasePin.Provider>
    </OpenBoard.Provider>
  );
}
