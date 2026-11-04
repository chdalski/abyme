import {
  type NodeChange,
  type XYPosition,
  applyNodeChanges,
} from "@xyflow/react";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";

import type { BoardView } from "./bindings";
import { nextStamp } from "./boards";
import {
  type ElementNode,
  type Move,
  closeNoteField,
  draftNote,
  finishedMoves,
  finishedRemovals,
  keepUiState,
  onTheBoardAndHung,
  returnHome,
  toNodes,
  withNoteField,
} from "./nodes";

/**
 * The boxes of `view` as React Flow's controlled state. A new projection
 * replaces them, but keeps what the user selected and is dragging and the text
 * fields that are open. Every finished move goes to `move` at its position on
 * the board, a note's with the box it now hangs on, all moves of one change in
 * one call; when that fails, their boxes return to their positions in the board
 * file and `onMoveFailed` receives the message. A deleted note goes to
 * `remove` and stays until a view leaves it out. A new note lives among the
 * boxes, as React Flow measures only what this state holds, under an ID no
 * element can have; once written, it goes when a view asked for after the
 * write answered (`askedAt`, a stamp of `nextStamp`) leaves it out. A note a
 * view left out while its field was open stays until its field closes; nothing
 * is written from it, and no change React Flow reports for it is passed on.
 */
export function useBoardNodes(
  view: BoardView,
  askedAt: number | null,
  move: (moves: Move[]) => Promise<void>,
  onMoveFailed: (message: string) => void,
  remove: (elements: string[]) => void,
) {
  const [nodes, setNodes] = useState(() => toNodes(view));
  const [shownView, setShownView] = useState(view);
  if (view !== shownView) {
    setShownView(view);
    setNodes((current) => keepUiState(toNodes(view), current, askedAt));
  }

  // The boxes as last rendered, for deciding where a moved note now hangs; the
  // state itself changes only through updaters, so that no batch of changes
  // overwrites another that React has not rendered yet.
  const shown = useRef(nodes);
  useEffect(() => {
    shown.current = nodes;
  }, [nodes]);
  // The latest view, for what takes the place of a held note when its field
  // closes, however long it stayed open.
  const latest = useRef(view);
  useEffect(() => {
    latest.current = view;
  }, [view]);

  const onNodesChange = useCallback(
    (reported: NodeChange<ElementNode>[]) => {
      // A held note's ID may belong to another element by now, so neither its
      // removal nor its move goes on, whatever React Flow reports for it.
      const held = new Set(
        shown.current
          .filter((node) => node.data.leftOut)
          .map((node) => node.id),
      );
      const changes = reported.filter(
        (change) =>
          !(
            (change.type === "remove" ||
              change.type === "position" ||
              change.type === "select") &&
            held.has(change.id)
          ),
      );
      const kept = changes.filter((change) => change.type !== "remove");
      setNodes((current) => applyNodeChanges(kept, current));
      const removals = finishedRemovals(changes);
      if (removals.length > 0) {
        remove(removals);
      }
      const moves = onTheBoardAndHung(
        finishedMoves(changes),
        applyNodeChanges(kept, shown.current),
      );
      if (moves.length === 0) {
        return;
      }
      const elements = moves.map((finished) => finished.element);
      move(moves).catch((error: unknown) => {
        setNodes((current) => returnHome(current, elements));
        onMoveFailed(String(error));
      });
    },
    [move, onMoveFailed, remove],
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
    // The moment the write answered, taken here so that the updater stays pure.
    const writtenAt = nextStamp();
    setNodes((current) =>
      // A view can list the note before the write's answer arrives.
      current.some((node) => node.id === createdAs)
        ? current.filter((node) => node.id !== id)
        : current.map((node) =>
            node.id === id
              ? {
                  ...node,
                  data: {
                    ...node.data,
                    draft: { createdAs, writtenAt },
                  },
                }
              : node,
          ),
    );
  }, []);

  const noteFields = useMemo(
    () => ({
      open: (element: string) => {
        setNodes((current) => withNoteField(current, element, true));
      },
      close: (element: string) => {
        setNodes((current) =>
          closeNoteField(current, element, toNodes(latest.current)),
        );
      },
    }),
    [],
  );

  return {
    nodes,
    onNodesChange,
    addDraft,
    dropDraft,
    draftWritten,
    noteFields,
  };
}
