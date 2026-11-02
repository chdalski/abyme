import type { BoardEntry, BoardView, ElementFlag } from "./bindings";
import type { AbymeClient, AdmittedRequest } from "./client";

/**
 * A client for tests: it lists `boards` and opens the boards in `views`,
 * or rejects where they hold a message instead. `applied` records every
 * request, which rejects with `applyFailure` when that is set. It answers for
 * the review flags with `flags`, or rejects where that holds a message. `hold`
 * makes the answers for a board, or for `FLAGS`, wait until `release` is
 * called, and `change` plays a change of the project's files.
 */
export class FakeClient implements AbymeClient {
  /** What `hold` and `release` take for the answers on review flags; no board can have this ID. */
  static readonly FLAGS = "review_flags";
  boards: BoardEntry[] | string;
  views: Record<string, BoardView | string> = {};
  applied: AdmittedRequest[] = [];
  /** What a request that creates a note answers. */
  created = "new-note";
  applyFailure: string | null = null;
  flags: ElementFlag[] | string = [];
  flagCalls = 0;
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

  apply(operations: AdmittedRequest): Promise<string[]> {
    this.applied.push(operations);
    return this.applyFailure === null
      ? Promise.resolve(
          operations[0]?.op === "create_element" ? [this.created] : [],
        )
      : Promise.reject(this.applyFailure);
  }

  async reviewFlags(): Promise<ElementFlag[]> {
    this.flagCalls += 1;
    const waiting = this.held.get(FakeClient.FLAGS);
    if (waiting !== undefined) {
      await new Promise<void>((resolve) => waiting.push(resolve));
    }
    return answer(this.flags);
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
