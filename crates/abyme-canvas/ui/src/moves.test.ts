import { describe, expect, it } from "vitest";

import { toMove, toRelease } from "./moves";

describe("toMove", () => {
  it("moves and pins an existing element on an existing board at integer coordinates", () => {
    expect(toMove("context", "payments", { x: 40.4, y: -80.6 })).toEqual({
      op: "move_element",
      element: { existing: "payments" },
      board: { existing: "context" },
      x: 40,
      y: -81,
      pinned: true,
    });
  });
});

describe("toRelease", () => {
  it("releases the pin at the coordinates a move there would have", () => {
    const position = { x: 40.4, y: -80.6 };

    expect(toRelease("context", "payments", position)).toEqual({
      ...toMove("context", "payments", position),
      pinned: false,
    });
  });
});
