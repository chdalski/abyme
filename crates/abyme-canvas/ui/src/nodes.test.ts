import { describe, expect, it } from "vitest";

import type { BoardView } from "./bindings";
import { finishedMoves, keepUiState, returnHome, toNodes } from "./nodes";

const VIEW: BoardView = {
  id: "context",
  title: "Context",
  mode: null,
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

  it("makes boxes that can be dragged but not connected", () => {
    expect(
      toNodes(VIEW).every(
        (node) => node.draggable === true && node.connectable === false,
      ),
    ).toBe(true);
  });

  it("makes every box draggable except on a mirrored board", () => {
    const draggable = (mode: BoardView["mode"]) =>
      toNodes({ ...VIEW, mode }).map((node) => node.draggable);

    expect(draggable("mirrored")).toEqual([false, false]);
    expect(draggable("free")).toEqual([true, true]);
    expect(draggable(null)).toEqual([true, true]);
  });

  it("hands the projection's content to the box", () => {
    expect(toNodes(VIEW).map((node) => node.data)).toEqual([
      {
        element: "orders",
        shown: { kind: "missing" },
        home: { x: -20, y: 300 },
      },
      {
        element: "payments",
        shown: {
          kind: "element",
          type_ref: "__proto__",
          name: "Payments",
          summary: null,
        },
        home: { x: 40, y: 80 },
      },
    ]);
  });
});

describe("keepUiState", () => {
  it("leaves a dragged box where the drag holds it and takes the rest from the board file", () => {
    const current = toNodes(VIEW).map((node) =>
      node.id === "payments"
        ? { ...node, position: { x: 1, y: 2 }, dragging: true }
        : { ...node, position: { x: 3, y: 4 } },
    );

    const kept = keepUiState(toNodes(VIEW), current);

    expect(kept.map((node) => [node.id, node.position, node.dragging])).toEqual(
      [
        ["orders", { x: -20, y: 300 }, undefined],
        ["payments", { x: 1, y: 2 }, true],
      ],
    );
  });

  it("keeps a selected box selected, at its position from the board file", () => {
    const current = toNodes(VIEW).map((node) =>
      node.id === "orders"
        ? { ...node, position: { x: 3, y: 4 }, selected: true }
        : node,
    );

    const kept = keepUiState(toNodes(VIEW), current);

    expect(kept.map((node) => [node.id, node.position, node.selected])).toEqual(
      [
        ["orders", { x: -20, y: 300 }, true],
        ["payments", { x: 40, y: 80 }, undefined],
      ],
    );
  });
});

describe("returnHome", () => {
  it("puts the named boxes back at their positions in the board file", () => {
    const moved = toNodes(VIEW).map((node) => ({
      ...node,
      position: { x: 9, y: 9 },
    }));

    const returned = returnHome(moved, ["payments"]);

    expect(returned.map((node) => [node.id, node.position])).toEqual([
      ["orders", { x: 9, y: 9 }],
      ["payments", { x: 40, y: 80 }],
    ]);
  });
});

describe("finishedMoves", () => {
  it("takes the position changes that end a move, and nothing else", () => {
    expect(
      finishedMoves([
        {
          type: "position",
          id: "payments",
          position: { x: 1, y: 2 },
          dragging: true,
        },
        {
          type: "position",
          id: "orders",
          position: { x: 3, y: 4 },
          dragging: false,
        },
        { type: "select", id: "payments", selected: true },
        { type: "position", id: "payments", dragging: false },
      ]),
    ).toEqual([{ element: "orders", position: { x: 3, y: 4 } }]);
  });
});
