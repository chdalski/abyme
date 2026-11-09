import type { XYPosition } from "@xyflow/react";

import type { NewFreeNote, NewNote, NoteDeletion, NoteUpdate } from "./client";

/** What a note's text is made of. */
export type NoteParts = { name: string; body: string };

/** The line breaks `src/admission.rs` keeps out of a note's name. */
const LINE_BREAK = /\r\n|[\n\r\u0085\u2028\u2029]/;

/** A note's whole text: its name and, when it has one, a line break and its body. */
export function noteText(name: string, body: string): string {
  return body === "" ? name : `${name}\n${body}`;
}

/**
 * The name and body of typed text, or `null` for blank text. Blank lines at its
 * start and whitespace at its end are dropped; the first line left, without
 * whitespace at its ends, is the name, and the rest is the body. Rust checks the
 * result again and trusts none of this.
 */
export function splitNoteText(text: string): NoteParts | null {
  const stripped = text.replace(/^[\s\u0085]+|[\s\u0085]+$/g, "");
  if (stripped === "") {
    return null;
  }
  const lineBreak = LINE_BREAK.exec(stripped);
  return lineBreak === null
    ? { name: stripped, body: "" }
    : {
        name: stripped.slice(0, lineBreak.index).trimEnd(),
        body: stripped.slice(lineBreak.index + lineBreak[0].length),
      };
}

/** The request's name for the element it creates; it reaches no file. */
const NEW_NOTE_KEY = "note";

/**
 * The request that creates a note with `parts` on `board` at `position`, rounded
 * to the integers the board file holds, and pinned: a person placed it there;
 * and that hangs it on the element `hangOn`, if one is given.
 */
export function toNewNote(
  board: string,
  position: XYPosition,
  parts: NoteParts,
  hangOn: string | null,
): NewNote {
  const element = { new: NEW_NOTE_KEY };
  const on = { existing: board };
  const free: NewFreeNote = [
    {
      op: "create_element",
      key: NEW_NOTE_KEY,
      type_ref: "note",
      name: parts.name,
      body: parts.body,
      statement: "idea",
    },
    { op: "place_element", element, board: on },
    {
      op: "move_element",
      element,
      board: on,
      x: Math.round(position.x),
      y: Math.round(position.y),
      pinned: true,
    },
  ];
  return hangOn === null
    ? free
    : [
        ...free,
        {
          op: "attach_note",
          note: element,
          to: { existing: hangOn },
          board: on,
          pinned: true,
        },
      ];
}

/** The operation that deletes the note `element`. */
export function toNoteDeletion(element: string): NoteDeletion {
  return { op: "delete_element", element: { existing: element } };
}

/** The operation that writes `parts` as the text of the note `element`. */
export function toNoteUpdate(element: string, parts: NoteParts): NoteUpdate {
  return {
    op: "update_element",
    element: { existing: element },
    name: parts.name,
    body: parts.body,
  };
}
