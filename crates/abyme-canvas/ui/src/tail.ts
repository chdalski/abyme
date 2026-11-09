import type { Rect } from "@xyflow/react";

/** The side of a note's speech bubble that its tail leaves from. */
export type TailSide = "top" | "right" | "bottom" | "left";

/**
 * The side whose tail points from `note` to `box`, the box it hangs on: along
 * the axis on which the centres of the two rects lie farther apart, toward the
 * box, and on a tie the vertical side. Without a box, with equal centres, or
 * with a rect not yet measured, the tail points down.
 */
export function tailSide(note: Rect, box: Rect | undefined): TailSide {
  if (box === undefined) {
    return "bottom";
  }
  const right = box.x + box.width / 2 - (note.x + note.width / 2);
  const down = box.y + box.height / 2 - (note.y + note.height / 2);
  if (!Number.isFinite(right) || !Number.isFinite(down)) {
    return "bottom";
  }
  if (Math.abs(down) >= Math.abs(right)) {
    return down < 0 ? "top" : "bottom";
  }
  return right < 0 ? "left" : "right";
}
