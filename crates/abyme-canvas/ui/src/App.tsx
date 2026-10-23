import "./canvas.css";

import { useCallback, useState } from "react";

import { BoardList } from "./BoardList";
import { BoardPane } from "./BoardPane";
import { type BoardsState, loadBoards } from "./boards";
import type { AbymeClient } from "./client";
import { useLive } from "./live";

/**
 * The canvas: the project's boards, and the board the user opened. Both are
 * asked for again whenever the project's files change.
 */
export function App({ client }: { client: AbymeClient }) {
  const load = useCallback(() => loadBoards(client), [client]);
  const boards = useLive<BoardsState>(client, load, { kind: "loading" });
  const [open, setOpen] = useState<string | null>(null);

  return (
    <main>
      <nav>
        <h1>Boards</h1>
        <BoardList state={boards} open={open} onOpen={setOpen} />
      </nav>
      <section className="board">
        {open === null ? (
          <p>Open a board from the list.</p>
        ) : (
          <BoardPane key={open} client={client} board={open} />
        )}
      </section>
    </main>
  );
}
