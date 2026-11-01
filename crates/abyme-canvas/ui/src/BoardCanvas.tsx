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

/** The one node type the canvas knows. */
const NODE_TYPES = { element: ElementBox };

/**
 * A board's boxes and the arrows between them on the canvas. Moving boxes
 * moves their elements; the canvas cannot delete a box, since no delete
 * reaches the board file. Arrows follow from the view and hold no state. A
 * mirrored board offers no release of a pin, as it offers no move.
 */
export function BoardCanvas({
  view,
  move,
  release,
  onMoveFailed,
  onOpen,
}: {
  view: BoardView;
  move: (moves: Move[]) => Promise<void>;
  release: (element: string, position: XYPosition) => void;
  onMoveFailed: (message: string) => void;
  onOpen: (board: string) => void;
}) {
  const { nodes, onNodesChange } = useBoardNodes(view, move, onMoveFailed);
  const edges = useMemo(() => toEdges(view), [view]);
  const openFirstInnerBoard = useCallback<NodeMouseHandler<ElementNode>>(
    (_event, node) => {
      const [first] = node.data.innerBoards;
      if (first !== undefined) {
        onOpen(first.id);
      }
    },
    [onOpen],
  );

  return (
    <OpenBoard.Provider value={onOpen}>
      <ReleasePin.Provider value={view.mode === "mirrored" ? null : release}>
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
      </ReleasePin.Provider>
    </OpenBoard.Provider>
  );
}
