import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

import type { BoardEntry, BoardView, ElementFlag } from "./bindings";

/**
 * An operation the canvas may send: the operations `src/admission.rs`
 * admits, typed by hand. Today that is moving and pinning an element that
 * exists on a board that exists, to integer coordinates, or releasing its pin
 * with `pinned: false`, which Rust admits only at its position in the board
 * file.
 */
export type AdmittedOperation = {
  op: "move_element";
  element: { existing: string };
  board: { existing: string };
  x: number;
  y: number;
  pinned: boolean;
};

/**
 * What the UI asks of the canvas's Rust side. The UI reaches it only through
 * this interface, so tests give the UI a fake one.
 */
export interface AbymeClient {
  /** The project's boards in board-ID order; rejects with the core's message. */
  listBoards(): Promise<BoardEntry[]>;
  /** What the board `board` shows; rejects with the core's message. */
  openBoard(board: string): Promise<BoardView>;
  /**
   * Applies `operations` to the project, all or none; rejects with the
   * canvas's or the core's message.
   */
  apply(operations: AdmittedOperation[]): Promise<void>;
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
  listBoards: () => invoke<BoardEntry[]>("list_boards"),
  openBoard: (board) => invoke<BoardView>("open_board", { board }),
  apply: async (operations) => {
    await invoke("apply", { operations });
  },
  reviewFlags: () => invoke<ElementFlag[]>("review_flags"),
  onProjectChanged: (onChange) => listen("project-changed", () => onChange()),
};
