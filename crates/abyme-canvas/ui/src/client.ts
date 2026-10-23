import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

import type { BoardEntry } from "./bindings";

/**
 * What the UI asks of the canvas's Rust side. The UI reaches it only through
 * this interface, so tests give the UI a fake one.
 */
export interface AbymeClient {
  /** The project's boards in board-ID order; rejects with the core's message. */
  listBoards(): Promise<BoardEntry[]>;
  /**
   * Calls `onChange` whenever the project's files change; resolves to a
   * function that stops listening.
   */
  onProjectChanged(onChange: () => void): Promise<() => void>;
}

/** The client over Tauri's IPC, with the command and the event of `src/app.rs`. */
export const tauriClient: AbymeClient = {
  listBoards: () => invoke<BoardEntry[]>("list_boards"),
  onProjectChanged: (onChange) => listen("project-changed", () => onChange()),
};
