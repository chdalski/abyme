import {
  act,
  cleanup,
  fireEvent,
  render,
  screen,
  waitFor,
  within,
} from "@testing-library/react";
import { StrictMode } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { App } from "./App";
import type { BoardView, Placement } from "./bindings";
import { FakeClient } from "./fakeClient";
import { boardView, placedBox } from "./testBoards";

afterEach(cleanup);

const NO_PROJECT =
  "no Abyme project in this directory or above it: run `abyme-mcp init` in the directory that is to become the project's root";

function view(id: string, title: string, names: string[] = []): BoardView {
  return boardView({
    id,
    title,
    placements: names.map((name, index) =>
      placedBox(name.toLowerCase(), {
        x: index * 200,
        shown: {
          kind: "element",
          type_ref: "c4/container",
          name,
          summary: null,
        },
      }),
    ),
  });
}

function clientWithBoards(): FakeClient {
  const client = new FakeClient([
    { id: "containers", title: "Containers", depth: 0 },
    { id: "context", title: "Context", depth: 0 },
  ]);
  client.views = {
    containers: view("containers", "Containers", ["Orders"]),
    context: view("context", "Context", ["Payments"]),
  };
  return client;
}

/** The marks on the open board, by the box's element. */
function marks(container: HTMLElement) {
  return Object.fromEntries(
    [...container.querySelectorAll(".react-flow__node")].map((box) => [
      box.getAttribute("data-id"),
      box.querySelector(".element-flag")?.textContent ?? null,
    ]),
  );
}

/** Opens the board `context`, which holds the box of `payments`. */
async function openContext(client: FakeClient) {
  const rendered = render(<App client={client} />);
  fireEvent.click(await screen.findByRole("button", { name: "Context" }));
  await screen.findByRole("heading", { name: "Context" });
  await waitFor(() => expect(client.listening).toBe(3));
  return rendered;
}

describe("review flags", () => {
  it("marks a box with its level, in words, and no other box", async () => {
    const client = clientWithBoards();
    client.views.context = view("context", "Context", [
      "Payments",
      "Orders",
      "Calm",
      "__proto__",
    ]);
    client.flags = [
      { element: "payments", priority: "task" },
      { element: "orders", priority: "hint" },
    ];

    const { container } = await openContext(client);

    await waitFor(() =>
      expect(marks(container)).toEqual({
        payments: "⚠ task",
        orders: "⚠ hint",
        calm: null,
        ["__proto__"]: null,
      }),
    );
    expect(screen.queryByRole("status")).toBeNull();
  });

  it("marks a note like a box", async () => {
    const client = clientWithBoards();
    client.views.context = boardView({
      id: "context",
      title: "Context",
      placements: [
        placedBox("ask", {
          shown: { kind: "note", name: "Ask why", body: "" },
        }),
      ],
    });
    client.flags = [{ element: "ask", priority: "hint" }];

    const { container } = await openContext(client);

    await waitFor(() => expect(marks(container)).toEqual({ ask: "⚠ hint" }));
    expect(screen.getByText("Ask why")).toBeDefined();
  });

  it("keeps the box's text and offers no button on a mark", async () => {
    const client = clientWithBoards();
    client.flags = [{ element: "payments", priority: "task" }];

    const { container } = await openContext(client);

    const mark = await waitFor(() => {
      const found = container.querySelector(".element-flag");
      expect(found).not.toBeNull();
      return found;
    });
    expect(mark?.closest("button")).toBeNull();
    expect(mark?.querySelector("button")).toBeNull();
    expect(screen.getByText("Payments")).toBeDefined();
    expect(screen.getByText("c4/container")).toBeDefined();
  });

  it("shows the board while the first answer is pending, says so, and marks nothing", async () => {
    const client = clientWithBoards();
    client.flags = [{ element: "payments", priority: "task" }];
    client.hold(FakeClient.FLAGS);

    const { container } = await openContext(client);

    expect(screen.getByRole("status").textContent).toBe(
      "Checking review flags…",
    );
    expect(screen.getByText("Payments")).toBeDefined();
    expect(marks(container)).toEqual({ payments: null });

    client.release(FakeClient.FLAGS);
    await waitFor(() =>
      expect(marks(container)).toEqual({ payments: "⚠ task" }),
    );
    expect(screen.queryByRole("status")).toBeNull();
  });

  it("holds up neither an arrow nor a move while the first answer is pending, and disturbs neither when it comes", async () => {
    const client = clientWithBoards();
    client.views.context = {
      ...view("context", "Context", ["Payments", "Orders"]),
      relations: [
        { from: "payments", kind: "uses", to: "orders", label: "calls" },
      ],
    };
    client.flags = [{ element: "orders", priority: "hint" }];
    client.hold(FakeClient.FLAGS);
    const { container } = await openContext(client);

    await waitFor(() =>
      expect(
        container.querySelector(".react-flow__edge-text")?.textContent,
      ).toBe("calls"),
    );
    const box = container.querySelector(
      '.react-flow__node[data-id="payments"]',
    );
    if (!(box instanceof HTMLElement)) {
      throw new Error("no box for payments");
    }
    fireEvent.keyDown(box, { key: "Enter" });
    await waitFor(() => expect(box.classList.contains("selected")).toBe(true));
    fireEvent.keyDown(box, { key: "ArrowRight" });
    await waitFor(() => expect(client.applied).toHaveLength(1));
    const movedTo = box.style.transform;
    const opened = client.openCalls.length;

    client.release(FakeClient.FLAGS);

    await waitFor(() =>
      expect(marks(container)).toEqual({ payments: null, orders: "⚠ hint" }),
    );
    expect(box.isConnected).toBe(true);
    expect(box.style.transform).toBe(movedTo);
    expect(client.openCalls).toHaveLength(opened);
  });

  it("says why flags are unavailable, as text, and marks nothing", async () => {
    const client = clientWithBoards();
    client.flags = "<img src=x onerror=alert(1)>";

    const { container } = await openContext(client);

    await waitFor(() =>
      expect(screen.getByRole("status").textContent).toBe(
        "Review flags are unavailable, so no box is marked: <img src=x onerror=alert(1)>",
      ),
    );
    expect(container.querySelector("img")).toBeNull();
    expect(marks(container)).toEqual({ payments: null });
  });

  it("takes every mark away when a later answer fails, and brings them back", async () => {
    const client = clientWithBoards();
    client.flags = [{ element: "payments", priority: "task" }];
    const { container } = await openContext(client);
    await waitFor(() =>
      expect(marks(container)).toEqual({ payments: "⚠ task" }),
    );

    client.flags = "git failed";
    client.change();
    await waitFor(() =>
      expect(screen.getByRole("status").textContent).toContain("git failed"),
    );
    expect(marks(container)).toEqual({ payments: null });

    client.flags = [{ element: "payments", priority: "hint" }];
    client.change();
    await waitFor(() =>
      expect(marks(container)).toEqual({ payments: "⚠ hint" }),
    );
    expect(screen.queryByRole("status")).toBeNull();
  });

  it("asks once for all boards, and again when the project's files change", async () => {
    const client = clientWithBoards();
    await openContext(client);
    await waitFor(() => expect(client.flagCalls).toBe(1));

    fireEvent.click(screen.getByRole("button", { name: "Containers" }));
    await screen.findByRole("heading", { name: "Containers" });
    expect(client.flagCalls).toBe(1);

    client.change();
    await waitFor(() => expect(client.flagCalls).toBe(2));
  });
});

