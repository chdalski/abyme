import type { XYPosition } from "@xyflow/react";
import { useCallback, useMemo, useState } from "react";

import type { BoardView } from "./bindings";

import { BoardCanvas } from "./BoardCanvas";
import { type BoardState, inOrder, loadBoard } from "./boards";
import type { AbymeClient, AdmittedRequest } from "./client";
import { useLive } from "./live";
import { toAttachment, toDetachment, toMove, toRelease } from "./moves";
import {
  type NoteParts,
  toNewNote,
  toNoteDeletion,
  toNoteUpdate,
} from "./notes";
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
  onNoteWrite = () => {},
}: {
  client: AbymeClient;
  board: string;
  onOpen: (board: string) => void;
  onNoteWrite?: (write: Promise<boolean>) => void;
}) {
  const load = useCallback(() => loadBoard(client, board), [client, board]);
  const state = useLive<BoardState>(client, load, { kind: "loading" });
  // The view as last loaded, with the stamp of when it was asked for.
  const [lastLoaded, setLastLoaded] = useState<{
    view: BoardView;
    askedAt: number;
  } | null>(null);
  if (state.kind === "loaded" && state.view !== lastLoaded?.view) {
    setLastLoaded({ view: state.view, askedAt: state.askedAt });
  }
  const [writeFailure, setWriteFailure] = useState<string | null>(null);
  // Writes reach the core in the order the user made them, so
  // that the board file ends at the last one, however fast the arrow keys
  // repeat, and a release right after a drag finds the box where the drag
  // left it.
  const send = useMemo(
    () => inOrder((request: AdmittedRequest) => client.apply(request)),
    [client],
  );
  const move = useCallback(
    async (moves: Move[]) => {
      // A note's attachment goes right after its move, so that it hangs on
      // the box where the move put it.
      await send(
        moves.flatMap(({ element, position, hang }) => [
          toMove(board, element, position),
          ...(hang === undefined
            ? []
            : hang === "detach"
              ? [toDetachment(board, element)]
              : [toAttachment(board, element, hang.attachTo)]),
        ]),
      );
      setWriteFailure(null);
    },
    [send, board],
  );
  const release = useCallback(
    (element: string, position: XYPosition) => {
      send([toRelease(board, element, position)]).then(
        () => setWriteFailure(null),
        (error: unknown) => setWriteFailure(String(error)),
      );
    },
    [send, board],
  );

  const writeNote = useCallback(
    (element: string, parts: NoteParts) => {
      const write = send([toNoteUpdate(element, parts)]).then(
        () => {
          setWriteFailure(null);
          return true;
        },
        (error: unknown) => {
          setWriteFailure(String(error));
          return false;
        },
      );
      onNoteWrite(write);
      return write;
    },
    [send, onNoteWrite],
  );
  const createNote = useCallback(
    (position: XYPosition, parts: NoteParts) => {
      const created = send(toNewNote(board, position, parts)).then(
        ([id]) => {
          setWriteFailure(null);
          return id ?? null;
        },
        (error: unknown) => {
          setWriteFailure(String(error));
          return null;
        },
      );
      onNoteWrite(created.then((id) => id !== null));
      return created;
    },
    [send, board, onNoteWrite],
  );

  const view =
    state.kind === "loaded" ? state.view : (lastLoaded?.view ?? null);
  const askedAt =
    state.kind === "loaded" ? state.askedAt : (lastLoaded?.askedAt ?? null);
  // Only what the view shows as a note, whatever React Flow hands over; Rust
  // decides again from the files.
  const deleteNotes = useCallback(
    (elements: string[]) => {
      const notes = new Set(
        (view?.placements ?? [])
          .filter((placement) => placement.shown.kind === "note")
          .map((placement) => placement.element),
      );
      const deleted = elements.filter((element) => notes.has(element));
      if (deleted.length === 0) {
        return;
      }
      send(deleted.map(toNoteDeletion)).then(
        () => setWriteFailure(null),
        (error: unknown) => setWriteFailure(String(error)),
      );
    },
    [send, view],
  );
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
      {writeFailure !== null && (
        <p role="alert" className="board-alert">
          {writeFailure}
        </p>
      )}
      <div className="flow">
        <BoardCanvas
          view={view}
          askedAt={askedAt}
          move={move}
          release={release}
          writeNote={writeNote}
          createNote={createNote}
          deleteNotes={deleteNotes}
          onMoveFailed={setWriteFailure}
          onOpen={onOpen}
        />
      </div>
    </>
  );
}
