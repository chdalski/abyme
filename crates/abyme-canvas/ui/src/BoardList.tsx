import type { BoardsState } from "./boards";

/**
 * The project's boards by title. Titles and messages come from the
 * repository and are rendered as text only.
 */
export function BoardList({ state }: { state: BoardsState }) {
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
            <li key={board.id}>{board.title}</li>
          ))}
        </ul>
      );
  }
}
