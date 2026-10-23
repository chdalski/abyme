import { describe, expect, it } from "vitest";

import type { BoardView } from "./bindings";
import { toNodes } from "./nodes";

const VIEW: BoardView = {
  id: "context",
  title: "Context",
  placements: [
    {
      element: "orders",
      x: -20,
      y: 300,
      width: 200,
      height: 100,
      shown: { kind: "missing" },
    },
    {
      element: "payments",
      x: 40,
      y: 80,
      width: 180,
      height: 90,
      shown: {
        kind: "element",
        type_ref: "__proto__",
        name: "Payments",
        summary: null,
      },
    },
  ],
};

describe("toNodes", () => {
  it("makes one box per placement at its rect from the board file", () => {
    expect(
      toNodes(VIEW).map((node) => [
        node.id,
        node.position,
        node.width,
        node.height,
      ]),
    ).toEqual([
      ["orders", { x: -20, y: 300 }, 200, 100],
      ["payments", { x: 40, y: 80 }, 180, 90],
    ]);
  });

  it("gives every box the one node type, whatever the element's type", () => {
    expect(toNodes(VIEW).map((node) => node.type)).toEqual([
      "element",
      "element",
    ]);
  });

  it("makes boxes that can neither be dragged nor connected", () => {
    expect(
      toNodes(VIEW).every(
        (node) => node.draggable === false && node.connectable === false,
      ),
    ).toBe(true);
  });

  it("hands the projection's content to the box", () => {
    expect(toNodes(VIEW).map((node) => node.data)).toEqual([
      { element: "orders", shown: { kind: "missing" } },
      {
        element: "payments",
        shown: {
          kind: "element",
          type_ref: "__proto__",
          name: "Payments",
          summary: null,
        },
      },
    ]);
  });
});
