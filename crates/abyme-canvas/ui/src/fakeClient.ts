import type { BoardEntry } from "./bindings";
import type { AbymeClient } from "./client";

/**
 * A client for tests: it lists `boards`, or rejects with `boards` when that
 * is a message, and `change` plays a change of the project's files.
 */
export class FakeClient implements AbymeClient {
  boards: BoardEntry[] | string;
  listCalls = 0;
  private listeners: (() => void)[] = [];

  constructor(boards: BoardEntry[] | string) {
    this.boards = boards;
  }

  listBoards(): Promise<BoardEntry[]> {
    this.listCalls += 1;
    const boards = this.boards;
    return typeof boards === "string"
      ? Promise.reject(boards)
      : Promise.resolve(boards);
  }

  onProjectChanged(onChange: () => void): Promise<() => void> {
    this.listeners.push(onChange);
    return Promise.resolve(() => {
      this.listeners = this.listeners.filter(
        (listener) => listener !== onChange,
      );
    });
  }

  get listening(): number {
    return this.listeners.length;
  }

  change(): void {
    this.listeners.forEach((listener) => listener());
  }
}
