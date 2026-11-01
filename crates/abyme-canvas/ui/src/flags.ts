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
