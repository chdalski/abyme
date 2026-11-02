import { describe, expect, it } from "vitest";

import type { BoardView } from "./bindings";
import {
  type ElementNode,
  draftNote,
  finishedMoves,
  keepUiState,
  returnHome,
  toNodes,
} from "./nodes";
import { boardView, placedBox } from "./testBoards";

const VIEW: BoardView = boardView({
  placements: [
    placedBox("orders", { x: -20, y: 300, width: 200, height: 100 }),
    placedBox("payments", {
      x: 40,
      y: 80,
      shown: {
        kind: "element",
        type_ref: "__proto__",
        name: "Payments",
        summary: null,
      },
    }),
  ],
});

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

  it("makes a box that is no note draggable except on a mirrored board", () => {
    const draggable = (mode: BoardView["mode"]) =>
      toNodes({ ...VIEW, mode }).map((node) => node.draggable);

    expect(draggable("mirrored")).toEqual([false, false]);
    expect(draggable("free")).toEqual([true, true]);
    expect(draggable(null)).toEqual([true, true]);
  });

  it("makes only a note draggable on a mirrored board", () => {
    const draggable = (mode: BoardView["mode"]) =>
      toNodes(
        boardView({
          mode,
          placements: [
            placedBox("plain"),
            placedBox("ask", {
              shown: { kind: "note", name: "Ask", body: "" },
            }),
          ],
        }),
      ).map((node) => [node.id, node.draggable]);

    expect(draggable("mirrored")).toEqual([
      ["plain", false],
      ["ask", true],
    ]);
    expect(draggable("free")).toEqual([
      ["plain", true],
      ["ask", true],
    ]);
  });

  it("hands the projection's content to the box", () => {
    expect(toNodes(VIEW).map((node) => node.data)).toEqual([
      {
        element: "orders",
        shown: { kind: "missing" },
        home: { x: -20, y: 300 },
        innerBoards: [],
        pinned: false,
        draft: null,
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
        innerBoards: [],
        pinned: false,
        draft: null,
      },
    ]);
  });

  it("hands the pin of each placement to its box", () => {
    const view = boardView({
      placements: [placedBox("free"), placedBox("held", { pinned: true })],
    });

    expect(toNodes(view).map((node) => node.data.pinned)).toEqual([
      false,
      true,
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

  const nodesWithPin = (pinned: boolean) =>
    toNodes(boardView({ placements: [placedBox("kept", { pinned })] }));

  it("takes the pin from the file, also for a box the user holds or selected", () => {
    for (const held of [{ dragging: true }, { selected: true }]) {
      for (const pinned of [true, false]) {
        const current = nodesWithPin(!pinned).map((node) => ({
          ...node,
          ...held,
        }));

        const [kept] = keepUiState(nodesWithPin(pinned), current);

        expect(kept?.data.pinned).toBe(pinned);
      }
    }
  });
});

describe("keepUiState and a new note", () => {
  const written = (draft: ElementNode, createdAs: string): ElementNode => ({
    ...draft,
    data: { ...draft.data, draft: { createdAs } },
  });

  it("keeps a new note through a view that does not list it", () => {
    const draft = draftNote("+note-1", { x: 1, y: 2 });

    expect(keepUiState(toNodes(VIEW), [draft]).map((node) => node.id)).toEqual([
      "orders",
      "payments",
      "+note-1",
    ]);
  });

  it("keeps a written note until a view lists its ID, and then drops it", () => {
    const draft = written(draftNote("+note-1", { x: 1, y: 2 }), "new-one");
    const without = keepUiState(toNodes(VIEW), [draft]);
    expect(without.map((node) => node.id)).toContain("+note-1");

    const listing = boardView({ placements: [placedBox("new-one")] });
    expect(
      keepUiState(toNodes(listing), [draft]).map((node) => node.id),
    ).toEqual(["new-one"]);
  });

  it("gives a new note no way to be selected or dragged", () => {
    const draft = draftNote("+note-1", { x: 1, y: 2 });

    expect([draft.selectable, draft.draggable, draft.deletable]).toEqual([
      false,
      false,
      false,
    ]);
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