describe("leaving a board while a note's text is being written", () => {
  /** A client whose board `context` holds the note `ask`, opened for typing. */
  async function typing(client: FakeClient) {
    client.views.context = boardView({
      id: "context",
      title: "Context",
      placements: [
        placedBox("ask", {
          shown: { kind: "note", name: "Ask", body: "" },
        }),
      ],
    });
    await openContext(client);
    fireEvent.doubleClick(screen.getByText("Ask"));
    const field = screen.getByRole("textbox", { name: "Text of the note" });
    fireEvent.change(field, { target: { value: "Asked" } });
    return field;
  }
  const leave = () =>
    fireEvent.click(screen.getByRole("button", { name: "Containers" }));
  const settle = () =>
    act(() => new Promise<void>((done) => queueMicrotask(done)));

  it("goes to the other board once the write worked", async () => {
    const client = clientWithBoards();
    let finish = () => {};
    vi.spyOn(client, "apply").mockImplementationOnce(
      () =>
        new Promise<string[]>((resolve) => {
          finish = () => resolve([]);
        }),
    );
    const field = await typing(client);

    fireEvent.blur(field);
    leave();
    await settle();
    expect(screen.getByRole("heading", { name: "Context" })).toBeDefined();

    finish();
    expect(
      await screen.findByRole("heading", { name: "Containers" }),
    ).toBeDefined();
  });

  it("stays with the text and the message when a write on its way fails", async () => {
    const client = clientWithBoards();
    let fail: (message: string) => void = () => {};
    vi.spyOn(client, "apply").mockImplementationOnce(
      () =>
        new Promise<string[]>((_resolve, reject) => {
          fail = reject;
        }),
    );
    const field = await typing(client);

    fireEvent.blur(field);
    leave();
    await settle();
    fail("the file is broken");

    expect((await screen.findByRole("alert")).textContent).toBe(
      "the file is broken",
    );
    expect(
      screen.getByRole<HTMLTextAreaElement>("textbox", {
        name: "Text of the note",
      }).value,
    ).toBe("Asked");
    expect(screen.getByRole("heading", { name: "Context" })).toBeDefined();

    leave();
    expect(
      await screen.findByRole("heading", { name: "Containers" }),
    ).toBeDefined();
  });

  it("stays too when the write failed before the click arrived", async () => {
    const client = clientWithBoards();
    client.applyFailure = "the file is broken";
    const field = await typing(client);

    fireEvent.blur(field);
    await screen.findByRole("alert");
    leave();
    await settle();

    expect(screen.getByRole("heading", { name: "Context" })).toBeDefined();
    expect(screen.getByRole("textbox")).toBeDefined();

    leave();
    expect(
      await screen.findByRole("heading", { name: "Containers" }),
    ).toBeDefined();
  });

  const containers = () => screen.getByRole("button", { name: "Containers" });
  /** A press of the mouse on `target`: its button goes down, then the click. */
  const press = (target: HTMLElement) => {
    fireEvent.mouseDown(target, { detail: 1 });
    fireEvent.click(target, { detail: 1 });
  };

  it("holds back the click of the press that ended the field, and lets a later press go", async () => {
    const client = clientWithBoards();
    let fail: (message: string) => void = () => {};
    vi.spyOn(client, "apply").mockImplementationOnce(
      () =>
        new Promise<string[]>((_resolve, reject) => {
          fail = reject;
        }),
    );
    const field = await typing(client);

    fireEvent.mouseDown(containers(), { detail: 1 });
    fireEvent.blur(field);
    await settle();
    fail("the file is broken");
    await screen.findByRole("alert");
    fireEvent.click(containers(), { detail: 1 });
    await settle();
    expect(screen.getByRole("heading", { name: "Context" })).toBeDefined();
    expect(screen.getByRole("alert").textContent).toBe("the file is broken");
    expect(
      screen.getByRole<HTMLTextAreaElement>("textbox", {
        name: "Text of the note",
      }).value,
    ).toBe("Asked");

    press(containers());
    expect(
      await screen.findByRole("heading", { name: "Containers" }),
    ).toBeDefined();
  });

  it("holds back the further clicks of a double click that ended the field", async () => {
    const client = clientWithBoards();
    let fail: (message: string) => void = () => {};
    vi.spyOn(client, "apply").mockImplementationOnce(
      () =>
        new Promise<string[]>((_resolve, reject) => {
          fail = reject;
        }),
    );
    const field = await typing(client);

    fireEvent.mouseDown(containers(), { detail: 1 });
    fireEvent.blur(field);
    await settle();
    fail("the file is broken");
    await screen.findByRole("alert");
    fireEvent.click(containers(), { detail: 1 });
    fireEvent.mouseDown(containers(), { detail: 2 });
    fireEvent.click(containers(), { detail: 2 });
    fireEvent.doubleClick(containers(), { detail: 2 });
    await settle();

    expect(screen.getByRole("heading", { name: "Context" })).toBeDefined();
    press(containers());
    expect(
      await screen.findByRole("heading", { name: "Containers" }),
    ).toBeDefined();
  });

  it("holds back the further clicks of a double click whose first click waited for the failing write", async () => {
    const client = clientWithBoards();
    let fail: (message: string) => void = () => {};
    vi.spyOn(client, "apply").mockImplementationOnce(
      () =>
        new Promise<string[]>((_resolve, reject) => {
          fail = reject;
        }),
    );
    const field = await typing(client);

    fireEvent.mouseDown(containers(), { detail: 1 });
    fireEvent.blur(field);
    fireEvent.click(containers(), { detail: 1 });
    await settle();
    fail("the file is broken");
    await screen.findByRole("alert");
    fireEvent.mouseDown(containers(), { detail: 2 });
    fireEvent.click(containers(), { detail: 2 });
    await settle();

    expect(screen.getByRole("heading", { name: "Context" })).toBeDefined();
    expect(screen.getByRole("alert").textContent).toBe("the file is broken");
  });

  it.each([
    ["unchanged", "Ask"],
    ["blank", ""],
  ])(
    "lets the next press go when the field then ends with %s text and writes nothing",
    async (_kind, text) => {
      const client = clientWithBoards();
      client.applyFailure = "the file is broken";
      const field = await typing(client);
      fireEvent.blur(field);
      await screen.findByRole("alert");
      client.applyFailure = null;

      fireEvent.change(field, { target: { value: text } });
      fireEvent.keyDown(field, { key: "Escape" });
      await waitFor(() => expect(screen.queryByRole("textbox")).toBeNull());
      press(containers());

      expect(
        await screen.findByRole("heading", { name: "Containers" }),
      ).toBeDefined();
    },
  );

  it("lets a press of the keyboard go after a failed write", async () => {
    const client = clientWithBoards();
    client.applyFailure = "the file is broken";
    const field = await typing(client);
    fireEvent.blur(field);
    await screen.findByRole("alert");

    fireEvent.keyDown(containers(), { key: "Enter" });
    fireEvent.click(containers(), { detail: 0 });

    expect(
      await screen.findByRole("heading", { name: "Containers" }),
    ).toBeDefined();
  });

  describe("with a new note", () => {
    /** The board `context` is open, with a new note being typed. */
    async function typingANewNote(client: FakeClient) {
      await openContext(client);
      fireEvent.keyDown(window, { key: "n" });
      const field = screen.getByRole("textbox", { name: "Text of the note" });
      fireEvent.change(field, { target: { value: "Ask" } });
      return field;
    }

    it("goes to the other board once the note is written", async () => {
      const client = clientWithBoards();
      let finish = () => {};
      vi.spyOn(client, "apply").mockImplementationOnce(
        () =>
          new Promise<string[]>((resolve) => {
            finish = () => resolve(["ask"]);
          }),
      );
      const field = await typingANewNote(client);

      fireEvent.blur(field);
      leave();
      await settle();
      expect(screen.getByRole("heading", { name: "Context" })).toBeDefined();

      finish();
      expect(
        await screen.findByRole("heading", { name: "Containers" }),
      ).toBeDefined();
    });

    it("stays with the text and the message when the note cannot be written", async () => {
      const client = clientWithBoards();
      client.applyFailure = "the file is broken";
      const field = await typingANewNote(client);

      fireEvent.blur(field);
      leave();
      await screen.findByRole("alert");
      await settle();

      expect(screen.getByRole("heading", { name: "Context" })).toBeDefined();
      expect(
        screen.getByRole<HTMLTextAreaElement>("textbox", {
          name: "Text of the note",
        }).value,
      ).toBe("Ask");
    });
  });

  it("goes at once without a write", async () => {
    const client = clientWithBoards();
    await openContext(client);

    leave();

    expect(
      await screen.findByRole("heading", { name: "Containers" }),
    ).toBeDefined();
  });
});

