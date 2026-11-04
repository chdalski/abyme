import { describe, expect, it } from "vitest";

import type { BoardView } from "./bindings";
import {
  type ElementNode,
  closeNoteField,
  draftNote,
  finishedMoves,
  finishedRemovals,
  keepUiState,
  onTheBoardAndHung,
  returnHome,
  toNodes,
  withNoteField,
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
        editing: false,
        leftOut: false,
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
        editing: false,
        leftOut: false,
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

    const kept = keepUiState(toNodes(VIEW), current, null);

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

    const kept = keepUiState(toNodes(VIEW), current, null);

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

        const [kept] = keepUiState(nodesWithPin(pinned), current, null);

        expect(kept?.data.pinned).toBe(pinned);
      }
    }
  });
});

describe("keepUiState and a new note", () => {
  const written = (
    draft: ElementNode,
    createdAs: string,
    writtenAt = 5,
  ): ElementNode => ({
    ...draft,
    data: { ...draft.data, draft: { createdAs, writtenAt } },
  });

  it("drops a written note that a view asked for after its write answered leaves out", () => {
    const deleted = written(draftNote("+note-1", { x: 1, y: 2 }), "gone", 5);
    const other = written(draftNote("+note-2", { x: 3, y: 4 }), "later", 9);
    const typing = draftNote("+note-3", { x: 5, y: 6 });

    const kept = keepUiState(toNodes(VIEW), [deleted, other, typing], 7);

    expect(kept.map((node) => node.id)).toEqual([
      "orders",
      "payments",
      "+note-2",
      "+note-3",
    ]);
  });

  it("keeps a written note through a view asked for before its write answered", () => {
    const draft = written(draftNote("+note-1", { x: 1, y: 2 }), "new-one", 5);

    for (const askedAt of [4, null]) {
      expect(
        keepUiState(toNodes(VIEW), [draft], askedAt).map((node) => node.id),
      ).toContain("+note-1");
    }
  });

  it("keeps a new note through a view that does not list it", () => {
    const draft = draftNote("+note-1", { x: 1, y: 2 });

    expect(
      keepUiState(toNodes(VIEW), [draft], null).map((node) => node.id),
    ).toEqual(["orders", "payments", "+note-1"]);
  });

  it("keeps a written note until a view lists its ID, and then drops it", () => {
    const draft = written(draftNote("+note-1", { x: 1, y: 2 }), "new-one");
    const without = keepUiState(toNodes(VIEW), [draft], null);
    expect(without.map((node) => node.id)).toContain("+note-1");

    const listing = boardView({ placements: [placedBox("new-one")] });
    expect(
      keepUiState(toNodes(listing), [draft], null).map((node) => node.id),
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

describe("a note a view leaves out while its field is open", () => {
  const BOX = {
    kind: "element",
    type_ref: "t",
    name: "B",
    summary: null,
  } as const;
  const note = (name: string) => ({ kind: "note", name, body: "" }) as const;
  /** The box `b` at (100, 100) and the note `n` hanging on it at (110, 120). */
  const first = boardView({
    placements: [
      placedBox("b", { x: 100, y: 100, shown: BOX }),
      placedBox("n", { x: 110, y: 120, shown: note("N"), attached_to: "b" }),
    ],
  });
  const withoutN = boardView({
    placements: [placedBox("b", { x: 100, y: 100, shown: BOX })],
  });
  /** `n` as a box at (400, 0), with the note `m` hanging on it. */
  const nAsABox = boardView({
    placements: [
      placedBox("b", { x: 100, y: 100, shown: BOX }),
      placedBox("n", { x: 400, y: 0, shown: BOX }),
      placedBox("m", { x: 410, y: 10, shown: note("M"), attached_to: "n" }),
    ],
  });
  const opened = (view: BoardView, element: string) =>
    withNoteField(toNodes(view), element, true).map((node) =>
      node.id === element ? { ...node, selected: true } : node,
    );
  const held = () => keepUiState(toNodes(withoutN), opened(first, "n"), null);

  it("is held where it lies on the board, hanging on nothing, and offers nothing but its field", () => {
    const n = held().find((node) => node.id === "n");

    expect(n?.position).toEqual({ x: 110, y: 120 });
    expect(n?.parentId).toBeUndefined();
    expect([n?.selected, n?.selectable, n?.draggable, n?.deletable]).toEqual([
      false,
      false,
      false,
      false,
    ]);
    expect(n?.data).toMatchObject({
      leftOut: true,
      editing: true,
      shown: note("N"),
    });
  });

  it("is held where it lay on the board when the box it hung on is gone too", () => {
    const kept = keepUiState(toNodes(boardView()), opened(first, "n"), null);

    expect(kept.map((node) => [node.id, node.position])).toEqual([
      ["n", { x: 110, y: 120 }],
    ]);
  });

  it("stays as it is through a later view that lists its ID, which holds back that box and what hangs on it", () => {
    const kept = keepUiState(toNodes(nAsABox), held(), null);

    expect(kept.map((node) => node.id)).toEqual(["b", "n"]);
    expect(kept.find((node) => node.id === "n")).toEqual(
      held().find((node) => node.id === "n"),
    );
  });

  it("is not held when its field is closed, when a view lists it, or when it is new", () => {
    const closed = keepUiState(toNodes(withoutN), toNodes(first), null);
    const listed = keepUiState(toNodes(first), opened(first, "n"), null);
    const unwritten = draftNote("+note-1", { x: 0, y: 0 });
    const written: ElementNode = {
      ...unwritten,
      data: { ...unwritten.data, draft: { createdAs: "n", writtenAt: 5 } },
    };
    const drafts = keepUiState(
      toNodes(withoutN),
      [...toNodes(withoutN), unwritten, written],
      null,
    );

    expect(closed.map((node) => node.id)).toEqual(["b"]);
    expect(listed.map((node) => [node.id, node.data.leftOut])).toEqual([
      ["b", false],
      ["n", false],
    ]);
    expect(drafts.map((node) => [node.id, node.data.leftOut])).toEqual([
      ["b", false],
      ["+note-1", false],
      ["+note-1", false],
    ]);
  });

  it("goes when its field closes and the latest view does not list its ID", () => {
    const closed = closeNoteField(held(), "n", toNodes(withoutN));

    expect(closed.map((node) => node.id)).toEqual(["b"]);
  });

  it("gives way to the box the latest view lists under its ID, with what hangs on it", () => {
    const closed = closeNoteField(held(), "n", toNodes(nAsABox));

    expect(
      closed.map((node) => [
        node.id,
        node.parentId,
        node.data.shown.kind,
        node.data.leftOut,
      ]),
    ).toEqual([
      ["b", undefined, "element", false],
      ["n", undefined, "element", false],
      ["m", "n", "note", false],
    ]);
  });

  it("brings back nothing that hangs on a note still held, until that note's field closes", () => {
    const both = boardView({
      placements: [
        placedBox("a", { shown: note("A") }),
        placedBox("c", { x: 300, shown: note("C") }),
      ],
    });
    const twoHeld = keepUiState(
      toNodes(boardView()),
      withNoteField(withNoteField(toNodes(both), "a", true), "c", true),
      null,
    );
    const cAsABox = toNodes(
      boardView({
        placements: [
          placedBox("c", { x: 300, shown: BOX }),
          placedBox("a", { x: 310, shown: note("A"), attached_to: "c" }),
        ],
      }),
    );

    const aClosed = closeNoteField(twoHeld, "a", cAsABox);
    const cClosed = closeNoteField(aClosed, "c", cAsABox);

    expect(aClosed.map((node) => [node.id, node.data.leftOut])).toEqual([
      ["c", true],
    ]);
    expect(cClosed.map((node) => [node.id, node.parentId])).toEqual([
      ["c", undefined],
      ["a", "c"],
    ]);
  });

  it("adds nothing when its field closes a second time", () => {
    const once = closeNoteField(held(), "n", toNodes(nAsABox));

    expect(closeNoteField(once, "n", toNodes(nAsABox))).toEqual(once);
  });

  it("only closes the field of a note no view left out", () => {
    const closed = closeNoteField(opened(first, "n"), "n", toNodes(withoutN));

    expect(closed.map((node) => [node.id, node.data.editing])).toEqual([
      ["b", false],
      ["n", false],
    ]);
  });
});
