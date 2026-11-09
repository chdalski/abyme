import type { BoardsState } from "./boards";

/**
 * How many levels the list indents at most: a deeper board, which a long chain
 * of inner boards in the files can make, lies as far in as this one.
 */
export const INDENT_LEVELS = 8;

/** How far a board at `depth` is indented, from the number alone. */
function indentOf(depth: number): string {
  return `${Math.min(depth, INDENT_LEVELS)}rem`;
}

/**
 * The project's boards as a tree, as `overview` lists them: each board below
 * the board it lies inside, indented by its depth; a click opens one. Titles
 * and messages come from the repository and are rendered as text only.
 */
export function BoardList({
  state,
  open,
  onOpen,
}: {
  state: BoardsState;
  open: string | null;
  onOpen: (board: string) => void;
}) {
  switch (state.kind) {
    case "loading":
      return <p>Loading the boards…</p>;
    case "failed":
      return <p role="alert">{state.message}</p>;
    case "loaded":
      if (state.boards.length === 0) {
        return <p>This project has no boards yet.</p>;
      }
      return (
        <ul aria-label="Boards">
          {state.boards.map((board) => (
            <li
              key={board.id}
              style={{ paddingInlineStart: indentOf(board.depth) }}
            >
              <button
                type="button"
                aria-current={board.id === open ? "page" : undefined}
                onClick={() => onOpen(board.id)}
              >
                {board.title}
              </button>
            </li>
          ))}
        </ul>
      );
  }
}
