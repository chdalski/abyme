import {
  type NodeChange,
  type XYPosition,
  applyNodeChanges,
} from "@xyflow/react";
import { useCallback, useRef, useState } from "react";

import type { BoardView } from "./bindings";
import {
  type ElementNode,
  type Move,
  draftNote,
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
 * receives the message. A new note lives among the boxes, as React Flow measures
 * only what this state holds, under an ID no element can have.
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

  const nextDraft = useRef(1);
  const addDraft = useCallback((position: XYPosition) => {
    const id = `+note-${String(nextDraft.current)}`;
    nextDraft.current += 1;
    setNodes((current) => [...current, draftNote(id, position)]);
  }, []);
  const dropDraft = useCallback((id: string) => {
    setNodes((current) => current.filter((node) => node.id !== id));
  }, []);
  const draftWritten = useCallback((id: string, createdAs: string) => {
    setNodes((current) =>
      // A view can list the note before the write's answer arrives.
      current.some((node) => node.id === createdAs)
        ? current.filter((node) => node.id !== id)
        : current.map((node) =>
            node.id === id
              ? { ...node, data: { ...node.data, draft: { createdAs } } }
              : node,
          ),
    );
  }, []);

  return { nodes, onNodesChange, addDraft, dropDraft, draftWritten };
}
