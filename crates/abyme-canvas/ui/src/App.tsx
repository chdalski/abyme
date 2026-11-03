import "./canvas.css";

import { useCallback, useEffect, useRef, useState } from "react";

import { BoardList } from "./BoardList";
import { BoardPane } from "./BoardPane";
import { type BoardsState, loadBoards } from "./boards";
import type { AbymeClient } from "./client";
import { ReviewMarks } from "./ElementBox";
import { type FlagsState, NO_MARKS, loadFlags, whileVisible } from "./flags";
import { useLive } from "./live";
import { RestOfOpeningPress } from "./press";

/** Listened for on the window before any handler of the page sees the event. */
const BEFORE_ALL = { capture: true, passive: true } as const;

/**
 * What a failed note write holds back: nothing, the next click to another
 * board, or the further clicks of the press that click belonged to.
 */
type Hold = "none" | "next" | "press";

/**
 * The way to another board, which waits for a note's write: a click on it ends
 * the note's input first, and a failed write must not take the text away with
 * the board. So it goes only after the write worked. After a write failed, the
 * clicks of the press that ended the input stay too, since its mouse button went
 * down before the failure; a press that starts after the failure goes on
 * purpose, with the message on the screen. One press opens at most one board:
 * the further clicks of a press that opened one, which land on the new board,
 * open nothing.
 */
function useOpenAfterNoteWrite(setOpen: (board: string) => void) {
  const pending = useRef<Promise<boolean> | null>(null);
  const hold = useRef<Hold>("none");
  // Which click of its press the click being handled is: 2 and more for the
  // further clicks of a double click, 0 for one from the keyboard.
  const clickOfPress = useRef(0);
  // Whether a board opened during the press going on, so that its further
  // clicks, which land on that board, act on nothing there. A write that answers
  // late marks the press going on then, whose clicks land on the new board too.
  const openedByPress = useRef(false);
  useEffect(() => {
    // A new press ends the hold and what the last press opened: a mouse button
    // going down for a first click, or a key.
    const newPress = () => {
      hold.current = "none";
      openedByPress.current = false;
    };
    const pressed = (event: MouseEvent) => {
      if (event.detail <= 1) {
        newPress();
      }
    };
    const keyed = () => {
      newPress();
    };
    const clicked = (event: MouseEvent) => {
      clickOfPress.current = event.detail;
    };
    window.addEventListener("mousedown", pressed, BEFORE_ALL);
    window.addEventListener("keydown", keyed, BEFORE_ALL);
    window.addEventListener("click", clicked, BEFORE_ALL);
    window.addEventListener("dblclick", clicked, BEFORE_ALL);
    return () => {
      window.removeEventListener("mousedown", pressed, BEFORE_ALL);
      window.removeEventListener("keydown", keyed, BEFORE_ALL);
      window.removeEventListener("click", clicked, BEFORE_ALL);
      window.removeEventListener("dblclick", clicked, BEFORE_ALL);
    };
  }, []);
  const noteWritten = useCallback((write: Promise<boolean>) => {
    pending.current = write;
    void write.then((written) => {
      if (pending.current === write) {
        pending.current = null;
      }
      hold.current = written ? "none" : "next";
    });
  }, []);
  const isRestOfOpeningPress = useCallback(
    () => openedByPress.current && clickOfPress.current >= 2,
    [],
  );
  const open = useCallback(
    (board: string) => {
      if (isRestOfOpeningPress()) {
        return;
      }
      const click = clickOfPress.current;
      const opened = () => {
        setOpen(board);
        openedByPress.current = click >= 1;
      };
      if (pending.current !== null) {
        void pending.current.then((written) => {
          if (written) {
            opened();
          } else {
            hold.current = "press";
          }
        });
      } else if (hold.current === "next") {
        hold.current = "press";
      } else if (hold.current === "press" && click >= 2) {
        // A further click of the press that is held back.
      } else {
        hold.current = "none";
        opened();
      }
    },
    [setOpen, isRestOfOpeningPress],
  );
  return { open, noteWritten, isRestOfOpeningPress };
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
  const {
    open: openBoard,
    noteWritten,
    isRestOfOpeningPress,
  } = useOpenAfterNoteWrite(setOpen);
  const askForFlags = useCallback(() => loadFlags(client), [client]);
  const flags = useLive<FlagsState>(
    client,
    askForFlags,
    { kind: "pending" },
    whileVisible,
  );

  return (
    <RestOfOpeningPress.Provider value={isRestOfOpeningPress}>
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
    </RestOfOpeningPress.Provider>
  );
}
