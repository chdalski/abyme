import type { BoardsState } from "./boards";

/**
 * The project's boards by title; a click opens one. Titles and messages
 * come from the repository and are rendered as text only.
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
            <li key={board.id}>
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
