import {
  act,
  cleanup,
  fireEvent,
  render,
  screen,
  waitFor,
} from "@testing-library/react";
import type { ReactFlowProps, XYPosition } from "@xyflow/react";
import { type FunctionComponent, useContext } from "react";
import { afterEach, describe, expect, it, vi } from "vitest";

import type { BoardView } from "./bindings";
import { BoardPane } from "./BoardPane";
import { ReleasePin } from "./ElementBox";
import { FakeClient } from "./fakeClient";
import { boardView, placedBox } from "./testBoards";
import type { ElementNode } from "./nodes";
import { NewNotes, WriteNote } from "./NoteContent";
import type { NoteParts } from "./notes";

// jsdom cannot drive React Flow's pointer drag, so React Flow is replaced by
// a stand-in that keeps the props the pane gives it; a test then reports
// the position changes React Flow reports when boxes move. A probe inside it
// keeps what a box would call to release its pin.
const flow = vi.hoisted(() => ({
  props: null as ReactFlowProps<ElementNode> | null,
  release: null as ((element: string, position: XYPosition) => void) | null,
  writeNote: null as
    ((element: string, parts: NoteParts) => Promise<boolean>) | null,
  newNotes: null as {
    create: (
      draft: string,
      position: XYPosition,
      parts: NoteParts,
    ) => Promise<boolean>;
  } | null,
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
  flow.writeNote = useContext(WriteNote);
  flow.newNotes = useContext(NewNotes);
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
      client.applied[0]?.map((move) =>
        move.op === "move_element"
          ? [move.element, move.x, move.y, move.pinned]
          : null,
      ),
    ).toEqual([
      [{ existing: "payments" }, 40, 80, true],
      [{ existing: "orders" }, 340, 80, true],
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
          new Promise<string[]>((resolve) => {
            finishMove = () => resolve([]);
          }),
      )
      .mockResolvedValue([]);
    await openPane(client);

    await dragTogether({ payments: { x: 40, y: 80 } });
    await releasePayments({ x: 40, y: 80 });
    expect(apply).toHaveBeenCalledTimes(1);

    await act(async () => {
      finishMove();
      await Promise.resolve();
    });
    expect(
      apply.mock.calls.map(([[first]]) =>
        first?.op === "move_element" ? first.pinned : null,
      ),
    ).toEqual([true, false]);
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

    expect(
      client.applied.map(([first]) =>
        first?.op === "move_element" ? first.pinned : null,
      ),
    ).toEqual([true, false]);
    expect((await screen.findByRole("alert")).textContent).toBe(message);
  });

  describe("moving a note", () => {
    const withANote = (applyFailure: string | null) => {
      const client = paneWith(applyFailure);
      client.views = {
        context: boardView({
          placements: [
            placedBox("payments", {
              shown: { kind: "note", name: "Ask", body: "" },
            }),
          ],
        }),
      };
      return client;
    };

    it("moves and stays pinned", async () => {
      const client = withANote(null);
      await openPane(client);

      await dragTogether({ payments: { x: 40, y: 80 } });

      expect(client.applied).toEqual([
        [
          {
            op: "move_element",
            element: { existing: "payments" },
            board: { existing: "context" },
            x: 40,
            y: 80,
            pinned: true,
          },
        ],
      ]);
    });

    it("returns to its place with the message when the move is refused", async () => {
      const message =
        "operation 1: the canvas releases a pin on board context only at the position in the board file";
      await openPane(withANote(message));

      await dragTogether({ payments: { x: 40, y: 80 } });

      expect((await screen.findByRole("alert")).textContent).toBe(message);
      expect(positionsOnCanvas()).toEqual({ payments: { x: 0, y: 0 } });
    });
  });

  describe("writing a note", () => {
    const writer = () => {
      const write = flow.writeNote;
      if (write === null) {
        throw new Error("the pane gave boxes no way to write a note");
      }
      return write;
    };

    it("sends one update in order behind a move", async () => {
      const client = paneWith(null);
      let finishMove = () => {};
      const apply = vi
        .spyOn(client, "apply")
        .mockImplementationOnce(
          () =>
            new Promise<string[]>((resolve) => {
              finishMove = () => resolve([]);
            }),
        )
        .mockResolvedValue([]);
      await openPane(client);

      await dragTogether({ payments: { x: 40, y: 80 } });
      const written = writer()("payments", { name: "Ask", body: "Why" });
      await act(() => Promise.resolve());
      expect(apply).toHaveBeenCalledTimes(1);

      finishMove();
      await expect(written).resolves.toBe(true);
      expect(apply).toHaveBeenLastCalledWith([
        {
          op: "update_element",
          element: { existing: "payments" },
          name: "Ask",
          body: "Why",
        },
      ]);
    });

    it("says it failed and shows why, as text", async () => {
      const message = "<img src=x onerror=alert(1)>";
      const { container } = await openPane(paneWith(message));

      let written: boolean | null = null;
      await act(async () => {
        written = await writer()("payments", { name: "Ask", body: "" });
      });

      expect(written).toBe(false);
      expect(screen.getByRole("alert").textContent).toBe(message);
      expect(container.querySelector("img")).toBeNull();
    });
  });

  describe("creating a note", () => {
    const creator = () => {
      const create = flow.newNotes?.create;
      if (create === undefined) {
        throw new Error("the pane gave boxes no way to create a note");
      }
      return create;
    };

    it("hangs a note whose centre lies on a box on it, in the one request", async () => {
      const client = paneWith(null);
      const apply = vi.spyOn(client, "apply").mockResolvedValue(["ask"]);
      await openPane(client);

      // A note of 180 by 90 at 10, 20 has its centre at 100, 65, on `payments`.
      await expect(
        creator()("+note-1", { x: 10, y: 20 }, { name: "Ask", body: "" }),
      ).resolves.toBe(true);

      expect(apply).toHaveBeenLastCalledWith([
        {
          op: "create_element",
          key: "note",
          type_ref: "note",
          name: "Ask",
          body: "",
          statement: "idea",
        },
        {
          op: "place_element",
          element: { new: "note" },
          board: { existing: "context" },
        },
        {
          op: "move_element",
          element: { new: "note" },
          board: { existing: "context" },
          x: 10,
          y: 20,
          pinned: true,
        },
        {
          op: "attach_note",
          note: { new: "note" },
          to: { existing: "payments" },
          board: { existing: "context" },
          pinned: true,
        },
      ]);
    });

    it("sends the three operations for this board, behind a move", async () => {
      const client = paneWith(null);
      let finishMove = () => {};
      const apply = vi
        .spyOn(client, "apply")
        .mockImplementationOnce(
          () =>
            new Promise<string[]>((resolve) => {
              finishMove = () => resolve([]);
            }),
        )
        .mockResolvedValue(["ask"]);
      await openPane(client);

      await dragTogether({ payments: { x: 40, y: 80 } });
      const written = creator()(
        "+note-1",
        { x: 1.4, y: 2.6 },
        { name: "Ask", body: "Why" },
      );
      await act(() => Promise.resolve());
      expect(apply).toHaveBeenCalledTimes(1);

      finishMove();
      await expect(written).resolves.toBe(true);
      expect(apply).toHaveBeenLastCalledWith([
        {
          op: "create_element",
          key: "note",
          type_ref: "note",
          name: "Ask",
          body: "Why",
          statement: "idea",
        },
        {
          op: "place_element",
          element: { new: "note" },
          board: { existing: "context" },
        },
        {
          op: "move_element",
          element: { new: "note" },
          board: { existing: "context" },
          x: 1,
          y: 3,
          pinned: true,
        },
      ]);
    });

    /** The IDs of the boxes the pane hands to the canvas. */
    const ids = () => (flow.props?.nodes ?? []).map((node) => node.id);

    /** Starts a new note with `N` and writes it as `new-note`. */
    async function writeANewNote(client: FakeClient) {
      vi.spyOn(client, "apply").mockResolvedValue(["new-note"]);
      act(() => void fireEvent.keyDown(window, { key: "n" }));
      expect(ids()).toContain("+note-1");
      await act(async () => {
        await creator()("+note-1", { x: 1, y: 2 }, { name: "Ask", body: "" });
      });
    }

    it("lets a written note go without a message when a later view leaves it out", async () => {
      const client = paneWith(null);
      await openPane(client);
      await waitFor(() => expect(client.listening).toBe(1));
      await writeANewNote(client);
      expect(ids()).toContain("+note-1");
      // Each answer of the core is a new view, which still lacks the note.
      client.views = { context: { ...VIEW } };

      await act(async () => {
        client.change();
        await Promise.resolve();
      });

      await waitFor(() => expect(ids()).not.toContain("+note-1"));
      expect(screen.queryAllByRole("alert")).toEqual([]);
    });

    it("keeps a written note through a view asked for before the write answered", async () => {
      const client = paneWith(null);
      await openPane(client);
      await waitFor(() => expect(client.listening).toBe(1));
      client.hold("context");
      client.views = { context: { ...VIEW } };
      act(() => client.change());

      await writeANewNote(client);
      await act(async () => {
        client.release("context");
        await Promise.resolve();
      });

      await waitFor(() => expect(client.openCalls).toHaveLength(2));
      await act(() => new Promise<void>((done) => setTimeout(done, 0)));
      expect(ids()).toContain("+note-1");
    });

    it("says it failed and shows why, as text", async () => {
      const message = "<img src=x onerror=alert(1)>";
      const { container } = await openPane(paneWith(message));

      let written: boolean | null = null;
      await act(async () => {
        written = await creator()(
          "+note-1",
          { x: 1, y: 2 },
          { name: "Ask", body: "" },
        );
      });

      expect(written).toBe(false);
      expect(screen.getByRole("alert").textContent).toBe(message);
      expect(container.querySelector("img")).toBeNull();
    });
  });

  describe("deleting notes", () => {
    const withNotes = () =>
      boardView({
        placements: [
          placedBox("payments"),
          placedBox("ask", { shown: { kind: "note", name: "Ask", body: "" } }),
          placedBox("why", { shown: { kind: "note", name: "Why", body: "" } }),
        ],
      });
    const removeThrough = (ids: string[]) => {
      const change = flow.props?.onNodesChange;
      if (change === undefined) {
        throw new Error("the pane gave React Flow no change handler");
      }
      act(() => change(ids.map((id) => ({ type: "remove" as const, id }))));
    };

    it("sends one request for the notes, and none for a box beside them", async () => {
      const client = paneWith(null);
      client.views = { context: withNotes() };
      await openPane(client);

      removeThrough(["ask", "payments", "why"]);

      await waitFor(() => expect(client.applied).toHaveLength(1));
      expect(client.applied).toEqual([
        [
          { op: "delete_element", element: { existing: "ask" } },
          { op: "delete_element", element: { existing: "why" } },
        ],
      ]);
      expect(Object.keys(positionsOnCanvas())).toEqual([
        "payments",
        "ask",
        "why",
      ]);
    });

    it("sends nothing when no note is among them", async () => {
      const client = paneWith(null);
      client.views = { context: withNotes() };
      await openPane(client);

      removeThrough(["payments"]);
      await act(() => Promise.resolve());

      expect(client.applied).toEqual([]);
    });

    it("shows the core's refusal as text and keeps the note", async () => {
      const message = "<img src=x onerror=alert(1)> still relates to ask";
      const client = paneWith(message);
      client.views = { context: withNotes() };
      const { container } = await openPane(client);

      removeThrough(["ask"]);

      expect((await screen.findByRole("alert")).textContent).toBe(message);
      expect(container.querySelector("img")).toBeNull();
      expect(Object.keys(positionsOnCanvas())).toContain("ask");
    });
  });

  describe("a failed reload", () => {
    const OUT_OF_DATE = "This board may be out of date: ";
    const alerts = () =>
      screen.queryAllByRole("alert").map((alert) => alert.textContent);

    async function failReload(client: FakeClient, message: string) {
      client.views.context = message;
      client.change();
      await screen.findByText(`${OUT_OF_DATE}${message}`);
    }

    async function openListening(client: FakeClient) {
      const rendered = await openPane(client);
      await waitFor(() => expect(client.listening).toBe(1));
      return rendered;
    }

    it("keeps the board and shows the message above it as text, writing nothing", async () => {
      const message = "<img src=x onerror=alert(1)>";
      const client = paneWith(null);
      const { container } = await openListening(client);

      await failReload(client, message);

      expect(alerts()).toEqual([`${OUT_OF_DATE}${message}`]);
      expect(container.querySelector("img")).toBeNull();
      expect(screen.getByRole("heading", { name: "Context" })).toBeDefined();
      expect(positionsOnCanvas()).toEqual({
        orders: { x: 300, y: 0 },
        payments: { x: 0, y: 0 },
      });
      expect(client.applied).toEqual([]);
    });

    it("removes its message with the next good reload, writing nothing", async () => {
      const client = paneWith(null);
      await openListening(client);
      await failReload(client, "the file is broken");

      client.views.context = VIEW;
      client.change();

      await waitFor(() => expect(alerts()).toEqual([]));
      expect(client.applied).toEqual([]);
    });

    it("shows a failed move beside it, sends only that move, and puts the box back", async () => {
      const client = paneWith(null);
      await openListening(client);
      await failReload(client, "the file is broken");

      client.applyFailure = "the core cannot read the file";
      await dragTogether({ payments: { x: 40, y: 80 } });

      await waitFor(() =>
        expect(alerts()).toEqual([
          `${OUT_OF_DATE}the file is broken`,
          "the core cannot read the file",
        ]),
      );
      expect(client.applied).toEqual([
        [
          {
            op: "move_element",
            element: { existing: "payments" },
            board: { existing: "context" },
            x: 40,
            y: 80,
            pinned: true,
          },
        ],
      ]);
      expect(positionsOnCanvas().payments).toEqual({ x: 0, y: 0 });
    });

    it("shows a failed release beside it", async () => {
      const client = paneWith(null);
      await openListening(client);
      await failReload(client, "the file is broken");

      client.applyFailure = "the core cannot read the file";
      await releasePayments({ x: 0, y: 0 });

      await waitFor(() =>
        expect(alerts()).toEqual([
          `${OUT_OF_DATE}the file is broken`,
          "the core cannot read the file",
        ]),
      );
    });

    it("removes only its own message with a good reload, not a move's", async () => {
      const client = paneWith(null);
      await openListening(client);
      await failReload(client, "the file is broken");
      client.applyFailure = "the core cannot read the file";
      await dragTogether({ payments: { x: 40, y: 80 } });
      await screen.findByText("the core cannot read the file");

      client.views.context = VIEW;
      client.change();

      await waitFor(() =>
        expect(alerts()).toEqual(["the core cannot read the file"]),
      );
    });

    it("shows the message alone when the first load fails", async () => {
      const client = paneWith(null);
      client.views.context = "the file is broken";
      const { container } = render(
        <BoardPane client={client} board="context" onOpen={() => {}} />,
      );

      expect((await screen.findByRole("alert")).textContent).toBe(
        "the file is broken",
      );
      expect(container.querySelector(".flow")).toBeNull();
      expect(screen.queryByRole("heading")).toBeNull();
    });
  });

  it("gives React Flow the delete keys, and no box a way to be deleted", async () => {
    await openPane(paneWith(null));

    expect(flow.props?.deleteKeyCode).toEqual(["Delete", "Backspace"]);
    expect(flow.props?.nodes?.map((node) => node.deletable)).toEqual([
      false,
      false,
    ]);
  });
});

