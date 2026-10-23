import { useEffect, useState } from "react";

import { BoardList } from "./BoardList";
import { type BoardsState, coalesced, loadBoards } from "./boards";
import type { AbymeClient } from "./client";

/** The canvas: the project's boards, asked for again whenever its files change. */
export function App({ client }: { client: AbymeClient }) {
  const [boards, setBoards] = useState<BoardsState>({ kind: "loading" });

  useEffect(() => {
    const refresh = coalesced(async () => setBoards(await loadBoards(client)));
    void refresh();
    const listening = client.onProjectChanged(() => void refresh());
    return () => {
      void listening.then((stop) => stop());
    };
  }, [client]);

  return (
    <main>
      <h1>Boards</h1>
      <BoardList state={boards} />
    </main>
  );
}
