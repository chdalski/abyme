import type { BoardEntry, BoardView } from "./bindings";
import type { AbymeClient } from "./client";

/** What the board list shows. */
export type BoardsState =
  | { kind: "loading" }
  | { kind: "loaded"; boards: BoardEntry[] }
  | { kind: "failed"; message: string };

/**
 * What the open board shows, and for a view the stamp taken when it was asked
 * for, which tells whether it can know of a write that answered at a stamp.
 */
export type BoardState =
  | { kind: "loading" }
  | { kind: "loaded"; view: BoardView; askedAt: number }
  | { kind: "failed"; message: string };

let lastStamp = 0;

/**
 * A stamp later than every one taken before it, to tell which of two moments
 * of this page came first; no clock, so no two stamps are equal.
 */
export function nextStamp(): number {
  lastStamp += 1;
  return lastStamp;
}

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
  // Taken before the question goes out, so a view counts as later than a write
  // only if it was asked for after the write answered.
  const askedAt = nextStamp();
  try {
    return { kind: "loaded", view: await client.openBoard(board), askedAt };
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
export function inOrder<T, R>(
  task: (input: T) => Promise<R>,
): (input: T) => Promise<R> {
  let last: Promise<unknown> = Promise.resolve();
  return (input) => {
    const run = last.then(() => task(input));
    last = run.catch(() => undefined);
    return run;
  };
}
