import "./canvas.css";

import { useCallback, useRef, useState } from "react";

import { BoardList } from "./BoardList";
import { BoardPane } from "./BoardPane";
import { type BoardsState, loadBoards } from "./boards";
import type { AbymeClient } from "./client";
import { ReviewMarks } from "./ElementBox";
import { type FlagsState, NO_MARKS, loadFlags, whileVisible } from "./flags";
import { useLive } from "./live";

/**
 * The way to another board, which waits for a note's write: a click on it ends
 * the note's input first, and a failed write must not take the text away with
 * the board. So it goes only after the write worked. After a write failed, the
 * next click stays too, since it can belong to the same press of the mouse, and
 * the one after that goes on purpose, with the message on the screen.
 */
function useOpenAfterNoteWrite(setOpen: (board: string) => void) {
  const pending = useRef<Promise<boolean> | null>(null);
  const heldBack = useRef(false);
  const noteWritten = useCallback((write: Promise<boolean>) => {
    pending.current = write;
    void write.then((written) => {
      if (pending.current === write) {
        pending.current = null;
      }
      heldBack.current = !written;
    });
  }, []);
  const open = useCallback(
    (board: string) => {
      if (pending.current !== null) {
        void pending.current.then((written) => {
          if (written) {
            setOpen(board);
          } else {
            heldBack.current = false;
          }
        });
      } else if (heldBack.current) {
        heldBack.current = false;
      } else {
        setOpen(board);
      }
    },
    [setOpen],
  );
  return { open, noteWritten };
}

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
 * change, and the flags also while the window is visible. The flags stay out of
 * the board's own answer and are not asked for per board, since Git can be slow.
 */
export function App({ client }: { client: AbymeClient }) {
  const load = useCallback(() => loadBoards(client), [client]);
  const boards = useLive<BoardsState>(client, load, { kind: "loading" });
  const [open, setOpen] = useState<string | null>(null);
  const { open: openBoard, noteWritten } = useOpenAfterNoteWrite(setOpen);
  const askForFlags = useCallback(() => loadFlags(client), [client]);
  const flags = useLive<FlagsState>(
    client,
    askForFlags,
    { kind: "pending" },
    whileVisible,
  );

  return (
    <main>
      <nav>
        <h1>Boards</h1>
        <BoardList state={boards} open={open} onOpen={openBoard} />
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
                onOpen={openBoard}
                onNoteWrite={noteWritten}
              />
            </ReviewMarks.Provider>
          </>
        )}
      </section>
    </main>
  );
}