describe("attaching a note", () => {
  const NOTE = { kind: "note", name: "Ask", body: "" } as const;
  const WITH_A_NOTE: BoardView = boardView({
    placements: [
      placement("b", 100),
      placement("c", 400),
      placedBox("n", { x: 120, y: 10, shown: NOTE, attached_to: "b" }),
    ],
  });

  function paneWithANote(): FakeClient {
    const client = new FakeClient([]);
    client.views = { context: WITH_A_NOTE };
    return client;
  }

  it("sends the move at whole numbers on the board, then the attachment, in one request", async () => {
    const client = paneWithANote();
    await openPane(client);

    // Relative to `b` at 100, 0: the note lands at 400.6, 10.4, on `c`.
    await dragTogether({ n: { x: 300.6, y: 10.4 } });

    expect(client.applied).toEqual([
      [
        {
          op: "move_element",
          element: { existing: "n" },
          board: { existing: "context" },
          x: 401,
          y: 10,
          pinned: true,
        },
        {
          op: "attach_note",
          note: { existing: "n" },
          to: { existing: "c" },
          board: { existing: "context" },
          pinned: true,
        },
      ],
    ]);
  });

  it("sends the move, then the detachment, when the note leaves its box for none", async () => {
    const client = paneWithANote();
    await openPane(client);

    await dragTogether({ n: { x: 0, y: 500 } });

    expect(client.applied).toEqual([
      [
        {
          op: "move_element",
          element: { existing: "n" },
          board: { existing: "context" },
          x: 100,
          y: 500,
          pinned: true,
        },
        {
          op: "detach_note",
          note: { existing: "n" },
          board: { existing: "context" },
        },
      ],
    ]);
  });

  it("sends only the box's move when the box is dragged with its note", async () => {
    const client = paneWithANote();
    await openPane(client);

    await dragTogether({ b: { x: 600, y: 300 } });

    expect(client.applied).toEqual([
      [
        {
          op: "move_element",
          element: { existing: "b" },
          board: { existing: "context" },
          x: 600,
          y: 300,
          pinned: true,
        },
      ],
    ]);
    const note = flow.props?.nodes?.find((node) => node.id === "n");
    expect(note?.parentId).toBe("b");
    expect(note?.position).toEqual({ x: 20, y: 10 });
  });
});
