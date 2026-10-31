import { describe, expect, it } from "vitest";

import { toMove } from "./moves";

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