describe("deleting a note another element relates to", () => {
  it("keeps the note and the arrow, and says why", async () => {
    const client = clientWithBoards();
    client.views.context = boardView({
      id: "context",
      title: "Context",
      placements: [
        placedBox("payments", {
          shown: {
            kind: "element",
            type_ref: "c4/container",
            name: "Payments",
            summary: null,
          },
        }),
        placedBox("ask", {
          y: 300,
          shown: { kind: "note", name: "Ask", body: "" },
        }),
      ],
      relations: [{ from: "payments", kind: "uses", to: "ask", label: "asks" }],
    });
    client.applyFailure = "element ask is still related: payments uses it";
    const { container } = await openContext(client);
    const note = container.querySelector('.react-flow__node[data-id="ask"]');
    if (!(note instanceof HTMLElement)) {
      throw new Error("no note");
    }
    fireEvent.keyDown(note, { key: "Enter" });
    fireEvent.keyUp(note, { key: "Enter" });
    await waitFor(() => expect(note.classList.contains("selected")).toBe(true));

    fireEvent.keyDown(note, { key: "Delete" });

    expect((await screen.findByRole("alert")).textContent).toBe(
      "element ask is still related: payments uses it",
    );
    expect(client.applied).toEqual([
      [{ op: "delete_element", element: { existing: "ask" } }],
    ]);
    expect(note.isConnected).toBe(true);
    expect(container.querySelector(".react-flow__edge-text")?.textContent).toBe(
      "asks",
    );
  });
});

describe("a note deleted while its text is typed", () => {
  const DELETED = "This note was deleted, so its text was not written.";
  const ASK = placedBox("ask", {
    x: 200,
    y: 50,
    shown: { kind: "note", name: "Ask", body: "" },
  });
  const element = (name: string) =>
    ({
      kind: "element",
      type_ref: "c4/container",
      name,
      summary: null,
    }) as const;
  /** The board `context`, whose box `payments` shows `marker`, with `placements`. */
  const contextWith = (marker: string, ...placements: Placement[]) =>
    boardView({
      id: "context",
      title: "Context",
      placements: [
        placedBox("payments", { shown: element(marker) }),
        ...placements,
      ],
    });
  const field = () =>
    screen.getByRole<HTMLTextAreaElement>("textbox", {
      name: "Text of the note",
    });
  /** Brings `view`, whose box `payments` shows `marker`, as a change of the files does. */
  async function reload(
    client: FakeClient,
    marker: string,
    ...placements: Placement[]
  ) {
    client.views.context = contextWith(marker, ...placements);
    act(() => {
      client.change();
    });
    await screen.findByText(marker);
  }
  /** Opens `ask` for typing, types `Asked`, and has the AI delete it. */
  async function deletedWhileTyped(client: FakeClient) {
    client.views.context = contextWith("Payments", ASK);
    const rendered = await openContext(client);
    fireEvent.doubleClick(screen.getByText("Ask"));
    fireEvent.change(field(), { target: { value: "Asked" } });
    await reload(client, "Deleted");
    return rendered;
  }
  const settle = () =>
    act(() => new Promise<void>((done) => queueMicrotask(done)));

  it("keeps the note where it lay, with its field and text, and lets it be neither selected, moved, nor deleted", async () => {
    const client = clientWithBoards();
    const { container } = await deletedWhileTyped(client);
    const note = container.querySelector('.react-flow__node[data-id="ask"]');
    if (!(note instanceof HTMLElement)) {
      throw new Error("no note");
    }

    expect(field().value).toBe("Asked");
    expect(note.style.transform).toBe("translate(200px,50px)");
    expect(note.classList.contains("selectable")).toBe(false);
    expect(note.classList.contains("draggable")).toBe(false);
    fireEvent.click(note);
    fireEvent.keyDown(note, { key: "Enter" });
    fireEvent.keyUp(note, { key: "Enter" });
    fireEvent.keyDown(note, { key: "ArrowRight" });
    fireEvent.keyDown(note, { key: "Delete" });
    fireEvent.keyDown(note, { key: "Backspace" });
    await settle();

    expect(note.classList.contains("selected")).toBe(false);
    expect(note.style.transform).toBe("translate(200px,50px)");
    expect(field().value).toBe("Asked");
    expect(client.applied).toEqual([]);
  });

  it.each([
    ["Esc", [() => fireEvent.keyDown(field(), { key: "Escape" })]],
    ["a click next to it", [() => fireEvent.blur(field())]],
    [
      "Esc and a blur",
      [
        () => fireEvent.keyDown(field(), { key: "Escape" }),
        () => fireEvent.blur(field()),
      ],
    ],
    [
      "Esc twice",
      [
        () => fireEvent.keyDown(field(), { key: "Escape" }),
        () => fireEvent.keyDown(field(), { key: "Escape" }),
      ],
    ],
  ])(
    "writes nothing on %s with changed text, says so, and keeps the field",
    async (_how, ends) => {
      const client = clientWithBoards();
      await deletedWhileTyped(client);

      for (const end of ends) {
        end();
        await settle();
      }

      expect((await screen.findByRole("alert")).textContent).toBe(DELETED);
      expect(field().value).toBe("Asked");
      expect(field().readOnly).toBe(false);
      expect(client.applied).toEqual([]);
    },
  );

  it.each([
    ["a click next to it", [() => fireEvent.blur(field())]],
    ["Esc", [() => fireEvent.keyDown(field(), { key: "Escape" })]],
  ])(
    "stays on the board after %s, with the text and the message, when the click of that press goes to another board",
    async (_how, ends) => {
      const client = clientWithBoards();
      await deletedWhileTyped(client);

      for (const end of ends) {
        end();
      }
      fireEvent.click(screen.getByRole("button", { name: "Containers" }));
      await settle();

      expect(screen.getByRole("heading", { name: "Context" })).toBeDefined();
      expect(field().value).toBe("Asked");
      expect((await screen.findByRole("alert")).textContent).toBe(DELETED);
      expect(client.applied).toEqual([]);

      fireEvent.mouseDown(window, { detail: 1 });
      fireEvent.click(screen.getByRole("button", { name: "Containers" }));
      expect(
        await screen.findByRole("heading", { name: "Containers" }),
      ).toBeDefined();
    },
  );

  it.each([
    ["unchanged", "Ask"],
    ["blank", "  "],
  ])(
    "writes nothing for %s text, and the note goes at once",
    async (_kind, text) => {
      const client = clientWithBoards();
      await deletedWhileTyped(client);

      fireEvent.change(field(), { target: { value: text } });
      fireEvent.keyDown(field(), { key: "Escape" });
      await settle();

      expect(
        screen.queryByRole("textbox", { name: "Text of the note" }),
      ).toBeNull();
      expect(screen.queryByText("Ask")).toBeNull();
      expect(screen.queryByRole("alert")).toBeNull();
      expect(client.applied).toEqual([]);
    },
  );

  it("shows nothing a later view lists under its ID until the field closes, then the box of the latest view with what hangs on it", async () => {
    const client = clientWithBoards();
    const { container } = await deletedWhileTyped(client);
    const askAsABox = (name: string) => [
      placedBox("ask", { x: 400, y: 0, shown: element(name) }),
      placedBox("hung", {
        x: 420,
        y: 10,
        shown: { kind: "note", name: "Hung", body: "" },
        attached_to: "ask",
      }),
    ];

    await reload(client, "Third", ...askAsABox("Ask three"));
    expect(screen.queryByText("Ask three")).toBeNull();
    expect(screen.queryByText("Hung")).toBeNull();
    await reload(client, "Fourth", ...askAsABox("Ask four"));
    fireEvent.keyDown(field(), { key: "Escape" });
    await settle();
    expect(field().value).toBe("Asked");
    expect(screen.queryByText("Ask four")).toBeNull();

    fireEvent.change(field(), { target: { value: "" } });
    fireEvent.keyDown(field(), { key: "Escape" });
    expect(await screen.findByText("Ask four")).toBeDefined();
    expect(client.applied).toEqual([]);

    // The note hangs on the box of the latest view and moves against it.
    const hung = container.querySelector('.react-flow__node[data-id="hung"]');
    if (!(hung instanceof HTMLElement)) {
      throw new Error("no hung note");
    }
    fireEvent.keyDown(hung, { key: "Enter" });
    fireEvent.keyUp(hung, { key: "Enter" });
    await waitFor(() => expect(hung.classList.contains("selected")).toBe(true));
    fireEvent.keyDown(hung, { key: "ArrowRight" });
    await waitFor(() => expect(client.applied).toHaveLength(1));
    expect(client.applied[0]).toEqual([
      {
        op: "move_element",
        element: { existing: "hung" },
        board: { existing: "context" },
        x: 425,
        y: 10,
        pinned: true,
      },
    ]);
  });

  it.each([
    [
      "another note",
      { kind: "note", name: "Other", body: "" } as const,
      "Other",
    ],
    [
      "a box whose file is missing",
      { kind: "missing" } as const,
      "Missing element ask",
    ],
  ])(
    "refuses the write when a later view lists its ID as %s, and gives way to it once closed",
    async (_as, shown, text) => {
      const client = clientWithBoards();
      await deletedWhileTyped(client);

      await reload(client, "Third", placedBox("ask", { x: 400, shown }));
      expect(screen.queryByText(text)).toBeNull();
      fireEvent.keyDown(field(), { key: "Escape" });
      expect((await screen.findByRole("alert")).textContent).toBe(DELETED);
      fireEvent.change(field(), { target: { value: "" } });
      fireEvent.keyDown(field(), { key: "Escape" });

      expect(await screen.findByText(text)).toBeDefined();
      expect(client.applied).toEqual([]);
    },
  );

  it("refuses the next end itself when a write on its way fails after the note was deleted", async () => {
    const client = clientWithBoards();
    client.views.context = contextWith("Payments", ASK);
    await openContext(client);
    let fail: (message: string) => void = () => {};
    const apply = vi.spyOn(client, "apply").mockImplementationOnce(
      () =>
        new Promise<string[]>((_resolve, reject) => {
          fail = reject;
        }),
    );
    fireEvent.doubleClick(screen.getByText("Ask"));
    fireEvent.change(field(), { target: { value: "Asked" } });
    fireEvent.keyDown(field(), { key: "Escape" });

    await reload(client, "Deleted");
    await act(async () => {
      fail("no element has the ID ask");
      await Promise.resolve();
    });
    expect(field().value).toBe("Asked");
    fireEvent.keyDown(field(), { key: "Escape" });

    expect((await screen.findByRole("alert")).textContent).toBe(DELETED);
    expect(apply).toHaveBeenCalledTimes(1);
  });

  it("lets the note go when a write on its way works after the note was deleted", async () => {
    const client = clientWithBoards();
    client.views.context = contextWith("Payments", ASK);
    await openContext(client);
    let finish = () => {};
    vi.spyOn(client, "apply").mockImplementationOnce(
      () =>
        new Promise<string[]>((resolve) => {
          finish = () => resolve([]);
        }),
    );
    fireEvent.doubleClick(screen.getByText("Ask"));
    fireEvent.change(field(), { target: { value: "Asked" } });
    fireEvent.keyDown(field(), { key: "Escape" });

    await reload(client, "Deleted");
    await act(async () => {
      finish();
      await Promise.resolve();
    });

    await waitFor(() =>
      expect(
        screen.queryByRole("textbox", { name: "Text of the note" }),
      ).toBeNull(),
    );
    expect(screen.queryByText("Asked")).toBeNull();
  });

  it("still writes a new note whose field is open through a later view", async () => {
    const client = clientWithBoards();
    client.views.context = contextWith("Payments");
    await openContext(client);
    fireEvent.keyDown(window, { key: "n" });
    fireEvent.change(field(), { target: { value: "Idea" } });

    await reload(client, "Later");
    fireEvent.keyDown(field(), { key: "Escape" });

    await waitFor(() => expect(client.applied).toHaveLength(1));
    expect(client.applied[0]?.[0]?.op).toBe("create_element");
    expect(screen.queryByText(DELETED)).toBeNull();
  });
});

