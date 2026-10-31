import type { XYPosition } from "@xyflow/react";

import type { AdmittedOperation } from "./client";

/** The operation that moves and pins `element` on `board` at `position`, rounded to the integers the board file holds: a person placed it there. */
export function toMove(
  board: string,
  element: string,
  position: XYPosition,
): AdmittedOperation {
  return {
    op: "move_element",
    element: { existing: element },
    board: { existing: board },
    x: Math.round(position.x),
    y: Math.round(position.y),
    pinned: true,
  };
}
