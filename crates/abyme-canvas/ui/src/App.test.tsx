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
import type { BoardView } from "./bindings";
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
    { id: "containers", title: "Containers" },
    { id: "context", title: "Context" },
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

  it("shows the new list after the project's files change", async () => {
    const client = new FakeClient([{ id: "context", title: "Context" }]);
    render(<App client={client} />);
    await screen.findByText("Context");
    await waitFor(() => expect(client.listening).toBe(2));

    client.boards = [
      { id: "containers", title: "Containers" },
      { id: "context", title: "Context" },
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
    const client = new FakeClient([{ id: "board", title }]);
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
        { id: "inner", title: "Inner" },
        { id: "outer", title },
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

    it("shows a title in the breadcrumb as text", async () => {
      const title = "<img src=x onerror=alert(1)>";
      render(<App client={nestedClient(title)} />);
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
