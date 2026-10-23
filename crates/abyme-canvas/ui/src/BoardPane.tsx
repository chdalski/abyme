import { useCallback } from "react";

import { BoardCanvas } from "./BoardCanvas";
import { type BoardState, loadBoard } from "./boards";
import type { AbymeClient } from "./client";
import { useLive } from "./live";

/**
 * The open board: its title and boxes, or the core's message in their place.
 * It shows the board `board` only; the app gives every board a pane of its
 * own, so a late answer for another board never reaches it.
 */
export function BoardPane({
  client,
  board,
}: {
  client: AbymeClient;
  board: string;
}) {
  const load = useCallback(() => loadBoard(client, board), [client, board]);
  const state = useLive<BoardState>(client, load, { kind: "loading" });

  switch (state.kind) {
    case "loading":
      return <p>Opening the board…</p>;
    case "failed":
      return <p role="alert">{state.message}</p>;
    case "loaded":
      return (
        <>
          <h2>{state.view.title}</h2>
          <div className="flow">
            <BoardCanvas view={state.view} />
          </div>
        </>
      );
  }
}
