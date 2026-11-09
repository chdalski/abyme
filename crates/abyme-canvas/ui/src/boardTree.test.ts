import { describe, expect, it } from "vitest";

import type { ListedBoard } from "./bindings";
import { hasInner, pathTo, visibleRows } from "./boardTree";

function rows(...depths: number[]): ListedBoard[] {
  return depths.map((depth, index) => ({
    id: `b${index}`,
    title: `B${index}`,
    depth,
  }));
}

const allOpen = () => true;
const allClosed = () => false;
const ids = (shown: { row: ListedBoard }[]) => shown.map(({ row }) => row.id);

describe("the board tree", () => {
  it("knows which rows have boards below them", () => {
    const list = rows(0, 1, 2, 1, 0);

    expect(list.map((_, index) => hasInner(list, index))).toEqual([
      true,
      true,
      false,
      false,
      false,
    ]);
  });

  it("finds the boards above a board, nearest first", () => {
    const list = rows(0, 1, 2, 1, 2, 0);

    expect([...pathTo(list, "b4")]).toEqual(["b3", "b0"]);
    expect([...pathTo(list, "b0")]).toEqual([]);
  });

  it("finds no way to a board the list does not hold", () => {
    expect([...pathTo(rows(0, 1), "gone")]).toEqual([]);
    expect([...pathTo(rows(0, 1), null)]).toEqual([]);
  });

  it("hides the rows below a closed branch, and only those", () => {
    const list = rows(0, 1, 2, 0, 1);

    expect(ids(visibleRows(list, (board) => board !== "b0"))).toEqual([
      "b0",
      "b3",
      "b4",
    ]);
    expect(ids(visibleRows(list, allClosed))).toEqual(["b0", "b3"]);
    expect(ids(visibleRows(list, allOpen))).toEqual([
      "b0",
      "b1",
      "b2",
      "b3",
      "b4",
    ]);
  });

  it.each([
    ["starts deep", [2, 3, 0]],
    ["jumps more than one level", [0, 3, 1]],
    ["holds a negative depth", [0, -1, 1]],
    ["holds a fraction", [0, 1.5, 1]],
    ["holds NaN", [0, Number.NaN, 1]],
    ["holds Infinity", [0, Number.POSITIVE_INFINITY, 1]],
  ])(
    "shows every row of a list that %s once all branches are open, without failing",
    (_shape, depths) => {
      const list = rows(...depths);

      expect(ids(visibleRows(list, allOpen))).toEqual(
        list.map((row) => row.id),
      );
      expect(() => pathTo(list, "b2")).not.toThrow();
    },
  );

  it("treats a board named twice like any other row", () => {
    const list: ListedBoard[] = [
      { id: "twice", title: "Twice", depth: 0 },
      { id: "inner", title: "Inner", depth: 1 },
      { id: "twice", title: "Twice", depth: 0 },
    ];

    expect(ids(visibleRows(list, allOpen))).toEqual([
      "twice",
      "inner",
      "twice",
    ]);
    expect([...pathTo(list, "inner")]).toEqual(["twice"]);
  });

  it("walks a chain of ten thousand levels without recursion", () => {
    const list = rows(...Array.from({ length: 10_000 }, (_, depth) => depth));

    expect(pathTo(list, "b9999").size).toBe(9_999);
    expect(visibleRows(list, allOpen)).toHaveLength(10_000);
    expect(visibleRows(list, allClosed)).toHaveLength(1);
  });
});
