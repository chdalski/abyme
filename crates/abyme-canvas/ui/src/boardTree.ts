import type { ListedBoard } from "./bindings";

/**
 * The level a row of the list stands at: its depth, or 0 for a depth that is
 * not a whole number of at least 0, so that no sequence the list holds can
 * hide a row for good.
 */
function levelOf(row: ListedBoard | undefined): number {
  const depth = row?.depth ?? 0;
  return Number.isInteger(depth) && depth > 0 ? depth : 0;
}

/** Whether the row at `index` has boards below it: the next row lies deeper. */
export function hasInner(rows: readonly ListedBoard[], index: number): boolean {
  return (
    index + 1 < rows.length && levelOf(rows[index + 1]) > levelOf(rows[index])
  );
}

/**
 * The boards above `board` in the list, nearest first: the rows before it
 * whose level falls step by step. Empty for a board the list does not hold.
 * One pass backwards from its first row.
 */
export function pathTo(
  rows: readonly ListedBoard[],
  board: string | null,
): ReadonlySet<string> {
  const index = rows.findIndex((row) => row.id === board);
  const above = new Set<string>();
  let level = levelOf(rows[index]);
  for (let before = index - 1; before >= 0 && level > 0; before -= 1) {
    const row = rows[before];
    if (row !== undefined && levelOf(row) < level) {
      above.add(row.id);
      level = levelOf(row);
    }
  }
  return above;
}

/**
 * The rows the list shows, each with whether it has boards below it: every
 * row but those below a closed branch. One pass; a row is hidden only while a
 * branch above it is closed.
 */
export function visibleRows(
  rows: readonly ListedBoard[],
  isOpen: (board: string) => boolean,
): { row: ListedBoard; inner: boolean }[] {
  const shown: { row: ListedBoard; inner: boolean }[] = [];
  let closedAt: number | null = null;
  rows.forEach((row, index) => {
    const level = levelOf(row);
    if (closedAt !== null && level > closedAt) {
      return;
    }
    closedAt = null;
    const inner = hasInner(rows, index);
    shown.push({ row, inner });
    if (inner && !isOpen(row.id)) {
      closedAt = level;
    }
  });
  return shown;
}
