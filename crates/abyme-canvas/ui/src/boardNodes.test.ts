import { act, renderHook, waitFor } from "@testing-library/react";
import type { XYPosition } from "@xyflow/react";
import { describe, expect, it } from "vitest";

import type { BoardView } from "./bindings";
import { useBoardNodes } from "./boardNodes";
import type { ElementNode, Move } from "./nodes";
import { boardView, placedBox } from "./testBoards";

function view(payments: XYPosition, orders: XYPosition): BoardView {
  return boardView({
    placements: [placedBox("orders", orders), placedBox("payments", payments)],
  });
}

function positions(nodes: ElementNode[]): Record<string, XYPosition> {
  return Object.fromEntries(nodes.map((node) => [node.id, node.position]));
}

function setup(move: (moves: Move[]) => Promise<void>) {
  const failures: string[] = [];
  const hook = renderHook(
    ({ board }: { board: BoardView }) =>
      useBoardNodes(
        board,
        null,
        move,
        (message) => failures.push(message),
        () => {},
      ),
    { initialProps: { board: view({ x: 0, y: 0 }, { x: 300, y: 0 }) } },
  );
  return { hook, failures };
}

type Hook = ReturnType<typeof setup>["hook"];

/** Plays the position changes React Flow reports, one call per entry. */
function report(hook: Hook, ...changes: [string, XYPosition, boolean][][]) {
  changes.forEach((batch) => {
    act(() => {
      hook.result.current.onNodesChange(
        batch.map(([id, position, dragging]) => ({
          type: "position",
          id,
          position,
          dragging,
        })),
      );
    });
  });
}

describe("useBoardNodes", () => {
  it("sends one move when a drag of one box ends", () => {
    const calls: Move[][] = [];
    const { hook } = setup((moves) => {
      calls.push(moves);
      return Promise.resolve();
    });

    report(
      hook,
      [["payments", { x: 20, y: 40 }, true]],
      [["payments", { x: 40.4, y: 80.6 }, true]],
      [["payments", { x: 40.4, y: 80.6 }, false]],
    );

    expect(calls).toEqual([
      [{ element: "payments", position: { x: 40.4, y: 80.6 } }],
    ]);
  });

  it("sends the moves of several boxes dragged together in one call", () => {
    const calls: Move[][] = [];
    const { hook } = setup((moves) => {
      calls.push(moves);
      return Promise.resolve();
    });

    report(
      hook,
      [
        ["payments", { x: 10, y: 10 }, true],
        ["orders", { x: 310, y: 10 }, true],
      ],
      [
        ["payments", { x: 10, y: 10 }, false],
        ["orders", { x: 310, y: 10 }, false],
      ],
    );

    expect(calls).toEqual([
      [
        { element: "payments", position: { x: 10, y: 10 } },
        { element: "orders", position: { x: 310, y: 10 } },
      ],
    ]);
  });

  it("sends a move made with the arrow keys", () => {
    const calls: Move[][] = [];
    const { hook } = setup((moves) => {
      calls.push(moves);
      return Promise.resolve();
    });

    report(hook, [["payments", { x: 5, y: 0 }, false]]);

    expect(calls).toEqual([
      [{ element: "payments", position: { x: 5, y: 0 } }],
    ]);
  });

  it("sends a move back to where the box started before the earlier move has reloaded", () => {
    const calls: Move[][] = [];
    const { hook } = setup((moves) => {
      calls.push(moves);
      return Promise.resolve();
    });

    report(
      hook,
      [["payments", { x: 5, y: 0 }, false]],
      [["payments", { x: 0, y: 0 }, false]],
    );

    expect(calls).toEqual([
      [{ element: "payments", position: { x: 5, y: 0 } }],
      [{ element: "payments", position: { x: 0, y: 0 } }],
    ]);
  });

  it("returns every moved box to its position in the board file when the moves fail", async () => {
    const { hook, failures } = setup(() =>
      Promise.reject("operation 2: x is out of range"),
    );

    report(hook, [
      ["payments", { x: 40, y: 80 }, false],
      ["orders", { x: 1_000_001, y: 0 }, false],
    ]);

    await waitFor(() =>
      expect(failures).toEqual(["operation 2: x is out of range"]),
    );
    expect(positions(hook.result.current.nodes)).toEqual({
      orders: { x: 300, y: 0 },
      payments: { x: 0, y: 0 },
    });
  });

  it("keeps a dragged box where the drag holds it when the board reloads", () => {
    const { hook } = setup(() => Promise.resolve());

    report(hook, [["payments", { x: 40, y: 80 }, true]]);
    hook.rerender({ board: view({ x: 0, y: 0 }, { x: 500, y: 100 }) });

    expect(positions(hook.result.current.nodes)).toEqual({
      orders: { x: 500, y: 100 },
      payments: { x: 40, y: 80 },
    });
  });

  it("keeps a box selected across a reload, so the arrow keys keep moving it", () => {
    const { hook } = setup(() => Promise.resolve());

    act(() => {
      hook.result.current.onNodesChange([
        { type: "select", id: "payments", selected: true },
      ]);
    });
    report(hook, [["payments", { x: 5, y: 0 }, false]]);
    hook.rerender({ board: view({ x: 5, y: 0 }, { x: 300, y: 0 }) });

    const payments = hook.result.current.nodes.find(
      (node) => node.id === "payments",
    );
    expect(payments?.selected).toBe(true);
    expect(payments?.position).toEqual({ x: 5, y: 0 });
  });

  it("takes the board file's positions on a reload once no box is dragged", () => {
    const { hook } = setup(() => Promise.resolve());

    report(hook, [["payments", { x: 40, y: 80 }, false]]);
    hook.rerender({ board: view({ x: 40, y: 80 }, { x: 500, y: 100 }) });

    expect(positions(hook.result.current.nodes)).toEqual({
      orders: { x: 500, y: 100 },
      payments: { x: 40, y: 80 },
    });
  });
});

