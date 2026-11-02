import type { XYPosition } from "@xyflow/react";
import { useCallback, useMemo, useState } from "react";

import type { BoardView } from "./bindings";

import { BoardCanvas } from "./BoardCanvas";
import { type BoardState, inOrder, loadBoard } from "./boards";
import type { AbymeClient, AdmittedOperation } from "./client";
import { useLive } from "./live";
import { toMove, toRelease } from "./moves";
import type { Move } from "./nodes";

/**
 * The open board: its title and boxes, or the core's message in their place
 * when it never loaded. When a later reload fails, it keeps the board as last
 * loaded with the message above it, so that nothing a person is doing goes away;
 * writes still go by the files. It shows the board `board` only; the app gives
 * every board a pane of its own, so a late answer for another board never
 * reaches it, and neither does the view it keeps.
 */
export function BoardPane({
  client,
  board,
  onOpen,
}: {
  client: AbymeClient;
  board: string;
  onOpen: (board: string) => void;
}) {
  const load = useCallback(() => loadBoard(client, board), [client, board]);
  const state = useLive<BoardState>(client, load, { kind: "loading" });
  const [lastView, setLastView] = useState<BoardView | null>(null);
  if (state.kind === "loaded" && state.view !== lastView) {
    setLastView(state.view);
  }
  const [moveFailure, setMoveFailure] = useState<string | null>(null);
  // Moves and releases reach the core in the order the user made them, so
  // that the board file ends at the last one, however fast the arrow keys
  // repeat, and a release right after a drag finds the box where the drag
  // left it.
  const send = useMemo(
    () =>
      inOrder((operations: AdmittedOperation[]) => client.apply(operations)),
    [client],
  );
  const move = useCallback(
    async (moves: Move[]) => {
      await send(
        moves.map(({ element, position }) => toMove(board, element, position)),
      );
      setMoveFailure(null);
    },
    [send, board],
  );
  const release = useCallback(
    (element: string, position: XYPosition) => {
      send([toRelease(board, element, position)]).then(
        () => setMoveFailure(null),
        (error: unknown) => setMoveFailure(String(error)),
      );
    },
    [send, board],
  );

  const view = state.kind === "loaded" ? state.view : lastView;
  if (view === null) {
    return state.kind === "failed" ? (
      <p role="alert">{state.message}</p>
    ) : (
      <p>Opening the board…</p>
    );
  }
  return (
    <>
      {view.trail.length > 0 && (
        <nav aria-label="Boards above" className="breadcrumb">
          <ol>
            {view.trail.map((above) => (
              <li key={above.id}>
                <button type="button" onClick={() => onOpen(above.id)}>
                  {above.title}
                </button>
              </li>
            ))}
          </ol>
        </nav>
      )}
      <h2>{view.title}</h2>
      {state.kind === "failed" && (
        <p role="alert" className="board-alert">
          This board may be out of date: {state.message}
        </p>
      )}
      {moveFailure !== null && (
        <p role="alert" className="board-alert">
          {moveFailure}
        </p>
      )}
      <div className="flow">
        <BoardCanvas
          view={view}
          move={move}
          release={release}
          onMoveFailed={setMoveFailure}
          onOpen={onOpen}
        />
      </div>
    </>
  );
}
