import type { BoardEntry, BoardView } from "./bindings";
import type { AbymeClient } from "./client";

/**
 * A client for tests: it lists `boards` and opens the boards in `views`,
 * or rejects where they hold a message instead. `hold` makes the answer for
 * a board wait until `release` is called, and `change` plays a change of the
 * project's files.
 */
export class FakeClient implements AbymeClient {
  boards: BoardEntry[] | string;
  views: Record<string, BoardView | string> = {};
  listCalls = 0;
  openCalls: string[] = [];
  private listeners: (() => void)[] = [];
  private held = new Map<string, () => void>();

  constructor(boards: BoardEntry[] | string) {
    this.boards = boards;
  }

  listBoards(): Promise<BoardEntry[]> {
    this.listCalls += 1;
    return answer(this.boards);
  }

  async openBoard(board: string): Promise<BoardView> {
    this.openCalls.push(board);
    if (this.held.has(board)) {
      await new Promise<void>((resolve) => this.held.set(board, resolve));
    }
    return answer(this.views[board] ?? `no board has the ID ${board}`);
  }

  hold(board: string): void {
    this.held.set(board, () => {});
  }

  release(board: string): void {
    this.held.get(board)?.();
    this.held.delete(board);
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

function answer<T>(value: T | string): Promise<T> {
  return typeof value === "string"
    ? Promise.reject(value)
    : Promise.resolve(value);
}
