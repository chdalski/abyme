import type { BoardEntry, BoardView } from "./bindings";
import type { AbymeClient } from "./client";

/** What the board list shows. */
export type BoardsState =
  | { kind: "loading" }
  | { kind: "loaded"; boards: BoardEntry[] }
  | { kind: "failed"; message: string };

/** What the open board shows. */
export type BoardState =
  | { kind: "loading" }
  | { kind: "loaded"; view: BoardView }
  | { kind: "failed"; message: string };

/** Asks `client` for the boards; a rejection becomes the message shown. */
export async function loadBoards(client: AbymeClient): Promise<BoardsState> {
  try {
    return { kind: "loaded", boards: await client.listBoards() };
  } catch (error) {
    return { kind: "failed", message: String(error) };
  }
}

/** Asks `client` for the board `board`; a rejection becomes the message shown. */
export async function loadBoard(
  client: AbymeClient,
  board: string,
): Promise<BoardState> {
  try {
    return { kind: "loaded", view: await client.openBoard(board) };
  } catch (error) {
    return { kind: "failed", message: String(error) };
  }
}

/**
 * Wraps `task` so that at most one run is in flight: a call during a run
 * makes one more run after it, however many calls arrive meanwhile.
 */
export function coalesced(task: () => Promise<void>): () => Promise<void> {
  let running = false;
  let pending = false;
  return async () => {
    if (running) {
      pending = true;
      return;
    }
    running = true;
    try {
      do {
        pending = false;
        await task();
      } while (pending);
    } finally {
      running = false;
    }
  };
}

/**
 * Wraps `task` so that its runs never overlap and start in the order of the
 * calls: each call waits until every earlier one has ended, failed or not.
 */
export function inOrder<T>(
  task: (input: T) => Promise<void>,
): (input: T) => Promise<void> {
  let last: Promise<unknown> = Promise.resolve();
  return (input) => {
    const run = last.then(() => task(input));
    last = run.catch(() => undefined);
    return run;
  };
}
