import { type NodeChange, applyNodeChanges } from "@xyflow/react";
import { useCallback, useState } from "react";

import type { BoardView } from "./bindings";
import {
  type ElementNode,
  type Move,
  finishedMoves,
  keepUiState,
  returnHome,
  toNodes,
} from "./nodes";

/**
 * The boxes of `view` as React Flow's controlled state. A new projection
 * replaces them, but keeps what the user selected and is dragging. Every finished move goes
 * to `move`, all moves of one change in one call; when that fails, their
 * boxes return to their positions in the board file and `onMoveFailed`
 * receives the message.
 */
export function useBoardNodes(
  view: BoardView,
  move: (moves: Move[]) => Promise<void>,
  onMoveFailed: (message: string) => void,
) {
  const [nodes, setNodes] = useState(() => toNodes(view));
  const [shownView, setShownView] = useState(view);
  if (view !== shownView) {
    setShownView(view);
    setNodes((current) => keepUiState(toNodes(view), current));
  }

  const onNodesChange = useCallback(
    (changes: NodeChange<ElementNode>[]) => {
      setNodes((current) => applyNodeChanges(changes, current));
      const moves = finishedMoves(changes);
      if (moves.length === 0) {
        return;
      }
      const elements = moves.map((finished) => finished.element);
      move(moves).catch((error: unknown) => {
        setNodes((current) => returnHome(current, elements));
        onMoveFailed(String(error));
      });
    },
    [move, onMoveFailed],
  );

  return { nodes, onNodesChange };
}
