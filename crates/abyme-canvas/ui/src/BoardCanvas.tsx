import "@xyflow/react/dist/style.css";

import { type NodeChange, ReactFlow, applyNodeChanges } from "@xyflow/react";
import { useCallback, useEffect, useState } from "react";

import type { BoardView } from "./bindings";
import { ElementBox } from "./ElementBox";
import { type ElementNode, toNodes } from "./nodes";

/** The one node type the canvas knows. */
const NODE_TYPES = { element: ElementBox };

/** A board's boxes on the canvas; a new projection replaces them. */
export function BoardCanvas({ view }: { view: BoardView }) {
  const [nodes, setNodes] = useState(() => toNodes(view));
  useEffect(() => setNodes(toNodes(view)), [view]);
  const onNodesChange = useCallback(
    (changes: NodeChange<ElementNode>[]) =>
      setNodes((current) => applyNodeChanges(changes, current)),
    [],
  );

  return (
    <ReactFlow
      nodes={nodes}
      nodeTypes={NODE_TYPES}
      onNodesChange={onNodesChange}
      nodesDraggable={false}
      nodesConnectable={false}
      fitView
    />
  );
}