describe("two batches of changes before React renders", () => {
  it("keeps both", () => {
    const { hook } = setup(() => Promise.resolve());

    act(() => {
      hook.result.current.onNodesChange([
        {
          type: "position",
          id: "payments",
          position: { x: 7, y: 8 },
          dragging: true,
        },
      ]);
      hook.result.current.onNodesChange([
        {
          type: "position",
          id: "orders",
          position: { x: 9, y: 10 },
          dragging: true,
        },
      ]);
    });

    expect(positions(hook.result.current.nodes)).toEqual({
      orders: { x: 9, y: 10 },
      payments: { x: 7, y: 8 },
    });
  });
});

describe("a note a view left out while its field was open", () => {
  const NOTE = { kind: "note", name: "N", body: "" } as const;
  const asBox = (name: string) =>
    boardView({
      placements: [
        placedBox("n", {
          shown: { kind: "element", type_ref: "t", name, summary: null },
        }),
      ],
    });

  /** The note `n` with its field open, held by a view that leaves it out. */
  function held() {
    const moves: Move[][] = [];
    const removed: string[][] = [];
    const hook = renderHook(
      ({ board }: { board: BoardView }) =>
        useBoardNodes(
          board,
          null,
          (sent) => {
            moves.push(sent);
            return Promise.resolve();
          },
          () => {},
          (elements) => removed.push(elements),
        ),
      {
        initialProps: {
          board: boardView({ placements: [placedBox("n", { shown: NOTE })] }),
        },
      },
    );
    act(() => {
      hook.result.current.noteFields.open("n");
    });
    hook.rerender({ board: boardView() });
    return { hook, moves, removed };
  }

  it("passes on neither a selection, a move, nor a removal React Flow reports for it", () => {
    const { hook, moves, removed } = held();

    act(() => {
      hook.result.current.onNodesChange([
        { type: "select", id: "n", selected: true },
        {
          type: "position",
          id: "n",
          position: { x: 5, y: 5 },
          dragging: false,
        },
        { type: "remove", id: "n" },
      ]);
    });

    expect(moves).toEqual([]);
    expect(removed).toEqual([]);
    expect(
      hook.result.current.nodes.map((node) => [
        node.id,
        node.position,
        node.selected,
        node.data.leftOut,
      ]),
    ).toEqual([["n", { x: 0, y: 0 }, false, true]]);
  });

  it("gives way to the box of the view that came last before its field closed", () => {
    const { hook } = held();

    hook.rerender({ board: asBox("Three") });
    hook.rerender({ board: asBox("Four") });
    act(() => {
      hook.result.current.noteFields.close("n");
    });

    expect(
      hook.result.current.nodes.map((node) => [node.id, node.data.shown]),
    ).toEqual([
      ["n", { kind: "element", type_ref: "t", name: "Four", summary: null }],
    ]);
  });
});
