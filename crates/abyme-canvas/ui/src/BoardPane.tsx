import { useCallback, useMemo, useState } from "react";

import { BoardCanvas } from "./BoardCanvas";
import { type BoardState, inOrder, loadBoard } from "./boards";
import type { AbymeClient } from "./client";
import { useLive } from "./live";
import { toMove } from "./moves";
import type { Move } from "./nodes";

/**
 * The open board: its title and boxes, or the core's message in their place.
 * It shows the board `board` only; the app gives every board a pane of its
 * own, so a late answer for another board never reaches it.
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
  const [moveFailure, setMoveFailure] = useState<string | null>(null);
  // Moves reach the core in the order the user made them, so that the
  // board file ends at the last one, however fast the arrow keys repeat.
  const send = useMemo(
    () =>
      inOrder((moves: Move[]) =>
        client.apply(
          moves.map(({ element, position }) =>
            toMove(board, element, position),
          ),
        ),
      ),
    [client, board],
  );
  const move = useCallback(
    async (moves: Move[]) => {
      await send(moves);
      setMoveFailure(null);
    },
    [send],
  );

  switch (state.kind) {
    case "loading":
      return <p>Opening the board…</p>;
    case "failed":
      return <p role="alert">{state.message}</p>;
    case "loaded":
      return (
        <>
          {state.view.trail.length > 0 && (
            <nav aria-label="Boards above" className="breadcrumb">
              <ol>
                {state.view.trail.map((above) => (
                  <li key={above.id}>
                    <button type="button" onClick={() => onOpen(above.id)}>
                      {above.title}
                    </button>
                  </li>
                ))}
              </ol>
            </nav>
          )}
          <h2>{state.view.title}</h2>
          {moveFailure !== null && <p role="alert">{moveFailure}</p>}
          <div className="flow">
            <BoardCanvas
              view={state.view}
              move={move}
              onMoveFailed={setMoveFailure}
              onOpen={onOpen}
            />
          </div>
        </>
      );
  }
}
