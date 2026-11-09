import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

import type { BoardView, ElementFlag, ListedBoard } from "./bindings";

/**
 * Moving and pinning an element that exists on a board that exists, to integer
 * coordinates, or releasing its pin with `pinned: false`, which Rust admits only
 * at its position in the board file.
 */
export type AdmittedMove = {
  op: "move_element";
  element: { existing: string };
  board: { existing: string };
  x: number;
  y: number;
  pinned: boolean;
};

/**
 * Changing the text of an element that exists, which Rust admits only when its
 * file says it is a note.
 */
export type NoteUpdate = {
  op: "update_element";
  element: { existing: string };
  name: string;
  body: string;
};

/**
 * Deleting an element that exists, which Rust admits only when its file says it
 * is a note.
 */
export type NoteDeletion = {
  op: "delete_element";
  element: { existing: string };
};

/**
 * Hanging a note that exists on an element that exists on a board that exists,
 * where the note lies, which Rust admits only when its file says it is a note.
 * The canvas sends it right after its own move of the note, which pins it, so it
 * always confirms the pin.
 */
export type NoteAttachment = {
  op: "attach_note";
  note: { existing: string };
  to: { existing: string };
  board: { existing: string };
  pinned: true;
};

/**
 * Ending the attachment of a note that exists on a board that exists, where the
 * note lies, which Rust admits only when its file says it is a note.
 */
export type NoteDetachment = {
  op: "detach_note";
  note: { existing: string };
  board: { existing: string };
};

/**
 * An operation the canvas may send on its own: what `src/admission.rs` admits,
 * typed by hand.
 */
export type AdmittedOperation =
  AdmittedMove | NoteUpdate | NoteDeletion | NoteAttachment | NoteDetachment;

/**
 * The one request that creates an element: a note with its own statement kind
 * `idea`, placed on a board that exists and pinned where a person put it. Rust
 * admits it only as a whole.
 */
export type NewNote = [
  {
    op: "create_element";
    key: string;
    type_ref: "note";
    name: string;
    body: string;
    statement: "idea";
  },
  {
    op: "place_element";
    element: { new: string };
    board: { existing: string };
  },
  {
    op: "move_element";
    element: { new: string };
    board: { existing: string };
    x: number;
    y: number;
    pinned: true;
  },
];

/** A request the canvas may send. */
export type AdmittedRequest = AdmittedOperation[] | NewNote;

/**
 * What the UI asks of the canvas's Rust side. The UI reaches it only through
 * this interface, so tests give the UI a fake one.
 */
export interface AbymeClient {
  /** The project's boards as `overview` lists them, each with its depth; rejects with the core's message. */
  listBoards(): Promise<ListedBoard[]>;
  /** What the board `board` shows; rejects with the core's message. */
  openBoard(board: string): Promise<BoardView>;
  /**
   * Applies `operations` to the project, all or none, and resolves to the IDs
   * of the elements it created; rejects with the canvas's or the core's message.
   */
  apply(operations: AdmittedRequest): Promise<string[]>;
  /**
   * The project's flagged elements with their levels; rejects with why flags
   * are unavailable.
   */
  reviewFlags(): Promise<ElementFlag[]>;
  /**
   * Calls `onChange` whenever the project's files change; resolves to a
   * function that stops listening.
   */
  onProjectChanged(onChange: () => void): Promise<() => void>;
}

/** The client over Tauri's IPC, with the command and the event of `src/app.rs`. */
export const tauriClient: AbymeClient = {
  listBoards: () => invoke<ListedBoard[]>("list_boards"),
  openBoard: (board) => invoke<BoardView>("open_board", { board }),
  apply: (operations) => invoke<string[]>("apply", { operations }),
  reviewFlags: () => invoke<ElementFlag[]>("review_flags"),
  onProjectChanged: (onChange) => listen("project-changed", () => onChange()),
};
