import { useState } from "react";

import { pathTo, visibleRows } from "./boardTree";
import type { BoardsState } from "./boards";

/** A branch the user opened or closed, and when, by the count of opened boards. */
type Toggle = { readonly open: boolean; readonly at: number };

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
 * the board it lies inside, indented by its depth; a click opens one. A board
 * with boards below it has a toggle for its branch. Branches are closed but
 * those on the way to the open board, and opening a board opens its way again,
 * also a branch the user closed; what the user opened or closed stays so for
 * the session, also when the list loads again. Titles and messages come from
 * the repository and are rendered as text only; a title too long for the
 * sidebar is cut, and shown in full as its button's tooltip.
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
  // Kept above the list's states, so a failed reload keeps the user's toggles.
  const [toggles, setToggles] = useState<ReadonlyMap<string, Toggle>>(
    () => new Map(),
  );
  // Counts the boards opened, so that a toggle from before an opening gives
  // way to the way to the open board.
  const [opened, setOpened] = useState({ board: open, count: 0 });
  if (opened.board !== open) {
    setOpened({ board: open, count: opened.count + 1 });
  }

  switch (state.kind) {
    case "loading":
      return <p>Loading the boards…</p>;
    case "failed":
      return <p role="alert">{state.message}</p>;
    case "loaded":
      if (state.boards.length === 0) {
        return <p>This project has no boards yet.</p>;
      }
      {
        const onTheWay = pathTo(state.boards, open);
        const isOpen = (board: string) => {
          const toggle = toggles.get(board);
          if (onTheWay.has(board) && (toggle?.at ?? -1) < opened.count) {
            return true;
          }
          return toggle?.open ?? false;
        };
        const toggle = (board: string) => {
          setToggles((current) =>
            new Map(current).set(board, {
              open: !isOpen(board),
              at: opened.count,
            }),
          );
        };
        return (
          <ul aria-label="Boards">
            {visibleRows(state.boards, isOpen).map(({ row, inner }, index) => (
              <li
                // A list from broken files can name a board twice.
                key={`${index}:${row.id}`}
                style={{ paddingInlineStart: indentOf(row.depth) }}
              >
                {inner ? (
                  <button
                    type="button"
                    className="branch-toggle"
                    aria-expanded={isOpen(row.id)}
                    aria-label={isOpen(row.id) ? "Close branch" : "Open branch"}
                    onClick={() => toggle(row.id)}
                  >
                    {isOpen(row.id) ? "▾" : "▸"}
                  </button>
                ) : (
                  <span className="branch-toggle" aria-hidden="true" />
                )}
                <button
                  type="button"
                  className="board-link"
                  aria-current={row.id === open ? "page" : undefined}
                  title={row.title}
                  onClick={() => onOpen(row.id)}
                >
                  {row.title}
                </button>
              </li>
            ))}
          </ul>
        );
      }
  }
}
