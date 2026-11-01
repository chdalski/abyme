import type { FlagPriority } from "./bindings";
import type { AbymeClient } from "./client";

/** The level of each flagged element. A `Map`, since an ID can name a property of every object. */
export type FlagMarks = ReadonlyMap<string, FlagPriority>;

/** What the canvas knows of the review flags. Only `loaded` marks a box. */
export type FlagsState =
  | { kind: "pending" }
  | { kind: "loaded"; marks: FlagMarks }
  | { kind: "failed"; message: string };

/** No box is marked. */
export const NO_MARKS: FlagMarks = new Map();

/** How often the flags are asked for while the window is visible. */
const INTERVAL_MS = 30_000;

/**
 * Calls `ask` when the window gets focus and every interval, while the document
 * is visible; returns a function that stops. The watcher sees neither commits
 * nor changes under an anchor, which is what raises flags.
 */
export function whileVisible(ask: () => void): () => void {
  const askIfVisible = () => {
    if (document.visibilityState === "visible") {
      ask();
    }
  };
  window.addEventListener("focus", askIfVisible);
  const interval = setInterval(askIfVisible, INTERVAL_MS);
  return () => {
    window.removeEventListener("focus", askIfVisible);
    clearInterval(interval);
  };
}

/** Asks `client` for the review flags; a rejection becomes the message shown. */
export async function loadFlags(client: AbymeClient): Promise<FlagsState> {
  try {
    const flags = await client.reviewFlags();
    return {
      kind: "loaded",
      marks: new Map(flags.map((flag) => [flag.element, flag.priority])),
    };
  } catch (error) {
    return { kind: "failed", message: String(error) };
  }
}
