import type { XYPosition } from "@xyflow/react";

import type { AdmittedOperation } from "./client";

/** The operation that moves `element` on `board` to `position`, rounded to the integers the board file holds. */
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
  };
}