describe("a failed reload of the open board", () => {
  it("keeps the box and its selection through the failure and the recovery", async () => {
    const client = clientWithBoards();
    const { container } = await openContext(client);
    const box = container.querySelector(
      '.react-flow__node[data-id="payments"]',
    );
    if (!(box instanceof HTMLElement)) {
      throw new Error("no box for payments");
    }
    const good = client.views.context;
    fireEvent.keyDown(box, { key: "Enter" });
    await waitFor(() => expect(box.classList.contains("selected")).toBe(true));

    client.views.context = "the file is broken";
    client.change();
    await screen.findByText(/the file is broken/);
    expect(box.isConnected).toBe(true);
    expect(box.classList.contains("selected")).toBe(true);

    client.views.context = good ?? "no view";
    client.change();
    await waitFor(() => expect(screen.queryByRole("alert")).toBeNull());
    expect(box.isConnected).toBe(true);
    expect(box.classList.contains("selected")).toBe(true);
    expect(client.applied).toEqual([]);
  });

  it("does not carry the kept board over to another one", async () => {
    const client = clientWithBoards();
    await openContext(client);
    client.views.context = "the file is broken";
    client.change();
    await screen.findByText(/the file is broken/);

    client.views.containers = "no board has the ID containers";
    fireEvent.click(screen.getByRole("button", { name: "Containers" }));

    await screen.findByText("no board has the ID containers");
    expect(screen.queryByText("Payments")).toBeNull();
    expect(screen.queryByRole("heading", { name: "Context" })).toBeNull();
    expect(screen.queryByText(/out of date/)).toBeNull();
  });
});

describe("review flags stay current", () => {
  beforeEach(() => {
    vi.useFakeTimers({
      toFake: ["setInterval", "clearInterval"],
      shouldAdvanceTime: true,
    });
  });

  afterEach(() => {
    vi.useRealTimers();
    vi.restoreAllMocks();
  });

  const pass = (milliseconds: number) =>
    act(() => {
      vi.advanceTimersByTime(milliseconds);
    });
  const focus = () => act(() => void fireEvent.focus(window));
  const hideTheDocument = () =>
    vi
      .spyOn(Document.prototype, "visibilityState", "get")
      .mockReturnValue("hidden");

  /** The board `context` is open, and the first answer on flags has arrived. */
  async function openWithAnswer(client: FakeClient) {
    client.flags = [{ element: "payments", priority: "task" }];
    const rendered = await openContext(client);
    await waitFor(() =>
      expect(marks(rendered.container)).toEqual({ payments: "⚠ task" }),
    );
    return rendered;
  }

  /** Gives a run that has ended the time to clear the way for the next. */
  const settle = () =>
    act(() => new Promise<void>((done) => queueMicrotask(done)));

  it("asks again when the window gets focus", async () => {
    const client = clientWithBoards();
    await openWithAnswer(client);
    await settle();

    focus();

    await waitFor(() => expect(client.flagCalls).toBe(2));
  });

  it("asks again every 30 s while the document is visible", async () => {
    const client = clientWithBoards();
    await openWithAnswer(client);
    await settle();
    const before = client.flagCalls;

    pass(30_000);
    await waitFor(() => expect(client.flagCalls).toBe(before + 1));
    await settle();

    pass(30_000);
    await waitFor(() => expect(client.flagCalls).toBe(before + 2));
  });

  it("does not ask while the document is hidden, and asks again once it is visible", async () => {
    const client = clientWithBoards();
    await openWithAnswer(client);
    await settle();
    const before = client.flagCalls;
    const hidden = hideTheDocument();

    pass(90_000);
    focus();
    await settle();
    expect(client.flagCalls).toBe(before);

    hidden.mockRestore();
    pass(30_000);
    await waitFor(() => expect(client.flagCalls).toBe(before + 1));
  });

  const turnVisible = () =>
    act(() => void document.dispatchEvent(new Event("visibilitychange")));

  it("asks again when the document turns visible, also without focus", async () => {
    const client = clientWithBoards();
    await openWithAnswer(client);
    await settle();
    const before = client.flagCalls;
    const hidden = hideTheDocument();
    turnVisible();
    await settle();
    expect(client.flagCalls).toBe(before);

    hidden.mockRestore();
    turnVisible();

    await waitFor(() => expect(client.flagCalls).toBe(before + 1));
  });

  it("starts no second run when the document turns visible and gets focus at once", async () => {
    const client = clientWithBoards();
    await openWithAnswer(client);
    await settle();
    const before = client.flagCalls;
    client.hold(FakeClient.FLAGS);

    turnVisible();
    focus();
    turnVisible();
    await settle();
    expect(client.flagCalls).toBe(before + 1);

    client.release(FakeClient.FLAGS);
    await waitFor(() => expect(client.flagCalls).toBe(before + 2));
    await settle();
    expect(client.flagCalls).toBe(before + 2);
  });

  it("asks no more once the app is gone", async () => {
    const client = clientWithBoards();
    const { unmount } = await openWithAnswer(client);
    await settle();
    const before = client.flagCalls;

    unmount();
    turnVisible();
    focus();
    pass(30_000);
    await settle();

    expect(client.flagCalls).toBe(before);
  });

  it("starts no second run beside one still going, and one more after it", async () => {
    const client = clientWithBoards();
    const { container, rerender } = await openWithAnswer(client);
    await settle();
    const before = client.flagCalls;
    client.hold(FakeClient.FLAGS);

    pass(30_000);
    expect(client.flagCalls).toBe(before + 1);

    pass(90_000);
    focus();
    rerender(<App client={client} />);
    fireEvent.click(screen.getByRole("button", { name: "Containers" }));
    await screen.findByRole("heading", { name: "Containers" });
    pass(30_000);
    expect(client.flagCalls).toBe(before + 1);
    expect(screen.queryByRole("status")).toBeNull();
    fireEvent.click(screen.getByRole("button", { name: "Context" }));
    await screen.findByRole("heading", { name: "Context" });
    await waitFor(() =>
      expect(marks(container)).toEqual({ payments: "⚠ task" }),
    );

    client.release(FakeClient.FLAGS);
    await waitFor(() => expect(client.flagCalls).toBe(before + 2));
    await settle();
    expect(client.flagCalls).toBe(before + 2);
  });

  it("asks only for the flags", async () => {
    const client = clientWithBoards();
    await openWithAnswer(client);
    await settle();
    const { listCalls } = client;
    const opened = client.openCalls.length;

    pass(30_000);
    focus();
    await waitFor(() => expect(client.flagCalls).toBeGreaterThan(1));
    await settle();

    expect(client.listCalls).toBe(listCalls);
    expect(client.openCalls).toHaveLength(opened);
  });

  it("takes the marks away when a later run fails, and brings them back with the next", async () => {
    const client = clientWithBoards();
    const { container } = await openWithAnswer(client);
    await settle();

    client.flags = "git failed";
    pass(30_000);
    await waitFor(() =>
      expect(screen.getByRole("status").textContent).toContain("git failed"),
    );
    expect(marks(container)).toEqual({ payments: null });
    await settle();

    client.flags = [{ element: "payments", priority: "hint" }];
    pass(30_000);
    await waitFor(() =>
      expect(marks(container)).toEqual({ payments: "⚠ hint" }),
    );
    expect(screen.queryByRole("status")).toBeNull();
  });

  it("asks once per tick under StrictMode, and stops when the app closes", async () => {
    const client = clientWithBoards();
    const { unmount } = render(
      <StrictMode>
        <App client={client} />
      </StrictMode>,
    );
    await screen.findByRole("button", { name: "Context" });
    await waitFor(() => expect(client.flagCalls).toBeGreaterThan(0));
    await settle();
    const before = client.flagCalls;

    pass(30_000);
    await waitFor(() => expect(client.flagCalls).toBe(before + 1));
    await settle();
    expect(client.flagCalls).toBe(before + 1);

    unmount();
    pass(90_000);
    focus();
    await settle();
    expect(client.flagCalls).toBe(before + 1);
  });
});

