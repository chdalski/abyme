import type { XYPosition } from "@xyflow/react";

import type { AdmittedMove, NoteAttachment, NoteDetachment } from "./client";

/** The operation that moves and pins `element` on `board` at `position`, rounded to the integers the board file holds: a person placed it there. */
export function toMove(
  board: string,
  element: string,
  position: XYPosition,
): AdmittedMove {
  return placeAt(board, element, position, true);
}

/** The operation that releases the pin of `element` on `board`, which lies at `position`, rounded as a move rounds it. */
export function toRelease(
  board: string,
  element: string,
  position: XYPosition,
): AdmittedMove {
  return placeAt(board, element, position, false);
}

function placeAt(
  board: string,
  element: string,
  position: XYPosition,
  pinned: boolean,
): AdmittedMove {
  return {
    op: "move_element",
    element: { existing: element },
    board: { existing: board },
    x: Math.round(position.x),
    y: Math.round(position.y),
    pinned,
  };
}

/** The operation that hangs `note` on `to` on `board`, where the note lies, confirming its pin. */
export function toAttachment(
  board: string,
  note: string,
  to: string,
): NoteAttachment {
  return {
    op: "attach_note",
    note: { existing: note },
    to: { existing: to },
    board: { existing: board },
    pinned: true,
  };
}

/** The operation that ends the attachment of `note` on `board`, where the note lies. */
export function toDetachment(board: string, note: string): NoteDetachment {
  return {
    op: "detach_note",
    note: { existing: note },
    board: { existing: board },
  };
}
