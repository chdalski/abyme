import { act, cleanup, render, screen } from "@testing-library/react";
import type { ReactFlowProps, XYPosition } from "@xyflow/react";
import { type FunctionComponent, useContext } from "react";
import { afterEach, describe, expect, it, vi } from "vitest";

import type { BoardView } from "./bindings";
import { BoardPane } from "./BoardPane";
import { ReleasePin } from "./ElementBox";
import { FakeClient } from "./fakeClient";
import { boardView, placedBox } from "./testBoards";
import type { ElementNode } from "./nodes";

// jsdom cannot drive React Flow's pointer drag, so React Flow is replaced by
// a stand-in that keeps the props the pane gives it; a test then reports
// the position changes React Flow reports when boxes move. A probe inside it
// keeps what a box would call to release its pin.
const flow = vi.hoisted(() => ({
  props: null as ReactFlowProps<ElementNode> | null,
  release: null as ((element: string, position: XYPosition) => void) | null,
  Probe: (() => null) as FunctionComponent,
}));
vi.mock("@xyflow/react", async (original) => ({
  ...(await original<typeof import("@xyflow/react")>()),
  ReactFlow: (props: ReactFlowProps<ElementNode>) => {
    flow.props = props;
    return <flow.Probe />;
  },
}));

flow.Probe = function ReleaseProbe() {
  flow.release = useContext(ReleasePin);
  return null;
};

afterEach(cleanup);

function placement(element: string, x: number) {
  return placedBox(element, {
    x,
    shown: {
      kind: "element",
      type_ref: "c4/container",
      name: element,
      summary: null,
    },
  });
}

const VIEW: BoardView = boardView({
  placements: [placement("orders", 300), placement("payments", 0)],
});

async function openPane(client: FakeClient) {
  const rendered = render(
    <BoardPane client={client} board="context" onOpen={() => {}} />,
  );
  await screen.findByRole("heading", { name: "Context" });
  return rendered;
}

/** Drags the boxes in `targets` together and releases them there. */
async function dragTogether(targets: Record<string, XYPosition>) {
  const change = flow.props?.onNodesChange;
  if (change === undefined) {
    throw new Error("the pane gave React Flow no change handler");
  }
  const report = (dragging: boolean) =>
    change(
      Object.entries(targets).map(([id, position]) => ({
        type: "position",
        id,
        position,
        dragging,
      })),
    );
  act(() => report(true));
  await act(async () => {
    report(false);
    await Promise.resolve();
  });
}

function positionsOnCanvas() {
  return Object.fromEntries(
    (flow.props?.nodes ?? []).map((node) => [node.id, node.position]),
  );
}

function paneWith(applyFailure: string | null): FakeClient {
  const client = new FakeClient([]);
  client.views = { context: VIEW };
  client.applyFailure = applyFailure;
  return client;
}

describe("BoardPane", () => {
  it("sends one move at integer coordinates when a box is released", async () => {
    const client = paneWith(null);
    await openPane(client);

    await dragTogether({ payments: { x: 40.4, y: -80.6 } });

    expect(client.applied).toEqual([
      [
        {
          op: "move_element",
          element: { existing: "payments" },
          board: { existing: "context" },
          x: 40,
          y: -81,
          pinned: true,
        },
      ],
    ]);
    expect(positionsOnCanvas().payments).toEqual({ x: 40.4, y: -80.6 });
  });

  it("sends the moves of boxes released together in one request", async () => {
    const client = paneWith(null);
    await openPane(client);

    await dragTogether({
      payments: { x: 40, y: 80 },
      orders: { x: 340, y: 80 },
    });

    expect(client.applied).toHaveLength(1);
    expect(
      client.applied[0]?.map((move) => [
        move.element.existing,
        move.x,
        move.y,
        move.pinned,
      ]),
    ).toEqual([
      ["payments", 40, 80, true],
      ["orders", 340, 80, true],
    ]);
  });

  it("shows why a move failed, as text, and puts every box of it back", async () => {
    const message = "<img src=x onerror=alert(1)>";
    const client = paneWith(message);
    const { container } = await openPane(client);

    await dragTogether({
      payments: { x: 40, y: 80 },
      orders: { x: 340, y: 80 },
    });

    expect((await screen.findByRole("alert")).textContent).toBe(message);
    expect(container.querySelector("img")).toBeNull();
    expect(positionsOnCanvas()).toEqual({
      orders: { x: 300, y: 0 },
      payments: { x: 0, y: 0 },
    });
  });

  /** Releases the pin of `payments` as its box would, at its position. */
  async function releasePayments(position: XYPosition) {
    await act(async () => {
      flow.release?.("payments", position);
      await Promise.resolve();
    });
  }

  it("sends a release that says pinned: false, at integer coordinates", async () => {
    const client = paneWith(null);
    await openPane(client);

    await releasePayments({ x: 40.4, y: -80.6 });

    expect(client.applied).toEqual([
      [
        {
          op: "move_element",
          element: { existing: "payments" },
          board: { existing: "context" },
          x: 40,
          y: -81,
          pinned: false,
        },
      ],
    ]);
  });

  it("holds a release back until the move before it has been written", async () => {
    const client = paneWith(null);
    let finishMove = () => {};
    const apply = vi
      .spyOn(client, "apply")
      .mockImplementationOnce(
        () =>
          new Promise<void>((resolve) => {
            finishMove = resolve;
          }),
      )
      .mockResolvedValue();
    await openPane(client);

    await dragTogether({ payments: { x: 40, y: 80 } });
    await releasePayments({ x: 40, y: 80 });
    expect(apply).toHaveBeenCalledTimes(1);

    await act(async () => {
      finishMove();
      await Promise.resolve();
    });
    expect(apply.mock.calls.map(([[first]]) => first?.pinned)).toEqual([
      true,
      false,
    ]);
  });

  it("shows why a release failed, as text, and moves no box", async () => {
    const message = "<img src=x onerror=alert(1)>";
    const client = paneWith(message);
    const { container } = await openPane(client);

    await releasePayments({ x: 0, y: 0 });

    expect((await screen.findByRole("alert")).textContent).toBe(message);
    expect(container.querySelector("img")).toBeNull();
    expect(positionsOnCanvas()).toEqual({
      orders: { x: 300, y: 0 },
      payments: { x: 0, y: 0 },
    });
  });

  it("still sends a release after a move failed", async () => {
    const message = "the move failed";
    const client = paneWith(message);
    await openPane(client);

    await dragTogether({ payments: { x: 40, y: 80 } });
    await releasePayments({ x: 0, y: 0 });

    expect(client.applied.map(([first]) => first?.pinned)).toEqual([
      true,
      false,
    ]);
    expect((await screen.findByRole("alert")).textContent).toBe(message);
  });

  it("gives React Flow no key that deletes a box", async () => {
    await openPane(paneWith(null));

    expect(flow.props?.deleteKeyCode).toBeNull();
  });
});