describe("App", () => {
  it("shows the titles of the boards the client lists", async () => {
    render(<App client={clientWithBoards()} />);

    const items = await screen.findAllByRole("listitem");
    expect(items.map((item) => item.textContent)).toEqual([
      "Containers",
      "Context",
    ]);
  });

  it("shows the boards as the tree the canvas lists, each indented by its depth", async () => {
    render(
      <App
        client={
          new FakeClient([
            { id: "context", title: "Context", depth: 0 },
            { id: "containers", title: "Containers", depth: 1 },
            { id: "docs", title: "Docs", depth: 0 },
          ])
        }
      />,
    );

    fireEvent.click(await screen.findByRole("button", { name: "Open branch" }));

    const items = await screen.findAllByRole("listitem");
    expect(
      items.map((item) => [item.textContent, item.style.paddingInlineStart]),
    ).toEqual([
      ["▾Context", "0rem"],
      ["Containers", "1rem"],
      ["Docs", "0rem"],
    ]);
  });

  it("shows the new list after the project's files change", async () => {
    const client = new FakeClient([
      { id: "context", title: "Context", depth: 0 },
    ]);
    render(<App client={client} />);
    await screen.findByText("Context");
    await waitFor(() => expect(client.listening).toBe(2));

    client.boards = [
      { id: "containers", title: "Containers", depth: 0 },
      { id: "context", title: "Context", depth: 0 },
    ];
    client.change();

    expect(await screen.findByText("Containers")).toBeDefined();
  });

  it("shows the core's message outside a project", async () => {
    render(<App client={new FakeClient(NO_PROJECT)} />);

    expect((await screen.findByRole("alert")).textContent).toBe(NO_PROJECT);
  });

  it("opens the board a click names", async () => {
    const client = clientWithBoards();
    render(<App client={client} />);

    fireEvent.click(await screen.findByRole("button", { name: "Context" }));

    expect((await screen.findByRole("heading", { level: 2 })).textContent).toBe(
      "Context",
    );
    expect(await screen.findByText("Payments")).toBeDefined();
    expect(client.openCalls).toEqual(["context"]);
  });

  it("asks again for the open board after the project's files change", async () => {
    const client = clientWithBoards();
    render(<App client={client} />);
    fireEvent.click(await screen.findByRole("button", { name: "Context" }));
    await screen.findByText("Payments");
    await waitFor(() => expect(client.listening).toBe(3));

    client.views.context = view("context", "Context", ["Payments", "Shipping"]);
    client.change();

    expect(await screen.findByText("Shipping")).toBeDefined();
  });

  it("shows the core's message in place of a board that cannot be opened, and keeps the list", async () => {
    const client = clientWithBoards();
    client.views.context = "no board has the ID context";
    render(<App client={client} />);

    fireEvent.click(await screen.findByRole("button", { name: "Context" }));

    expect((await screen.findByRole("alert")).textContent).toBe(
      "no board has the ID context",
    );
    fireEvent.click(screen.getByRole("button", { name: "Containers" }));
    expect(await screen.findByText("Orders")).toBeDefined();
  });

  it("ignores a late answer for a board that is no longer open", async () => {
    const client = clientWithBoards();
    client.hold("context");
    render(<App client={client} />);
    fireEvent.click(await screen.findByRole("button", { name: "Context" }));
    fireEvent.click(screen.getByRole("button", { name: "Containers" }));
    await screen.findByText("Orders");

    client.release("context");
    await new Promise((resolve) => setTimeout(resolve, 0));

    expect(screen.getByRole("heading", { level: 2 }).textContent).toBe(
      "Containers",
    );
    expect(screen.queryByText("Payments")).toBeNull();
  });

  it("shows markup in a board's title as text", async () => {
    const title = "<img src=x onerror=alert(1)>";
    const client = new FakeClient([{ id: "board", title, depth: 0 }]);
    client.views = { board: view("board", title) };
    const { container } = render(<App client={client} />);

    fireEvent.click(await screen.findByRole("button", { name: title }));

    expect((await screen.findByRole("heading", { level: 2 })).textContent).toBe(
      title,
    );
    expect(container.querySelector("img")).toBeNull();
  });

  it("shows markup in a board's error as text", async () => {
    const message = "<img src=x onerror=alert(1)>";
    const client = clientWithBoards();
    client.views.context = message;
    const { container } = render(<App client={client} />);

    fireEvent.click(await screen.findByRole("button", { name: "Context" }));

    expect((await screen.findByRole("alert")).textContent).toBe(message);
    expect(container.querySelector("img")).toBeNull();
  });

  it("releases a pin with one click, and the marker goes with the reload", async () => {
    const client = clientWithBoards();
    const pinned = view("context", "Context", ["Payments"]);
    client.views.context = {
      ...pinned,
      placements: pinned.placements.map((placement) => ({
        ...placement,
        pinned: true,
      })),
    };
    render(<App client={client} />);
    fireEvent.click(await screen.findByRole("button", { name: "Context" }));

    fireEvent.click(await screen.findByRole("button", { name: "Release pin" }));

    await waitFor(() =>
      expect(client.applied).toEqual([
        [
          {
            op: "move_element",
            element: { existing: "payments" },
            board: { existing: "context" },
            x: 0,
            y: 0,
            pinned: false,
          },
        ],
      ]),
    );
    client.views.context = pinned;
    client.change();
    await waitFor(() =>
      expect(screen.queryByRole("button", { name: "Release pin" })).toBeNull(),
    );
  });

  it("stops listening when it is closed", async () => {
    const client = new FakeClient([]);
    const { unmount } = render(<App client={client} />);
    await waitFor(() => expect(client.listening).toBe(2));

    unmount();

    await waitFor(() => expect(client.listening).toBe(0));
  });

  describe("moving between boards", () => {
    /** The board `outer` whose box owns `inner`, with a trail back on `inner`. */
    function nestedClient(title = "Outer"): FakeClient {
      const client = new FakeClient([
        { id: "outer", title, depth: 0 },
        { id: "inner", title: "Inner", depth: 1 },
      ]);
      client.views = {
        outer: boardView({
          id: "outer",
          title,
          placements: [
            placedBox("shop", {
              inner_boards: [{ id: "inner", title: "Inner" }],
            }),
          ],
        }),
        inner: boardView({
          id: "inner",
          title: "Inner",
          trail: [{ id: "outer", title }],
        }),
      };
      return client;
    }

    const current = () =>
      screen
        .getByRole("list", { name: "Boards" })
        .querySelector('[aria-current="page"]')?.textContent;

    it("opens an inner board from its box and goes back with the breadcrumb", async () => {
      const client = nestedClient();
      render(<App client={client} />);
      fireEvent.click(await screen.findByRole("button", { name: "Outer" }));

      fireEvent.click(await screen.findByRole("button", { name: "▸ Inner" }));

      await screen.findByRole("heading", { name: "Inner" });
      expect(current()).toBe("Inner");
      const breadcrumb = screen.getByRole("navigation", {
        name: "Boards above",
      });
      const back = within(breadcrumb).getByRole("button", { name: "Outer" });
      back.focus();
      expect(document.activeElement).toBe(back);

      fireEvent.click(back);

      await screen.findByRole("heading", { name: "Outer" });
      expect(current()).toBe("Outer");
      expect(
        screen.queryByRole("navigation", { name: "Boards above" }),
      ).toBeNull();
      expect(client.applied).toEqual([]);
    });

    describe("with one press", () => {
      /**
       * `outer`, whose box `shop` owns `inner`; `inner`, a board
       * with the trail back to `outer` and the box `deep`, which owns
       * `deeper`, at the same place as `shop`; and `deeper`.
       */
      function threeLevels(): FakeClient {
        const client = new FakeClient([
          { id: "outer", title: "Outer", depth: 0 },
          { id: "inner", title: "Inner", depth: 1 },
          { id: "deeper", title: "Deeper", depth: 2 },
        ]);
        const owning = (element: string, id: string, title: string) =>
          placedBox(element, {
            shown: {
              kind: "element",
              type_ref: "t",
              name: element,
              summary: null,
            },
            inner_boards: [{ id, title }],
          });
        client.views = {
          outer: boardView({
            id: "outer",
            title: "Outer",
            placements: [owning("shop", "inner", "Inner")],
          }),
          inner: boardView({
            id: "inner",
            title: "Inner",
            trail: [{ id: "outer", title: "Outer" }],
            placements: [owning("deep", "deeper", "Deeper")],
          }),
          deeper: boardView({ id: "deeper", title: "Deeper" }),
        };
        return client;
      }

      /** Opens `inner` with the first click of a press on `shop`'s button. */
      async function firstClickOpensInner() {
        const client = threeLevels();
        const rendered = render(<App client={client} />);
        fireEvent.click(await screen.findByRole("button", { name: "Outer" }));
        const button = await screen.findByRole("button", { name: "▸ Inner" });
        fireEvent.mouseDown(button, { detail: 1 });
        fireEvent.click(button, { detail: 1 });
        await screen.findByRole("heading", { name: "Inner" });
        return { client, ...rendered };
      }

      /**
       * A mouse event of the second click of a press, as a browser sends it:
       * React Flow's drag reads the window from it.
       */
      const secondClick = (type: string) => {
        const event = new MouseEvent(type, {
          bubbles: true,
          cancelable: true,
          detail: 2,
        });
        Object.defineProperty(event, "view", { value: window });
        return event;
      };
      /** The rest of a double click whose first click opened a board, on `target`. */
      const restOfPress = (target: Element) => {
        for (const type of ["mousedown", "mouseup", "click", "dblclick"]) {
          fireEvent(target, secondClick(type));
        }
      };
      const settleAll = () =>
        act(() => new Promise<void>((done) => setTimeout(done, 0)));

      it("opens no further board when the rest lands on a ▸ button", async () => {
        await firstClickOpensInner();

        restOfPress(await screen.findByRole("button", { name: "▸ Deeper" }));
        await settleAll();

        expect(screen.getByRole("heading", { name: "Inner" })).toBeDefined();
      });

      it("opens no further board when the rest lands on a box with an inner board", async () => {
        const { container } = await firstClickOpensInner();
        const deep = await waitFor(() => {
          const found = container.querySelector(
            '.react-flow__node[data-id="deep"]',
          );
          if (found === null) {
            throw new Error("no box");
          }
          return found;
        });

        restOfPress(deep);
        await settleAll();

        expect(screen.getByRole("heading", { name: "Inner" })).toBeDefined();
      });

      it("opens no further board when the rest lands on a breadcrumb", async () => {
        await firstClickOpensInner();

        restOfPress(
          within(
            screen.getByRole("navigation", { name: "Boards above" }),
          ).getByRole("button", { name: "Outer" }),
        );
        await settleAll();

        expect(screen.getByRole("heading", { name: "Inner" })).toBeDefined();
      });

      it("starts no note when the rest lands on the empty pane", async () => {
        const { container } = await firstClickOpensInner();
        const pane = await waitFor(() => {
          const found = container.querySelector(".react-flow__pane");
          if (found === null) {
            throw new Error("no pane");
          }
          return found;
        });

        restOfPress(pane);
        await settleAll();

        expect(screen.queryByRole("textbox")).toBeNull();
      });

      it("opens a board with a later press, by the mouse or by Enter", async () => {
        await firstClickOpensInner();
        const deeper = await screen.findByRole("button", { name: "▸ Deeper" });
        restOfPress(deeper);
        await settleAll();

        fireEvent.keyDown(deeper, { key: "Enter" });
        fireEvent.click(deeper, { detail: 0 });
        await screen.findByRole("heading", { name: "Deeper" });

        fireEvent.click(screen.getByRole("button", { name: "Outer" }), {
          detail: 0,
        });
        await screen.findByRole("heading", { name: "Outer" });
        const shop = await screen.findByRole("button", { name: "▸ Inner" });
        fireEvent.mouseDown(shop, { detail: 1 });
        fireEvent.click(shop, { detail: 1 });
        expect(
          await screen.findByRole("heading", { name: "Inner" }),
        ).toBeDefined();
      });
    });

    it("shows a title in the breadcrumb as text", async () => {
      const title = "<img src=x onerror=alert(1)>";
      render(<App client={nestedClient(title)} />);
      fireEvent.click(
        await screen.findByRole("button", { name: "Open branch" }),
      );
      fireEvent.click(await screen.findByRole("button", { name: "Inner" }));

      const breadcrumb = await screen.findByRole("navigation", {
        name: "Boards above",
      });

      expect(breadcrumb.textContent).toContain(title);
      expect(breadcrumb.querySelector("img, script, a")).toBeNull();
    });

    it("shows the core's message for a board that has gone and keeps the list", async () => {
      const client = nestedClient();
      render(<App client={client} />);
      fireEvent.click(
        await screen.findByRole("button", { name: "Open branch" }),
      );
      fireEvent.click(await screen.findByRole("button", { name: "Inner" }));
      await screen.findByRole("heading", { name: "Inner" });
      delete client.views["outer"];

      fireEvent.click(
        within(
          screen.getByRole("navigation", { name: "Boards above" }),
        ).getByRole("button", { name: "Outer" }),
      );

      expect((await screen.findByRole("alert")).textContent).toContain("outer");
      expect(screen.getByRole("list", { name: "Boards" })).toBeDefined();
    });
  });
});

