import { describe, expect, it } from "vitest";

import type { BoardView } from "./bindings";
import {
  type ElementNode,
  draftNote,
  finishedMoves,
  finishedRemovals,
  keepUiState,
  onTheBoardAndHung,
  returnHome,
  toNodes,
} from "./nodes";
import { boardView, placedBox, placedElement } from "./testBoards";

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
    const view = boardView({
      placements: [placedElement("orders"), placedElement("payments")],
    });

    expect(
      toNodes(view).every(
        (node) => node.draggable === true && node.connectable === false,
      ),
    ).toBe(true);
  });

  it("makes a box that is no note draggable except on a mirrored board", () => {
    const draggable = (mode: BoardView["mode"]) =>
      toNodes(
        boardView({
          mode,
          placements: [placedElement("orders"), placedElement("payments")],
        }),
      ).map((node) => node.draggable);

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
            placedElement("plain"),
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
        attachedTo: null,
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
        attachedTo: null,
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

describe("toNodes and deleting", () => {
  it("makes only a note deletable", () => {
    const view = boardView({
      placements: [
        placedBox("plain"),
        placedBox("ask", { shown: { kind: "note", name: "Ask", body: "" } }),
      ],
    });

    expect(toNodes(view).map((node) => [node.id, node.deletable])).toEqual([
      ["plain", false],
      ["ask", true],
    ]);
  });
});

describe("finishedRemovals", () => {
  it("takes each removed element once, and no new note", () => {
    expect(
      finishedRemovals([
        { type: "remove", id: "ask" },
        { type: "remove", id: "ask" },
        { type: "remove", id: "+note-1" },
        { type: "select", id: "plain", selected: true },
        { type: "remove", id: "other" },
      ]),
    ).toEqual(["ask", "other"]);
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

const NOTE = { kind: "note", name: "Ask", body: "" } as const;
const BOX = {
  kind: "element",
  type_ref: "c4/container",
  name: "Box",
  summary: null,
} as const;

/**
 * The boxes `b` at 0, 0 and `c` at 400, 0, the box `gone` at 800, 0 whose
 * file is missing, the note `other` at 0, 300, and the note `n`, which hangs
 * on `b` at 20, 10, all 180 by 90.
 */
const WITH_A_NOTE: BoardView = boardView({
  placements: [
    placedBox("b", { shown: BOX }),
    placedBox("c", { x: 400, shown: BOX }),
    placedBox("gone", { x: 800 }),
    placedBox("n", { x: 20, y: 10, shown: NOTE, attached_to: "b" }),
    placedBox("other", { y: 300, shown: NOTE }),
  ],
});

describe("an attached note", () => {
  it("is the child of its box, at its position relative to the box, after its box", () => {
    const nodes = toNodes(
      boardView({
        placements: [
          placedBox("a-note", { x: 120, y: 90, shown: NOTE, attached_to: "z" }),
          placedBox("z", { x: 100, y: 50, shown: BOX }),
        ],
      }),
    );

    expect(nodes.map((node) => node.id)).toEqual(["z", "a-note"]);
    const [box, note] = nodes;
    expect(box?.parentId).toBeUndefined();
    expect(note?.parentId).toBe("z");
    expect(note?.position).toEqual({ x: 20, y: 40 });
    expect(note?.data.home).toEqual({ x: 20, y: 40 });
    expect(note?.data.attachedTo).toBe("z");
  });
});

describe("onTheBoardAndHung", () => {
  const hung = (element: string, position: { x: number; y: number }) =>
    onTheBoardAndHung([{ element, position }], toNodes(WITH_A_NOTE));

  it("gives a child note's move its position on the board", () => {
    expect(hung("n", { x: 30, y: 20 })).toEqual([
      { element: "n", position: { x: 30, y: 20 } },
    ]);
  });

  it("hangs a note dropped with its centre on another box there", () => {
    // Relative to `b`: the centre lands at 400 + 90, 0 + 45.
    expect(hung("n", { x: 400, y: 0 })).toEqual([
      {
        element: "n",
        position: { x: 400, y: 0 },
        hang: { attachTo: "c" },
      },
    ]);
    expect(hung("other", { x: 320, y: -40 })).toEqual([
      {
        element: "other",
        position: { x: 320, y: -40 },
        hang: { attachTo: "c" },
      },
    ]);
  });

  it("takes a note dragged off its box onto no other off it", () => {
    expect(hung("n", { x: 0, y: 500 })).toEqual([
      { element: "n", position: { x: 0, y: 500 }, hang: "detach" },
    ]);
  });

  it("hangs nothing on a box whose file is missing, or on a note", () => {
    expect(hung("n", { x: 800, y: 0 })).toEqual([
      { element: "n", position: { x: 800, y: 0 }, hang: "detach" },
    ]);
    expect(hung("n", { x: 0, y: 290 })).toEqual([
      { element: "n", position: { x: 0, y: 290 }, hang: "detach" },
    ]);
  });

  it("hangs nothing new when the note stays on its box", () => {
    expect(hung("n", { x: 50, y: 5 })).toEqual([
      { element: "n", position: { x: 50, y: 5 } },
    ]);
  });

  it("hangs a note on the box drawn on top where boxes overlap", () => {
    const overlapping = boardView({
      placements: [
        placedBox("low", { shown: BOX }),
        placedBox("top", { x: 50, shown: BOX }),
        placedBox("n", { y: 300, shown: NOTE }),
      ],
    });

    expect(
      onTheBoardAndHung(
        [{ element: "n", position: { x: 40, y: 0 } }],
        toNodes(overlapping),
      ),
    ).toEqual([
      { element: "n", position: { x: 40, y: 0 }, hang: { attachTo: "top" } },
    ]);
  });

  it("hangs no box anywhere", () => {
    expect(hung("c", { x: 0, y: 0 })).toEqual([
      { element: "c", position: { x: 0, y: 0 } },
    ]);
  });
});

describe("a note on a box it could not be attached to now", () => {
  it("stays there while its centre lies on the box", () => {
    const view = boardView({
      placements: [
        placedBox("gone"),
        placedBox("n", { x: 10, y: 10, shown: NOTE, attached_to: "gone" }),
      ],
    });

    expect(
      onTheBoardAndHung(
        [{ element: "n", position: { x: 15, y: 10 } }],
        toNodes(view),
      ),
    ).toEqual([{ element: "n", position: { x: 15, y: 10 } }]);
    expect(
      onTheBoardAndHung(
        [{ element: "n", position: { x: 0, y: 500 } }],
        toNodes(view),
      ),
    ).toEqual([{ element: "n", position: { x: 0, y: 500 }, hang: "detach" }]);
  });
});

describe("a note on its box under another box", () => {
  it("moves to the box drawn on top", () => {
    const view = boardView({
      placements: [
        placedBox("low", { shown: BOX }),
        placedBox("top", { x: 50, shown: BOX }),
        placedBox("n", { x: 40, y: 0, shown: NOTE, attached_to: "low" }),
      ],
    });

    expect(
      onTheBoardAndHung(
        [{ element: "n", position: { x: 45, y: 0 } }],
        toNodes(view),
      ),
    ).toEqual([
      { element: "n", position: { x: 45, y: 0 }, hang: { attachTo: "top" } },
    ]);
  });
});

describe("a box whose file is missing", () => {
  it("cannot be dragged on any board and keeps its pin", () => {
    for (const mode of [null, "free", "mirrored"] as const) {
      const [node] = toNodes(
        boardView({ mode, placements: [placedBox("gone", { pinned: true })] }),
      );

      expect(node?.draggable).toBe(false);
      expect(node?.data.pinned).toBe(true);
    }
  });
});
