import type { BoardEntry, BoardView } from "./bindings";
import type { AbymeClient, AdmittedOperation } from "./client";

/**
 * A client for tests: it lists `boards` and opens the boards in `views`,
 * or rejects where they hold a message instead. `applied` records every
 * request, which rejects with `applyFailure` when that is set. `hold` makes
 * the answers for a board wait until `release` is called, and `change`
 * plays a change of the project's files.
 */
export class FakeClient implements AbymeClient {
  boards: BoardEntry[] | string;
  views: Record<string, BoardView | string> = {};
  applied: AdmittedOperation[][] = [];
  applyFailure: string | null = null;
  listCalls = 0;
  openCalls: string[] = [];
  private listeners: (() => void)[] = [];
  private held = new Map<string, (() => void)[]>();

  constructor(boards: BoardEntry[] | string) {
    this.boards = boards;
  }

  listBoards(): Promise<BoardEntry[]> {
    this.listCalls += 1;
    return answer(this.boards);
  }

  async openBoard(board: string): Promise<BoardView> {
    this.openCalls.push(board);
    const waiting = this.held.get(board);
    if (waiting !== undefined) {
      await new Promise<void>((resolve) => waiting.push(resolve));
    }
    return answer(this.views[board] ?? `no board has the ID ${board}`);
  }

  apply(operations: AdmittedOperation[]): Promise<void> {
    this.applied.push(operations);
    return this.applyFailure === null
      ? Promise.resolve()
      : Promise.reject(this.applyFailure);
  }

  hold(board: string): void {
    this.held.set(board, []);
  }

  release(board: string): void {
    this.held.get(board)?.forEach((resolve) => resolve());
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
