import "@xyflow/react/dist/style.css";

import { ReactFlow } from "@xyflow/react";
import { useMemo } from "react";

import type { BoardView } from "./bindings";
import { useBoardNodes } from "./boardNodes";
import { toEdges } from "./edges";
import { ElementBox } from "./ElementBox";
import type { Move } from "./nodes";

/** The one node type the canvas knows. */
const NODE_TYPES = { element: ElementBox };

/**
 * A board's boxes and the arrows between them on the canvas. Moving boxes
 * moves their elements; the canvas cannot delete a box, since no delete
 * reaches the board file. Arrows follow from the view and hold no state.
 */
export function BoardCanvas({
  view,
  move,
  onMoveFailed,
}: {
  view: BoardView;
  move: (moves: Move[]) => Promise<void>;
  onMoveFailed: (message: string) => void;
}) {
  const { nodes, onNodesChange } = useBoardNodes(view, move, onMoveFailed);
  const edges = useMemo(() => toEdges(view), [view]);

  return (
    <ReactFlow
      nodes={nodes}
      edges={edges}
      nodeTypes={NODE_TYPES}
      onNodesChange={onNodesChange}
      nodesConnectable={false}
      deleteKeyCode={null}
      fitView
    />
  );
}