describe("the board tree's branches", () => {
  /**
   * `context` > `containers` > `components`, and `docs` > `notes`; each board
   * a view whose box owns the board below it, with the trail back.
   */
  function treeClient(
    ids: { top: string; middle: string } = {
      top: "context",
      middle: "containers",
    },
    topTitle = "Context",
  ): FakeClient {
    const { top, middle } = ids;
    const client = new FakeClient([
      { id: top, title: topTitle, depth: 0 },
      { id: middle, title: "Containers", depth: 1 },
      { id: "components", title: "Components", depth: 2 },
      { id: "docs", title: "Docs", depth: 0 },
      { id: "notes", title: "Notes", depth: 1 },
    ]);
    const owning = (board: string, inner: string, title: string) =>
      placedBox(`${board}-box`, { inner_boards: [{ id: inner, title }] });
    client.views = {
      [top]: boardView({
        id: top,
        title: topTitle,
        placements: [owning(top, middle, "Containers")],
      }),
      [middle]: boardView({
        id: middle,
        title: "Containers",
        placements: [owning(middle, "components", "Components")],
        trail: [{ id: top, title: topTitle }],
      }),
      components: boardView({
        id: "components",
        title: "Components",
        trail: [
          { id: top, title: topTitle },
          { id: middle, title: "Containers" },
        ],
      }),
      docs: boardView({ id: "docs", title: "Docs" }),
      notes: boardView({ id: "notes", title: "Notes" }),
    };
    return client;
  }

  /** The titles the list shows, top to bottom. */
  function shownTitles(): (string | null | undefined)[] {
    return within(screen.getByRole("list", { name: "Boards" }))
      .getAllByRole("listitem")
      .map((item) => within(item).getAllByRole("button").at(-1)?.textContent);
  }

  /** The toggle of the row of the board titled `title`. */
  function toggleOf(title: string): HTMLElement {
    const row = within(screen.getByRole("list", { name: "Boards" }))
      .getByRole("button", { name: title })
      .closest("li");
    if (row === null) {
      throw new Error(`no row for ${title}`);
    }
    return within(row).getByRole("button", { name: /branch$/ });
  }

  /** Opens the board titled `title` from the list. */
  function openFromList(title: string) {
    fireEvent.click(
      within(screen.getByRole("list", { name: "Boards" })).getByRole("button", {
        name: title,
      }),
    );
  }

  it("shows only the top-level boards when the canvas starts", async () => {
    render(<App client={treeClient()} />);

    await screen.findByRole("button", { name: "Context" });

    expect(shownTitles()).toEqual(["Context", "Docs"]);
    expect(toggleOf("Context").getAttribute("aria-expanded")).toBe("false");
  });

  it("gives a toggle only to a board with boards below it", async () => {
    render(<App client={treeClient()} />);
    await screen.findByRole("button", { name: "Context" });

    fireEvent.click(toggleOf("Context"));

    const containers = screen.getByRole("button", { name: "Containers" });
    expect(toggleOf("Containers")).toBeDefined();
    fireEvent.click(toggleOf("Containers"));
    const components = screen
      .getByRole("button", { name: "Components" })
      .closest("li");
    expect(containers).toBeDefined();
    expect(
      within(components as HTMLElement).queryByRole("button", {
        name: /branch$/,
      }),
    ).toBeNull();
  });

  it("opens and closes a branch with its toggle, writing and opening nothing", async () => {
    const client = treeClient();
    render(<App client={client} />);
    await screen.findByRole("button", { name: "Context" });

    fireEvent.click(toggleOf("Context"));
    expect(shownTitles()).toEqual(["Context", "Containers", "Docs"]);
    expect(toggleOf("Context").getAttribute("aria-expanded")).toBe("true");
    expect(toggleOf("Context").getAttribute("aria-label")).toBe("Close branch");

    fireEvent.click(toggleOf("Context"));
    expect(shownTitles()).toEqual(["Context", "Docs"]);

    expect(client.openCalls).toEqual([]);
    expect(client.applied).toEqual([]);
    expect(
      screen
        .getByRole("list", { name: "Boards" })
        .querySelector('[aria-current="page"]'),
    ).toBeNull();
  });

  it("opens every branch on the way to a board opened from a box, also one the user closed", async () => {
    render(<App client={treeClient()} />);
    await screen.findByRole("button", { name: "Context" });
    fireEvent.click(toggleOf("Context"));
    openFromList("Containers");
    await screen.findByRole("heading", { name: "Containers" });
    fireEvent.click(toggleOf("Context"));
    expect(shownTitles()).toEqual(["Context", "Docs"]);

    fireEvent.click(
      await screen.findByRole("button", { name: "▸ Components" }),
    );

    await screen.findByRole("heading", { name: "Components" });
    expect(shownTitles()).toEqual([
      "Context",
      "Containers",
      "Components",
      "Docs",
    ]);
  });

  it("opens the way to a board opened with the breadcrumb", async () => {
    render(<App client={treeClient()} />);
    await screen.findByRole("button", { name: "Context" });
    fireEvent.click(toggleOf("Context"));
    fireEvent.click(toggleOf("Containers"));
    openFromList("Components");
    await screen.findByRole("heading", { name: "Components" });
    fireEvent.click(toggleOf("Context"));
    expect(shownTitles()).toEqual(["Context", "Docs"]);

    fireEvent.click(
      within(
        screen.getByRole("navigation", { name: "Boards above" }),
      ).getByRole("button", { name: "Containers" }),
    );

    await screen.findByRole("heading", { name: "Containers" });
    expect(shownTitles()).toEqual([
      "Context",
      "Containers",
      "Components",
      "Docs",
    ]);
  });

  it("keeps the user's toggles when the list loads again after a change of the files", async () => {
    const client = treeClient();
    render(<App client={client} />);
    await screen.findByRole("button", { name: "Context" });
    await waitFor(() => expect(client.listening).toBe(2));
    fireEvent.click(toggleOf("Docs"));
    expect(shownTitles()).toEqual(["Context", "Docs", "Notes"]);
    const calls = client.listCalls;

    client.change();

    await waitFor(() => expect(client.listCalls).toBeGreaterThan(calls));
    await waitFor(() =>
      expect(shownTitles()).toEqual(["Context", "Docs", "Notes"]),
    );
  });

  it("keeps the user's toggles through a reload that fails", async () => {
    const client = treeClient();
    render(<App client={client} />);
    await screen.findByRole("button", { name: "Context" });
    await waitFor(() => expect(client.listening).toBe(2));
    fireEvent.click(toggleOf("Docs"));
    const boards = client.boards;

    client.boards = "the board files cannot be read";
    client.change();
    await screen.findByText("the board files cannot be read");
    client.boards = boards;
    client.change();

    await waitFor(() =>
      expect(shownTitles()).toEqual(["Context", "Docs", "Notes"]),
    );
  });

  it("shows the list after a reload that no longer holds the open board or the board above it", async () => {
    const client = treeClient();
    render(<App client={client} />);
    await screen.findByRole("button", { name: "Context" });
    await waitFor(() => expect(client.listening).toBe(2));
    fireEvent.click(toggleOf("Context"));
    openFromList("Containers");
    await screen.findByRole("heading", { name: "Containers" });

    client.boards = [
      { id: "docs", title: "Docs", depth: 0 },
      { id: "notes", title: "Notes", depth: 1 },
    ];
    client.change();

    await waitFor(() => expect(shownTitles()).toEqual(["Docs"]));
  });

  it("shows a title with markup on a board with boards below it as text, with a toggle of fixed name", async () => {
    const title = "<img src=x onerror=alert(1)>";
    const { container } = render(<App client={treeClient(undefined, title)} />);
    await screen.findByRole("button", { name: title });

    expect(toggleOf(title).getAttribute("aria-label")).toBe("Open branch");
    expect(toggleOf(title).textContent).toBe("▸");
    expect(container.querySelector("nav img")).toBeNull();
  });

  it("treats boards named __proto__ and constructor like any other", async () => {
    render(
      <App
        client={treeClient(
          { top: "__proto__", middle: "constructor" },
          "Proto",
        )}
      />,
    );
    await screen.findByRole("button", { name: "Proto" });

    fireEvent.click(toggleOf("Proto"));
    expect(shownTitles()).toEqual(["Proto", "Containers", "Docs"]);
    fireEvent.click(toggleOf("Containers"));
    openFromList("Components");
    await screen.findByRole("heading", { name: "Components" });
    fireEvent.click(toggleOf("Proto"));
    expect(shownTitles()).toEqual(["Proto", "Docs"]);

    fireEvent.click(
      within(
        screen.getByRole("navigation", { name: "Boards above" }),
      ).getByRole("button", { name: "Containers" }),
    );

    await screen.findByRole("heading", { name: "Containers" });
    expect(shownTitles()).toEqual([
      "Proto",
      "Containers",
      "Components",
      "Docs",
    ]);
  });
});

