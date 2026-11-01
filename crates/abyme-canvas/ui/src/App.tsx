import "./canvas.css";

import { useCallback, useState } from "react";

import { BoardList } from "./BoardList";
import { BoardPane } from "./BoardPane";
import { type BoardsState, loadBoards } from "./boards";
import type { AbymeClient } from "./client";
import { ReviewMarks } from "./ElementBox";
import { type FlagsState, NO_MARKS, loadFlags } from "./flags";
import { useLive } from "./live";

/**
 * The line above the board while the flags say nothing about its boxes, so that
 * a box without a mark does not read as clean.
 */
function FlagsStatus({ state }: { state: FlagsState }) {
  switch (state.kind) {
    case "pending":
      return (
        <p role="status" className="flags-status">
          Checking review flags…
        </p>
      );
    case "failed":
      return (
        <p role="status" className="flags-status">
          Review flags are unavailable, so no box is marked: {state.message}
        </p>
      );
    case "loaded":
      return null;
  }
}

/**
 * The canvas: the project's boards, the board the user opened, and the review
 * flags of its boxes. All are asked for again whenever the project's files
 * change. The flags stay out of the board's own answer and are asked for once,
 * not per board, since Git can be slow.
 */
export function App({ client }: { client: AbymeClient }) {
  const load = useCallback(() => loadBoards(client), [client]);
  const boards = useLive<BoardsState>(client, load, { kind: "loading" });
  const [open, setOpen] = useState<string | null>(null);
  const askForFlags = useCallback(() => loadFlags(client), [client]);
  const flags = useLive<FlagsState>(client, askForFlags, { kind: "pending" });

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
          <>
            <FlagsStatus state={flags} />
            <ReviewMarks.Provider
              value={flags.kind === "loaded" ? flags.marks : NO_MARKS}
            >
              <BoardPane
                key={open}
                client={client}
                board={open}
                onOpen={setOpen}
              />
            </ReviewMarks.Provider>
          </>
        )}
      </section>
    </main>
  );
}