describe("the sidebar", () => {
  function sidebarClient(title = "Context"): FakeClient {
    const client = new FakeClient([
      { id: "context", title, depth: 0 },
      { id: "containers", title: "Containers", depth: 1 },
    ]);
    client.views = {
      context: boardView({ id: "context", title }),
      containers: boardView({ id: "containers", title: "Containers" }),
    };
    return client;
  }

  async function renderSidebar(title?: string) {
    const rendered = render(<App client={sidebarClient(title)} />);
    // Also while the sidebar is collapsed, which hides the list.
    await screen.findByRole("list", { name: "Boards", hidden: true });
    return rendered;
  }

  function navOf(container: HTMLElement): HTMLElement {
    const nav = container.querySelector<HTMLElement>(".sidebar > nav");
    if (nav === null) {
      throw new Error("no sidebar");
    }
    return nav;
  }

  const handle = () =>
    screen.getByRole("separator", { name: "Width of the sidebar" });

  /** Drags the handle by `pixels`, a rem being 16 pixels in jsdom. */
  function drag(pixels: number) {
    fireEvent.mouseDown(handle(), { button: 0, clientX: 100 });
    fireEvent.mouseMove(window, { clientX: 100 + pixels });
    fireEvent.mouseUp(window, { clientX: 100 + pixels });
  }

  it("starts at the default width, expanded, with nothing stored", async () => {
    const { container } = await renderSidebar();

    expect(navOf(container).style.width).toBe("16rem");
    expect(handle().getAttribute("aria-valuenow")).toBe("16");
    expect(navOf(container).hidden).toBe(false);
  });

  it("changes its width with a drag of the handle and stores it", async () => {
    const { container } = await renderSidebar();

    drag(64);

    expect(navOf(container).style.width).toBe("20rem");
    expect(handle().getAttribute("aria-valuenow")).toBe("20");
    expect(window.localStorage.getItem("abyme.sidebar.width")).toBe("20");
  });

  it("keeps its width between the bounds however far the handle is dragged", async () => {
    const { container } = await renderSidebar();

    drag(10_000);
    expect(navOf(container).style.width).toBe("40rem");
    expect(window.localStorage.getItem("abyme.sidebar.width")).toBe("40");

    drag(-10_000);
    expect(navOf(container).style.width).toBe("10rem");
    expect(window.localStorage.getItem("abyme.sidebar.width")).toBe("10");
  });

  it("changes its width with the arrow keys on the handle, within the bounds", async () => {
    const { container } = await renderSidebar();

    fireEvent.keyDown(handle(), { key: "ArrowRight" });
    expect(navOf(container).style.width).toBe("17rem");
    for (let step = 0; step < 20; step += 1) {
      fireEvent.keyDown(handle(), { key: "ArrowLeft" });
    }
    expect(navOf(container).style.width).toBe("10rem");
    expect(window.localStorage.getItem("abyme.sidebar.width")).toBe("10");
  });

  it("goes back to the default width on a double-click on the handle", async () => {
    const { container } = await renderSidebar();
    drag(64);

    fireEvent.doubleClick(handle());

    expect(navOf(container).style.width).toBe("16rem");
    expect(window.localStorage.getItem("abyme.sidebar.width")).toBe("16");
  });

  it("collapses and expands, keeping the tree's branches", async () => {
    const { container } = await renderSidebar();
    fireEvent.click(screen.getByRole("button", { name: "Open branch" }));

    fireEvent.click(screen.getByRole("button", { name: "Collapse sidebar" }));

    expect(navOf(container).hidden).toBe(true);
    expect(screen.queryByRole("separator")).toBeNull();
    expect(window.localStorage.getItem("abyme.sidebar.collapsed")).toBe(
      "collapsed",
    );

    fireEvent.click(screen.getByRole("button", { name: "Expand sidebar" }));

    expect(navOf(container).hidden).toBe(false);
    expect(screen.getByRole("button", { name: "Containers" })).toBeDefined();
    expect(window.localStorage.getItem("abyme.sidebar.collapsed")).toBe("");
  });

  it("moves the focus to the button that undoes a collapse or an expansion", async () => {
    await renderSidebar();
    const collapse = screen.getByRole("button", { name: "Collapse sidebar" });
    collapse.focus();

    fireEvent.click(collapse);

    const expand = screen.getByRole("button", { name: "Expand sidebar" });
    expect(document.activeElement).toBe(expand);

    fireEvent.click(expand);

    expect(document.activeElement).toBe(
      screen.getByRole("button", { name: "Collapse sidebar" }),
    );
  });

  it("takes no focus when it starts collapsed", async () => {
    window.localStorage.setItem("abyme.sidebar.collapsed", "collapsed");

    await renderSidebar();

    expect(document.activeElement).toBe(document.body);
  });

  it("starts as the user left it, from the webview's storage", async () => {
    window.localStorage.setItem("abyme.sidebar.width", "24");
    window.localStorage.setItem("abyme.sidebar.collapsed", "collapsed");

    const { container } = await renderSidebar();

    expect(navOf(container).style.width).toBe("24rem");
    expect(navOf(container).hidden).toBe(true);
    expect(
      screen.getByRole("button", { name: "Expand sidebar" }),
    ).toBeDefined();
  });

  it.each([
    ["abc", "16rem"],
    ["NaN", "16rem"],
    ["Infinity", "16rem"],
    ["-Infinity", "16rem"],
    ["1e400", "16rem"],
    ["", "16rem"],
    ["  ", "16rem"],
    ["16rem", "16rem"],
    ["</style><img src=x>", "16rem"],
    ["5", "10rem"],
    ["100", "40rem"],
    ["12.5", "12.5rem"],
  ])("starts at %j stored as %s", async (stored, shown) => {
    window.localStorage.setItem("abyme.sidebar.width", stored);

    const { container } = await renderSidebar();

    expect(navOf(container).style.width).toBe(shown);
    expect(handle().getAttribute("aria-valuenow")).toBe(
      shown.replace("rem", ""),
    );
    expect(container.querySelector("img")).toBeNull();
  });

  it.each(["TRUE", "1", " collapsed", "{}"])(
    "starts expanded with %j stored as its state",
    async (stored) => {
      window.localStorage.setItem("abyme.sidebar.collapsed", stored);

      const { container } = await renderSidebar();

      expect(navOf(container).hidden).toBe(false);
    },
  );

  it("starts at the default width and expanded when the storage cannot be read", async () => {
    vi.spyOn(Storage.prototype, "getItem").mockImplementation(() => {
      throw new DOMException("denied", "SecurityError");
    });

    const { container } = await renderSidebar();

    expect(navOf(container).style.width).toBe("16rem");
    expect(navOf(container).hidden).toBe(false);
  });

  it("keeps working when the storage refuses a write", async () => {
    vi.spyOn(Storage.prototype, "setItem").mockImplementation(() => {
      throw new DOMException("full", "QuotaExceededError");
    });
    const { container } = await renderSidebar();

    drag(64);
    fireEvent.click(screen.getByRole("button", { name: "Collapse sidebar" }));

    expect(navOf(container).style.width).toBe("20rem");
    expect(navOf(container).hidden).toBe(true);
  });

  it("stores nothing but its width and its state", async () => {
    await renderSidebar();

    drag(64);
    fireEvent.click(screen.getByRole("button", { name: "Collapse sidebar" }));

    const keys = Array.from(
      { length: window.localStorage.length },
      (_, index) => window.localStorage.key(index),
    );
    expect(keys.sort()).toEqual([
      "abyme.sidebar.collapsed",
      "abyme.sidebar.width",
    ]);
  });

  it("stops a drag when the window loses the mouse", async () => {
    const { container } = await renderSidebar();
    fireEvent.mouseDown(handle(), { button: 0, clientX: 100 });
    fireEvent.mouseMove(window, { clientX: 164 });

    fireEvent.blur(window);
    fireEvent.mouseMove(window, { clientX: 400 });

    expect(navOf(container).style.width).toBe("20rem");
    expect(window.localStorage.getItem("abyme.sidebar.width")).toBe("20");
  });

  it("shows a title in full as the tooltip of its button, as text", async () => {
    const title = "<img src=x onerror=alert(1)>";
    const { container } = await renderSidebar(title);

    const button = screen.getByRole("button", { name: title });

    expect(button.textContent).toBe(title);
    expect(button.getAttribute("title")).toBe(title);
    expect(container.querySelector("img")).toBeNull();
  });
});
