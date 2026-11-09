import {
  act,
  cleanup,
  fireEvent,
  render,
  screen,
  waitFor,
} from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import type { XYPosition } from "@xyflow/react";

import type { BoardView } from "./bindings";
import { BoardCanvas } from "./BoardCanvas";
import { nextStamp } from "./boards";
import { ReviewMarks } from "./ElementBox";
import type { Move } from "./nodes";
import type { NoteParts } from "./notes";
import { boardView, placedBox, placedElement } from "./testBoards";
import { measureBoxes } from "./testLayout";

afterEach(cleanup);

/** A board of two boxes in the given mode. */
function boardIn(mode: BoardView["mode"]): BoardView {
  return boardView({
    mode,
    placements: [
      placedElement("orders"),
      placedElement("payments", { x: 200 }),
    ],
  });
}

/** A sized canvas of `view`. */
function canvasOf(
  view: BoardView,
  move: (moves: Move[]) => Promise<void>,
  onOpen: (board: string) => void = () => {},
  release: (element: string, position: XYPosition) => void = () => {},
  writeNote: (element: string, parts: NoteParts) => Promise<boolean> = () =>
    Promise.resolve(true),
  createNote: (
    position: XYPosition,
    parts: NoteParts,
  ) => Promise<string | null> = () => Promise.resolve("new-note"),
  deleteNotes: (elements: string[]) => void = () => {},
  askedAt: number | null = null,
) {
  return (
    <div style={{ width: 800, height: 600 }}>
      <BoardCanvas
        view={view}
        askedAt={askedAt}
        move={move}
        release={release}
        writeNote={writeNote}
        createNote={createNote}
        deleteNotes={deleteNotes}
        onMoveFailed={() => {}}
        refuseNote={() => Promise.resolve(false)}
        onOpen={onOpen}
      />
    </div>
  );
}

const DRAGGABLE_BOX = ".react-flow__node.draggable";

/** Two boxes in a row and one relation between them with `label`. */
function boardWithArrow(label: string | null, x = 0): BoardView {
  return boardView({
    placements: [
      placedElement("orders", { x }),
      placedElement("payments", { y: 300 }),
    ],
    relations: [{ from: "orders", kind: "uses", to: "payments", label }],
  });
}

describe("BoardCanvas", () => {
  it("offers a drag on every board but a mirrored one", () => {
    const move = () => Promise.resolve();

    for (const [mode, boxes] of [
      ["mirrored", 0],
      ["free", 2],
      [null, 2],
    ] as const) {
      const { container, unmount } = render(canvasOf(boardIn(mode), move));

      expect(container.querySelectorAll(DRAGGABLE_BOX)).toHaveLength(boxes);
      unmount();
    }
  });

  it("takes the drag away when an open board turns mirrored", () => {
    const move = () => Promise.resolve();
    const { container, rerender } = render(canvasOf(boardIn("free"), move));
    expect(container.querySelectorAll(DRAGGABLE_BOX)).toHaveLength(2);

    rerender(canvasOf(boardIn("mirrored"), move));

    expect(container.querySelectorAll(DRAGGABLE_BOX)).toHaveLength(0);
  });

  it("moves a selected box with an arrow key, but not on a mirrored board", () => {
    for (const [mode, calls] of [
      ["free", 1],
      ["mirrored", 0],
    ] as const) {
      const move = vi.fn(() => Promise.resolve());
      const { container, unmount } = render(canvasOf(boardIn(mode), move));
      const box = container.querySelector(".react-flow__node");
      if (box === null) {
        throw new Error("no box");
      }

      fireEvent.keyDown(box, { key: "Enter" });
      fireEvent.keyDown(box, { key: "ArrowRight" });

      expect(move).toHaveBeenCalledTimes(calls);
      unmount();
    }
  });

  it("shows one box per placement with what the projection says of it", () => {
    const view = boardView({
      placements: [
        placedBox("orders"),
        placedBox("payments", {
          x: 200,
          shown: {
            kind: "element",
            type_ref: "c4/container",
            name: "Payments",
            summary: null,
          },
        }),
      ],
    });

    const { container } = render(
      <div style={{ width: 800, height: 600 }}>
        <BoardCanvas
          view={view}
          move={() => Promise.resolve()}
          release={() => {}}
          writeNote={() => Promise.resolve(true)}
          createNote={() => Promise.resolve("new-note")}
          deleteNotes={() => {}}
          onOpen={() => {}}
          onMoveFailed={() => {}}
          refuseNote={() => Promise.resolve(false)}
        />
      </div>,
    );

    expect(container.querySelectorAll(".react-flow__node")).toHaveLength(2);
    expect(screen.getByText("Missing element orders")).toBeDefined();
    expect(screen.getByText("Payments")).toBeDefined();
  });

  it("draws an arrow per relation, and keeps it through a reload", () => {
    const { container, rerender } = render(
      canvasOf(boardWithArrow(null), () => Promise.resolve()),
    );
    const path = () =>
      container.querySelector(".react-flow__edge-path")?.getAttribute("d");
    expect(path()).toBeTruthy();
    const before = path();

    rerender(canvasOf(boardWithArrow(null), () => Promise.resolve()));

    expect(path()).toBe(before);
  });

  it("draws an arrow from the bottom of its source to the top of its target", () => {
    const { container } = render(
      canvasOf(boardWithArrow(null), () => Promise.resolve()),
    );

    const path = container
      .querySelector(".react-flow__edge-path")
      ?.getAttribute("d");

    expect(path).toMatch(/^M90,90\b/);
    expect(path).toMatch(/\b90,300$/);
  });

  it("moves an arrow with its box", () => {
    const { container, rerender } = render(
      canvasOf(boardWithArrow(null), () => Promise.resolve()),
    );
    const path = () =>
      container.querySelector(".react-flow__edge-path")?.getAttribute("d");
    const before = path();

    rerender(canvasOf(boardWithArrow(null, 500), () => Promise.resolve()));

    expect(path()).not.toBe(before);
  });

  it("shows a label as text and makes nothing of markup in it", () => {
    for (const label of [
      "<img src=x onerror=alert(1)>",
      "</text><script>alert(1)</script>",
      "__proto__",
      "constructor",
    ]) {
      const { container, unmount } = render(
        canvasOf(boardWithArrow(label), () => Promise.resolve()),
      );

      expect(
        container.querySelector(".react-flow__edge-text")?.textContent,
      ).toBe(label);
      expect(container.querySelector("img, script")).toBeNull();
      unmount();
    }
  });

  it("keeps the part of a long label it cuts out of the page", () => {
    const { container } = render(
      canvasOf(boardWithArrow(`${"x".repeat(40)}SECRET`), () =>
        Promise.resolve(),
      ),
    );

    expect(new XMLSerializer().serializeToString(container)).not.toContain(
      "SECRET",
    );
  });

  it("does nothing on a click on an arrow or the delete key", () => {
    const move = vi.fn(() => Promise.resolve());
    const { container } = render(canvasOf(boardWithArrow("label"), move));
    const edge = container.querySelector(".react-flow__edge");

    if (edge === null) {
      throw new Error("the arrow was not drawn");
    }
    fireEvent.click(edge);
    fireEvent.keyDown(edge, { key: "Delete" });
    fireEvent.keyDown(edge, { key: "Backspace" });

    expect(move).not.toHaveBeenCalled();
    expect(container.querySelector(".react-flow__edge")).not.toBeNull();
    expect(container.querySelector(".react-flow__edge.selected")).toBeNull();
  });
});

/** A box `shop`, whose element owns `boards`, and a box `plain`, on a free board. */
function boardWithInnerBoards(...boards: string[]): BoardView {
  return boardView({
    mode: "free",
    placements: [
      placedElement("plain", { x: 400 }),
      placedElement("shop", {
        inner_boards: boards.map((title, index) => ({
          id: `inner-${index}`,
          title,
        })),
      }),
    ],
  });
}

/** The box of `element` on a rendered canvas. */
function box(container: HTMLElement, element: string): HTMLElement {
  const found = container.querySelector(
    `.react-flow__node[data-id="${element}"]`,
  );
  if (!(found instanceof HTMLElement)) {
    throw new Error(`no box for ${element}`);
  }
  return found;
}

describe("the marker of a pin", () => {
  it("shows on a pinned box, also one whose file is missing, and on no other", () => {
    const { container } = render(
      canvasOf(
        boardView({
          placements: [
            placedBox("gone", { pinned: true }),
            placedBox("kept", {
              x: 200,
              pinned: true,
              shown: {
                kind: "element",
                type_ref: "t",
                name: "<b>pinned</b>",
                summary: null,
              },
            }),
            placedBox("free", { x: 400 }),
          ],
        }),
        () => Promise.resolve(),
      ),
    );

    for (const pinned of ["gone", "kept"]) {
      expect(
        box(container, pinned).querySelector(".element-pin")?.textContent,
      ).toContain("pinned");
    }
    expect(box(container, "free").querySelector(".element-pin")).toBeNull();
    expect(box(container, "kept").querySelector("b")).toBeNull();
    expect(screen.getAllByRole("button", { name: "Release pin" })).toEqual([
      box(container, "kept").querySelector("button.element-pin"),
    ]);
    expect(box(container, "gone").querySelector("button")).toBeNull();
  });

  it("is plain text with no button on a mirrored board", () => {
    const { container } = render(
      canvasOf(
        boardView({
          mode: "mirrored",
          placements: [placedElement("kept", { pinned: true })],
        }),
        () => Promise.resolve(),
      ),
    );

    expect(box(container, "kept").querySelector("button")).toBeNull();
    expect(
      box(container, "kept").querySelector(".element-pin")?.textContent,
    ).toBe("pinned");
  });

  it("is a button elsewhere that releases the pin where the box lies, and nothing else", () => {
    for (const mode of [null, "free"] as const) {
      const move = vi.fn(() => Promise.resolve());
      const onOpen = vi.fn();
      const release = vi.fn();
      const { unmount } = render(
        canvasOf(
          boardView({
            mode,
            placements: [
              placedElement("kept", {
                x: 200,
                y: 30,
                pinned: true,
                inner_boards: [{ id: "inner", title: "Inner" }],
              }),
            ],
          }),
          move,
          onOpen,
          release,
        ),
      );
      const marker = screen.getByRole("button", { name: "Release pin" });

      fireEvent.click(marker);
      fireEvent.doubleClick(marker);

      expect(release).toHaveBeenCalledWith("kept", { x: 200, y: 30 });
      expect(move).not.toHaveBeenCalled();
      expect(onOpen).not.toHaveBeenCalled();
      unmount();
    }
  });

  it("offers no release on a box without a pin", () => {
    render(
      canvasOf(boardView({ placements: [placedElement("free")] }), () =>
        Promise.resolve(),
      ),
    );

    expect(screen.queryByRole("button", { name: "Release pin" })).toBeNull();
  });

  it("names the release in its own words, whatever the element is called", () => {
    render(
      canvasOf(
        boardView({
          placements: [
            placedBox("kept", {
              pinned: true,
              shown: {
                kind: "element",
                type_ref: "t",
                name: "Keep this pin",
                summary: null,
              },
            }),
          ],
        }),
        () => Promise.resolve(),
      ),
    );

    expect(
      screen.getByRole("button", { name: "Release pin" }).textContent,
    ).toBe("pinned ×");
  });

  it("follows the file through a reload, in both directions", () => {
    const view = (pinned: boolean) =>
      boardView({ placements: [placedBox("kept", { pinned })] });
    const { container, rerender } = render(
      canvasOf(view(false), () => Promise.resolve()),
    );

    rerender(canvasOf(view(true), () => Promise.resolve()));
    expect(container.querySelector(".element-pin")).not.toBeNull();

    rerender(canvasOf(view(false), () => Promise.resolve()));
    expect(container.querySelector(".element-pin")).toBeNull();
  });
});

describe("deleting a note", () => {
  const withANote = () =>
    boardView({
      placements: [
        placedBox("plain"),
        placedBox("ask", {
          x: 300,
          shown: { kind: "note", name: "Ask", body: "" },
        }),
      ],
    });

  /** A canvas of `withANote` whose `deleteNotes` is `remove`, with `element` selected. */
  async function selected(
    element: string,
    remove: (elements: string[]) => void,
  ) {
    const rendered = render(
      canvasOf(
        withANote(),
        () => Promise.resolve(),
        () => {},
        () => {},
        () => Promise.resolve(true),
        () => Promise.resolve("new-note"),
        remove,
      ),
    );
    const node = box(rendered.container, element);
    fireEvent.keyDown(node, { key: "Enter" });
    // React Flow matches its delete key only while no other key is held.
    fireEvent.keyUp(node, { key: "Enter" });
    await waitFor(() => expect(node.classList.contains("selected")).toBe(true));
    return { ...rendered, node };
  }

  it.each(["Delete", "Backspace"])(
    "hands a selected note to the delete with %s, and keeps it on the screen",
    async (key) => {
      const remove = vi.fn();
      const { container, node } = await selected("ask", remove);

      fireEvent.keyDown(node, { key });

      await waitFor(() =>
        expect(remove).toHaveBeenCalledExactlyOnceWith(["ask"]),
      );
      expect(box(container, "ask").isConnected).toBe(true);
    },
  );

  it("does not delete a selected box", async () => {
    const remove = vi.fn();
    const { container, node } = await selected("plain", remove);

    fireEvent.keyDown(node, { key: "Delete" });
    await act(() => Promise.resolve());

    expect(remove).not.toHaveBeenCalled();
    expect(box(container, "plain").isConnected).toBe(true);
  });

  it("deletes nothing with the keys pressed in a note's text field", async () => {
    const remove = vi.fn();
    const { node } = await selected("ask", remove);
    const text = node.querySelector(".element-note");
    if (text === null) {
      throw new Error("no note text");
    }
    fireEvent.doubleClick(text);
    const field = screen.getByRole("textbox", { name: "Text of the note" });

    fireEvent.keyDown(field, { key: "Delete" });
    fireEvent.keyDown(field, { key: "Backspace" });
    await act(() => Promise.resolve());

    expect(remove).not.toHaveBeenCalled();
  });
});

describe("creating a note", () => {
  const plainBoard = () =>
    boardView({ placements: [placedElement("plain", { x: 300, y: 200 })] });
  const notes = (container: HTMLElement) =>
    [...container.querySelectorAll(".react-flow__node")]
      .map((node) => node.getAttribute("data-id"))
      .filter((id) => id?.startsWith("+") === true);
  const fieldOf = () => {
    const field = screen.getByRole("textbox", { name: "Text of the note" });
    if (!(field instanceof HTMLTextAreaElement)) {
      throw new Error("no text area");
    }
    return field;
  };
  const noop = () => {};

  /** A canvas whose `createNote` is `create`. */
  function canvasWith(
    create: (position: XYPosition, parts: NoteParts) => Promise<string | null>,
    view = plainBoard(),
    askedAt: number | null = null,
  ) {
    return canvasOf(
      view,
      () => Promise.resolve(),
      noop,
      noop,
      () => Promise.resolve(true),
      create,
      noop,
      askedAt,
    );
  }

  /** The element that holds the canvas and follows the mouse. */
  const surfaceOf = (container: HTMLElement) => {
    const surface = container.querySelector(".board-surface");
    if (surface === null) {
      throw new Error("no surface");
    }
    return surface;
  };
  const paneOf = (container: HTMLElement) => {
    const pane = container.querySelector(".react-flow__pane");
    if (pane === null) {
      throw new Error("no pane");
    }
    return pane;
  };

  it("starts with the key N at the mouse, with the field open and empty", () => {
    const create = vi.fn(() => Promise.resolve("new-note"));
    const { container } = render(canvasWith(create));
    fireEvent.mouseMove(surfaceOf(container), { clientX: 120, clientY: 60 });

    const notPrevented = fireEvent.keyDown(window, { key: "n" });

    expect(notPrevented).toBe(false);
    expect(notes(container)).toEqual(["+note-1"]);
    expect(fieldOf().value).toBe("");
    expect(document.activeElement).toBe(fieldOf());
    expect(create).not.toHaveBeenCalled();

    fireEvent.change(fieldOf(), { target: { value: "Ask" } });
    fireEvent.keyDown(fieldOf(), { key: "Escape" });
    expect(create).toHaveBeenCalledExactlyOnceWith(
      { x: 120, y: 60 },
      { name: "Ask", body: "" },
    );
  });

  /** Writes the open note, and returns where it was asked to go. */
  function writtenAt(
    create: ReturnType<
      typeof vi.fn<
        (position: XYPosition, parts: NoteParts) => Promise<string | null>
      >
    >,
  ) {
    fireEvent.change(fieldOf(), { target: { value: "Ask" } });
    fireEvent.keyDown(fieldOf(), { key: "Escape" });
    return create.mock.calls[0]?.[0];
  }

  /** A canvas whose surface lies at 10, 20 and is 400 by 200, with its middle at 210, 120. */
  function placedCanvas() {
    const create = vi.fn<
      (position: XYPosition, parts: NoteParts) => Promise<string | null>
    >(() => Promise.resolve("new-note"));
    const { container } = render(canvasWith(create));
    vi.spyOn(surfaceOf(container), "getBoundingClientRect").mockReturnValue({
      left: 10,
      top: 20,
      width: 400,
      height: 200,
    } as DOMRect);
    return { container, create };
  }

  it("starts in the middle of the view with N when the mouse was never there", () => {
    const { create } = placedCanvas();

    fireEvent.keyDown(window, { key: "n" });

    expect(writtenAt(create)).toEqual({ x: 210, y: 120 });
  });

  it("takes a capital N, as with Caps Lock, like a small one", () => {
    const { create } = placedCanvas();

    fireEvent.keyDown(window, { key: "N" });

    expect(writtenAt(create)).toEqual({ x: 210, y: 120 });
  });

  it("starts in the middle of the view with N after the mouse has left", () => {
    const { container, create } = placedCanvas();
    fireEvent.mouseMove(surfaceOf(container), { clientX: 120, clientY: 60 });
    fireEvent.mouseLeave(surfaceOf(container));

    fireEvent.keyDown(window, { key: "n" });

    expect(writtenAt(create)).toEqual({ x: 210, y: 120 });
  });

  it("starts in the middle of the view with the toolbar, wherever the mouse is", () => {
    const { container, create } = placedCanvas();
    fireEvent.mouseMove(surfaceOf(container), { clientX: 120, clientY: 60 });

    fireEvent.click(screen.getByRole("button", { name: "New note" }));

    expect(writtenAt(create)).toEqual({ x: 210, y: 120 });
  });

  it("takes the mouse on a new note, so that its text field can be clicked", () => {
    const { container } = render(canvasWith(() => Promise.resolve("new-note")));

    fireEvent.keyDown(window, { key: "n" });

    expect(box(container, "+note-1").style.pointerEvents).not.toBe("none");
  });

  it("leaves one note when a view lists it before the write's answer arrives", async () => {
    let answer: (id: string) => void = () => {};
    const create = () =>
      new Promise<string | null>((resolve) => {
        answer = resolve;
      });
    const { container, rerender } = render(canvasWith(create));
    fireEvent.keyDown(window, { key: "n" });
    fireEvent.change(fieldOf(), { target: { value: "Ask" } });
    fireEvent.keyDown(fieldOf(), { key: "Escape" });

    rerender(
      canvasWith(
        create,
        boardView({
          placements: [
            placedBox("new-note", {
              shown: { kind: "note", name: "Ask", body: "" },
            }),
          ],
        }),
      ),
    );
    await act(async () => {
      answer("new-note");
      await Promise.resolve();
    });

    expect(
      [...container.querySelectorAll(".react-flow__node")].map((node) =>
        node.getAttribute("data-id"),
      ),
    ).toEqual(["new-note"]);
  });

  it("keeps a written note through a view asked for before the answer, and drops it with one asked for after", async () => {
    const create = () => Promise.resolve<string | null>("new-note");
    const before = nextStamp();
    const { container, rerender } = render(canvasWith(create));
    fireEvent.keyDown(window, { key: "n" });
    fireEvent.change(fieldOf(), { target: { value: "Ask" } });
    await act(async () => {
      fireEvent.keyDown(fieldOf(), { key: "Escape" });
      await Promise.resolve();
    });
    const ids = () =>
      [...container.querySelectorAll(".react-flow__node")].map((node) =>
        node.getAttribute("data-id"),
      );

    rerender(canvasWith(create, plainBoard(), before));
    expect(ids()).toEqual(["plain", "+note-1"]);

    rerender(canvasWith(create, plainBoard(), nextStamp()));
    expect(ids()).toEqual(["plain"]);
  });

  it.each([
    ["a repeat", { key: "n", repeat: true }],
    ["Ctrl", { key: "n", ctrlKey: true }],
    ["Meta", { key: "n", metaKey: true }],
    ["Alt", { key: "n", altKey: true }],
    ["Shift", { key: "N", shiftKey: true }],
    ["another key", { key: "m" }],
  ])("does not start with %s", (_name, key) => {
    const { container } = render(canvasWith(() => Promise.resolve("new-note")));

    fireEvent.keyDown(window, key);

    expect(notes(container)).toEqual([]);
  });

  it("does not start with N in a text field", () => {
    const { container } = render(canvasWith(() => Promise.resolve("new-note")));
    fireEvent.keyDown(window, { key: "n" });

    fireEvent.keyDown(fieldOf(), { key: "n" });

    expect(notes(container)).toEqual(["+note-1"]);
  });

  it("stops listening for the key when it is gone", () => {
    const { container, unmount } = render(
      canvasWith(() => Promise.resolve("new-note")),
    );
    expect(notes(container)).toEqual([]);

    unmount();

    expect(fireEvent.keyDown(window, { key: "n" })).toBe(true);
  });

  it("starts with a double-click on an empty spot, at that spot", () => {
    const create = vi.fn(() => Promise.resolve("new-note"));
    const { container } = render(canvasWith(create));

    fireEvent.doubleClick(paneOf(container), { clientX: 120, clientY: 60 });

    expect(notes(container)).toEqual(["+note-1"]);
    fireEvent.change(fieldOf(), { target: { value: "Ask" } });
    fireEvent.keyDown(fieldOf(), { key: "Escape" });
    expect(create).toHaveBeenCalledExactlyOnceWith(
      { x: 120, y: 60 },
      { name: "Ask", body: "" },
    );
  });

  it("does not start with a double-click on a box", () => {
    const onOpen = vi.fn();
    const { container } = render(
      canvasOf(
        boardView({
          placements: [
            placedBox("shop", {
              inner_boards: [{ id: "inner", title: "Inner" }],
            }),
          ],
        }),
        () => Promise.resolve(),
        onOpen,
      ),
    );

    fireEvent.doubleClick(box(container, "shop"));

    expect(notes(container)).toEqual([]);
    expect(onOpen).toHaveBeenCalledExactlyOnceWith("inner");
  });

  it("starts on a mirrored board too", () => {
    const { container } = render(
      canvasWith(
        () => Promise.resolve("new-note"),
        boardView({ mode: "mirrored", placements: [placedBox("plain")] }),
      ),
    );

    fireEvent.keyDown(window, { key: "n" });

    expect(notes(container)).toEqual(["+note-1"]);
  });

  it("writes nothing for blank text, and the note goes", () => {
    const create = vi.fn(() => Promise.resolve("new-note"));
    const { container } = render(canvasWith(create));
    fireEvent.keyDown(window, { key: "n" });

    fireEvent.change(fieldOf(), { target: { value: " \n " } });
    fireEvent.keyDown(fieldOf(), { key: "Escape" });

    expect(create).not.toHaveBeenCalled();
    expect(notes(container)).toEqual([]);
  });

  it("writes once, shows the text, and opens no field again until a view lists it", async () => {
    const create = vi.fn(() => Promise.resolve("new-note"));
    const { container, rerender } = render(canvasWith(create));
    fireEvent.keyDown(window, { key: "n" });
    fireEvent.change(fieldOf(), { target: { value: "Ask" } });

    fireEvent.keyDown(fieldOf(), { key: "Escape" });
    fireEvent.blur(fieldOf());
    await waitFor(() => expect(screen.queryByRole("textbox")).toBeNull());
    expect(create).toHaveBeenCalledOnce();
    const shown = () =>
      box(container, "+note-1").querySelector(".element-note");
    expect(shown()?.textContent).toBe("Ask");

    const waiting = shown();
    if (waiting === null || waiting === undefined) {
      throw new Error("no note");
    }
    fireEvent.doubleClick(waiting);
    expect(screen.queryByRole("textbox")).toBeNull();
    expect(create).toHaveBeenCalledOnce();

    // A reload that came before the write does not list it yet.
    rerender(canvasWith(create, plainBoard()));
    expect(notes(container)).toEqual(["+note-1"]);

    rerender(
      canvasWith(
        create,
        boardView({
          placements: [
            placedBox("plain", { x: 300, y: 200 }),
            placedBox("new-note", {
              shown: { kind: "note", name: "Ask", body: "" },
            }),
          ],
        }),
      ),
    );
    expect(notes(container)).toEqual([]);
    expect(box(container, "new-note").textContent).toContain("Ask");
  });

  it("keeps the field and its text when the write fails", async () => {
    const create = vi.fn<
      (position: XYPosition, parts: NoteParts) => Promise<string | null>
    >(() => Promise.resolve(null));
    const { container } = render(canvasWith(create));
    fireEvent.keyDown(window, { key: "n" });
    fireEvent.change(fieldOf(), { target: { value: "Ask" } });

    fireEvent.keyDown(fieldOf(), { key: "Escape" });
    await waitFor(() => expect(create).toHaveBeenCalledOnce());
    await act(() => Promise.resolve());

    expect(fieldOf().value).toBe("Ask");
    expect(notes(container)).toEqual(["+note-1"]);
  });

  it("shows markup in a written note as text", async () => {
    const { container } = render(canvasWith(() => Promise.resolve("new-note")));
    fireEvent.keyDown(window, { key: "n" });
    fireEvent.change(fieldOf(), {
      target: { value: "<img src=x onerror=alert(1)>" },
    });

    fireEvent.keyDown(fieldOf(), { key: "Escape" });

    await waitFor(() => expect(screen.queryByRole("textbox")).toBeNull());
    expect(container.querySelector("img")).toBeNull();
    expect(box(container, "+note-1").textContent).toContain(
      "<img src=x onerror=alert(1)>",
    );
  });

  it("leaves a new note out of a move of the selected boxes", async () => {
    const move = vi.fn<(moves: Move[]) => Promise<void>>(() =>
      Promise.resolve(),
    );
    const { container } = render(canvasOf(plainBoard(), move));
    fireEvent.keyDown(window, { key: "n" });
    const plain = box(container, "plain");

    fireEvent.keyDown(plain, { key: "Enter" });
    await waitFor(() =>
      expect(plain.classList.contains("selected")).toBe(true),
    );
    fireEvent.keyDown(plain, { key: "ArrowRight" });

    await waitFor(() => expect(move).toHaveBeenCalled());
    for (const [moves] of move.mock.calls) {
      for (const { element } of moves) {
        expect(element.startsWith("+")).toBe(false);
      }
    }
  });
});

describe("a note on the canvas", () => {
  describe("its text field", () => {
    const noteView = (name = "Ask", body = "Why") =>
      boardView({
        placements: [
          placedBox("ask", {
            shown: { kind: "note", name, body },
            inner_boards: [{ id: "inner", title: "Inner" }],
          }),
        ],
      });
    const ask = (container: HTMLElement) =>
      box(container, "ask").querySelector(".element-note");
    const fieldOf = () => {
      const field = screen.getByRole("textbox", { name: "Text of the note" });
      if (!(field instanceof HTMLTextAreaElement)) {
        throw new Error("no text area");
      }
      return field;
    };

    /** Opens the text field of `ask` on a canvas with a fake write. */
    function opened(
      writeNote: (element: string, parts: NoteParts) => Promise<boolean>,
      view = noteView(),
    ) {
      const move = vi.fn(() => Promise.resolve());
      const onOpen = vi.fn();
      const rendered = render(
        canvasOf(view, move, onOpen, () => {}, writeNote),
      );
      const note = ask(rendered.container);
      if (note === null) {
        throw new Error("no note");
      }
      fireEvent.doubleClick(note);
      return { ...rendered, move, onOpen, field: fieldOf() };
    }

    afterEach(() => {
      vi.restoreAllMocks();
    });

    it("opens with the text on a double-click, focused, and opens no inner board", () => {
      const writeNote = vi.fn(() => Promise.resolve(true));
      const { field, onOpen } = opened(writeNote);

      expect(field.value).toBe("Ask\nWhy");
      expect(document.activeElement).toBe(field);
      expect(onOpen).not.toHaveBeenCalled();
      expect(writeNote).not.toHaveBeenCalled();
    });

    it("is as high as its text when it opens and after every input, wrapped lines included", () => {
      let measured = 120;
      vi.spyOn(
        HTMLTextAreaElement.prototype,
        "scrollHeight",
        "get",
      ).mockImplementation(() => measured);
      const { field } = opened(vi.fn(() => Promise.resolve(true)));

      expect(field.style.height).toBe("120px");
      expect(field.rows).toBe(3);
      expect(field.value).toBe("Ask\nWhy");

      measured = 260;
      fireEvent.input(field, {
        target: { value: `Ask\n${"long ".repeat(80)}` },
      });

      expect(field.style.height).toBe("260px");
      expect(field.rows).toBe(3);
    });

    it("leaves the button of the note's inner board working", () => {
      const onOpen = vi.fn();
      render(canvasOf(noteView(), () => Promise.resolve(), onOpen));

      fireEvent.click(screen.getByRole("button", { name: "▸ Inner" }));

      expect(onOpen).toHaveBeenCalledExactlyOnceWith("inner");
    });

    it.each([
      [
        "Esc",
        (field: HTMLTextAreaElement) =>
          fireEvent.keyDown(field, { key: "Escape" }),
      ],
      [
        "a click next to it",
        (field: HTMLTextAreaElement) => fireEvent.blur(field),
      ],
    ])(
      "writes the changed text once on %s, also if a blur follows",
      async (_how, end) => {
        const writeNote = vi.fn(() => Promise.resolve(true));
        const { field, container } = opened(writeNote);

        fireEvent.change(field, { target: { value: "Asked\nWhen\n" } });
        end(field);
        fireEvent.blur(field);

        expect(writeNote).toHaveBeenCalledExactlyOnceWith("ask", {
          name: "Asked",
          body: "When",
        });
        await waitFor(() =>
          expect(ask(container)?.textContent).toBe("Asked\nWhen"),
        );
        expect(screen.queryByRole("textbox")).toBeNull();
      },
    );

    it("does not end when the window loses focus", () => {
      vi.spyOn(document, "hasFocus").mockReturnValue(false);
      const writeNote = vi.fn(() => Promise.resolve(true));
      const { field } = opened(writeNote);

      fireEvent.change(field, { target: { value: "Asked" } });
      fireEvent.blur(field);

      expect(writeNote).not.toHaveBeenCalled();
      expect(fieldOf().value).toBe("Asked");
    });

    it.each(["Ask\nWhy", "Ask\nWhy\n", "\nAsk\nWhy  "])(
      "writes nothing for %j, which is unchanged",
      (value) => {
        const writeNote = vi.fn(() => Promise.resolve(true));
        const { field, container } = opened(writeNote);

        fireEvent.change(field, { target: { value } });
        fireEvent.keyDown(field, { key: "Escape" });

        expect(writeNote).not.toHaveBeenCalled();
        expect(ask(container)?.textContent).toBe("Ask\nWhy");
      },
    );

    it("writes nothing for blank text, and the old text returns", () => {
      const writeNote = vi.fn(() => Promise.resolve(true));
      const { field, container } = opened(writeNote);

      fireEvent.change(field, { target: { value: " \n " } });
      fireEvent.keyDown(field, { key: "Escape" });

      expect(writeNote).not.toHaveBeenCalled();
      expect(ask(container)?.textContent).toBe("Ask\nWhy");
    });

    it("keeps what is typed through a reload", () => {
      const writeNote = () => Promise.resolve(true);
      const { field, rerender } = opened(writeNote);
      fireEvent.change(field, { target: { value: "Typing" } });

      rerender(
        canvasOf(
          noteView("From the AI", "Other"),
          () => Promise.resolve(),
          () => {},
          () => {},
          writeNote,
        ),
      );

      expect(fieldOf().value).toBe("Typing");
    });

    it("has its field closed when it comes back after its file went missing", () => {
      const writeNote = () => Promise.resolve(true);
      const { rerender } = opened(writeNote);
      const reload = (view: BoardView) => {
        rerender(
          canvasOf(
            view,
            () => Promise.resolve(),
            () => {},
            () => {},
            writeNote,
          ),
        );
      };

      reload(
        boardView({
          placements: [placedBox("ask", { shown: { kind: "missing" } })],
        }),
      );
      reload(noteView());

      expect(
        screen.queryByRole("textbox", { name: "Text of the note" }),
      ).toBeNull();
    });

    it("writes nothing when a reload comes before any typing, and keeps its text", () => {
      const writeNote = vi.fn(() => Promise.resolve(true));
      const { rerender } = opened(writeNote);

      rerender(
        canvasOf(
          noteView("From the AI", "Other"),
          () => Promise.resolve(),
          () => {},
          () => {},
          writeNote,
        ),
      );
      expect(fieldOf().value).toBe("Ask\nWhy");
      expect(fieldOf().rows).toBe(3);

      fireEvent.keyDown(fieldOf(), { key: "Escape" });
      expect(writeNote).not.toHaveBeenCalled();
    });

    it("stays open with the text when the write fails, and can write again", async () => {
      const writeNote = vi
        .fn<(element: string, parts: NoteParts) => Promise<boolean>>()
        .mockResolvedValueOnce(false)
        .mockResolvedValueOnce(true);
      const { field, container } = opened(writeNote);
      fireEvent.change(field, { target: { value: "Asked" } });

      fireEvent.keyDown(field, { key: "Escape" });
      await waitFor(() => expect(writeNote).toHaveBeenCalledTimes(1));
      await act(() => Promise.resolve());
      expect(fieldOf().value).toBe("Asked");

      fireEvent.keyDown(fieldOf(), { key: "Escape" });
      await waitFor(() => expect(ask(container)?.textContent).toBe("Asked"));
      expect(writeNote).toHaveBeenCalledTimes(2);
    });

    it("takes no typing while its write is on the way", async () => {
      let finish: (written: boolean) => void = () => {};
      const { field } = opened(
        () =>
          new Promise<boolean>((resolve) => {
            finish = resolve;
          }),
      );
      fireEvent.change(field, { target: { value: "Asked" } });

      fireEvent.keyDown(field, { key: "Escape" });
      expect(fieldOf().readOnly).toBe(true);

      await act(async () => {
        finish(false);
        await Promise.resolve();
      });
      expect(fieldOf().readOnly).toBe(false);
    });

    it("moves no box with keys pressed in it", () => {
      const { field, move } = opened(() => Promise.resolve(true));

      fireEvent.keyDown(field, { key: "ArrowRight" });

      expect(move).not.toHaveBeenCalled();
    });
  });

  const onAMirroredBoard = (askIsANote: boolean) =>
    boardView({
      mode: "mirrored",
      placements: [
        placedElement("plain"),
        placedElement("ask", {
          x: 200,
          pinned: true,
          ...(askIsANote
            ? { shown: { kind: "note" as const, name: "Ask", body: "" } }
            : {}),
        }),
      ],
    });

  it("alone can be dragged on a mirrored board, and offers no release there", () => {
    const { container } = render(
      canvasOf(onAMirroredBoard(true), () => Promise.resolve()),
    );

    expect(
      [...container.querySelectorAll(DRAGGABLE_BOX)].map((box) =>
        box.getAttribute("data-id"),
      ),
    ).toEqual(["ask"]);
    expect(screen.queryByRole("button", { name: "Release pin" })).toBeNull();
  });

  it("loses the drag on a mirrored board when a reload says it is no note", () => {
    const move = () => Promise.resolve();
    const { container, rerender } = render(
      canvasOf(onAMirroredBoard(true), move),
    );

    rerender(canvasOf(onAMirroredBoard(false), move));

    expect(container.querySelectorAll(DRAGGABLE_BOX)).toHaveLength(0);
    expect(screen.queryByRole("button", { name: "Release pin" })).toBeNull();
  });

  it("shows its text in place of a box's lines, and no pin marker", () => {
    const { container } = render(
      canvasOf(
        boardView({
          placements: [
            placedBox("ask", {
              pinned: true,
              shown: { kind: "note", name: "Ask why", body: "It retries." },
            }),
          ],
        }),
        () => Promise.resolve(),
      ),
    );

    const note = box(container, "ask");
    expect(note.querySelector(".element-note")?.textContent).toBe(
      "Ask why\nIt retries.",
    );
    expect(note.querySelector(".element-type")).toBeNull();
    expect(note.querySelector(".element-pin")).toBeNull();
    expect(screen.queryByRole("button", { name: "Release pin" })).toBeNull();
  });
});

describe("inner boards on a box", () => {
  it("offers a button per inner board that opens it, and writes nothing", () => {
    const move = vi.fn(() => Promise.resolve());
    const onOpen = vi.fn();
    render(canvasOf(boardWithInnerBoards("Containers", "Spare"), move, onOpen));

    fireEvent.click(screen.getByRole("button", { name: "▸ Spare" }));

    expect(onOpen).toHaveBeenCalledExactlyOnceWith("inner-1");
    expect(move).not.toHaveBeenCalled();
  });

  it("opens a board with its button and with a double click on a mirrored board too", () => {
    const move = vi.fn(() => Promise.resolve());
    const onOpen = vi.fn();
    const { container } = render(
      canvasOf(
        { ...boardWithInnerBoards("Containers", "Spare"), mode: "mirrored" },
        move,
        onOpen,
      ),
    );

    fireEvent.click(screen.getByRole("button", { name: "▸ Spare" }));
    fireEvent.doubleClick(box(container, "shop"));

    expect(onOpen.mock.calls).toEqual([["inner-1"], ["inner-0"]]);
    expect(move).not.toHaveBeenCalled();
  });

  it("shows no button on a box without an inner board", () => {
    const { container } = render(
      canvasOf(boardWithInnerBoards("Containers"), () => Promise.resolve()),
    );

    expect(box(container, "plain").querySelector("button")).toBeNull();
    expect(box(container, "shop").querySelector("button")).not.toBeNull();
  });

  it("opens the first inner board on a double click, and writes nothing", () => {
    const move = vi.fn(() => Promise.resolve());
    const onOpen = vi.fn();
    const { container } = render(
      canvasOf(boardWithInnerBoards("Containers", "Spare"), move, onOpen),
    );

    fireEvent.doubleClick(box(container, "shop"));

    expect(onOpen).toHaveBeenCalledExactlyOnceWith("inner-0");
    expect(move).not.toHaveBeenCalled();
  });

  it("does nothing on a double click on a box without an inner board", () => {
    const move = vi.fn(() => Promise.resolve());
    const onOpen = vi.fn();
    const { container } = render(
      canvasOf(boardWithInnerBoards("Containers"), move, onOpen),
    );

    fireEvent.doubleClick(box(container, "plain"));

    expect(onOpen).not.toHaveBeenCalled();
    expect(move).not.toHaveBeenCalled();
  });

  it("does not open a board twice on a double click on its button", () => {
    const onOpen = vi.fn();
    render(
      canvasOf(
        boardWithInnerBoards("Containers", "Spare"),
        () => Promise.resolve(),
        onOpen,
      ),
    );

    fireEvent.doubleClick(screen.getByRole("button", { name: "▸ Spare" }));

    expect(onOpen).not.toHaveBeenCalled();
  });

  it("moves no box with the arrow keys from a focused button, and a click opens it", () => {
    const move = vi.fn(() => Promise.resolve());
    const onOpen = vi.fn();
    const { container } = render(
      canvasOf(boardWithInnerBoards("Containers"), move, onOpen),
    );
    const shop = box(container, "shop");
    fireEvent.keyDown(shop, { key: "Enter" });
    const button = screen.getByRole("button", { name: "▸ Containers" });
    button.focus();

    fireEvent.keyDown(button, { key: "ArrowRight" });
    fireEvent.keyDown(button, { key: "ArrowDown" });

    expect(move).not.toHaveBeenCalled();

    fireEvent.click(button);

    expect(onOpen).toHaveBeenCalledExactlyOnceWith("inner-0");
  });

  it("shows a title as text and makes nothing of markup in it", () => {
    const title = "<img src=x onerror=alert(1)>";
    const { container } = render(
      canvasOf(boardWithInnerBoards(title), () => Promise.resolve()),
    );

    expect(screen.getByRole("button", { name: `▸ ${title}` })).toBeDefined();
    const shop = box(container, "shop");
    expect(shop.querySelector("img, script, a")).toBeNull();
  });
});

/** Reports the observed boxes when told, as a browser does after layout. */
class MeasuringObserver {
  static current: MeasuringObserver[] = [];
  private readonly targets = new Set<Element>();

  constructor(private readonly report: ResizeObserverCallback) {
    MeasuringObserver.current.push(this);
  }

  observe(target: Element): void {
    this.targets.add(target);
  }

  unobserve(target: Element): void {
    this.targets.delete(target);
  }

  disconnect(): void {
    this.targets.clear();
  }

  measure(): void {
    const entries = [...this.targets].map(
      (target) =>
        ({
          target,
          contentRect: target.getBoundingClientRect(),
        }) as unknown as ResizeObserverEntry,
    );
    this.report(entries, this as unknown as ResizeObserver);
  }
}

/** The rectangle a browser would report for `element`, in a box of 180 x 90. */
function layoutOf(element: Element): DOMRect {
  const rect = (x: number, y: number, width: number, height: number) =>
    new DOMRect(x, y, width, height);
  if (element.classList.contains("react-flow__handle-top")) {
    return rect(90, 0, 0, 0);
  }
  if (element.classList.contains("react-flow__handle-bottom")) {
    return rect(90, 90, 0, 0);
  }
  return element.classList.contains("react-flow__node")
    ? rect(0, 0, 180, 90)
    : rect(0, 0, 800, 600);
}

/** A mouse event as a browser sends it, which d3-drag reads the window of. */
function mouse(type: string, clientX: number, clientY: number): MouseEvent {
  const event = new MouseEvent(type, {
    bubbles: true,
    cancelable: true,
    clientX,
    clientY,
  });
  Object.defineProperty(event, "view", { value: window });
  return event;
}

describe("arrows in a laid-out canvas", () => {
  beforeEach(() => {
    MeasuringObserver.current = [];
    vi.stubGlobal("ResizeObserver", MeasuringObserver);
    vi.stubGlobal(
      "DOMMatrixReadOnly",
      class {
        m22 = 1;
      },
    );
    vi.spyOn(HTMLElement.prototype, "offsetWidth", "get").mockReturnValue(180);
    vi.spyOn(HTMLElement.prototype, "offsetHeight", "get").mockReturnValue(90);
    vi.spyOn(Element.prototype, "getBoundingClientRect").mockImplementation(
      function (this: Element) {
        return layoutOf(this);
      },
    );
  });

  afterEach(() => {
    vi.unstubAllGlobals();
    vi.restoreAllMocks();
  });

  const measure = () =>
    act(() => {
      for (const observer of MeasuringObserver.current) {
        observer.measure();
      }
    });

  it("keeps an arrow through measuring, a drag, and a reload", async () => {
    const move = vi.fn(() => Promise.resolve());
    const { container, rerender } = render(
      canvasOf(boardWithArrow(null), move),
    );
    const path = () =>
      container.querySelector(".react-flow__edge-path")?.getAttribute("d");
    measure();
    expect(path()).toBeTruthy();
    measure();
    const before = path();
    expect(before).toBeTruthy();

    const box = container.querySelector('.react-flow__node[data-id="orders"]');
    if (box === null) {
      throw new Error("no box for orders");
    }
    act(() => {
      box.dispatchEvent(mouse("mousedown", 10, 10));
    });
    act(() => {
      window.dispatchEvent(mouse("mousemove", 100, 10));
    });
    act(() => {
      window.dispatchEvent(mouse("mousemove", 250, 15));
    });
    await waitFor(() => {
      expect(path()).toBeTruthy();
      expect(path()).not.toBe(before);
    });
    const dragged = path();

    await act(async () => {
      window.dispatchEvent(mouse("mouseup", 250, 15));
      await Promise.resolve();
    });
    expect(move).toHaveBeenCalledOnce();
    expect(path()).toBe(dragged);

    rerender(canvasOf(boardWithArrow(null, 240), move));
    expect(path()).toBeTruthy();
    measure();
    expect(path()).toBeTruthy();
  });

  it("releases a pin where a drag left the box, not where the file still has it", async () => {
    const move = vi.fn<(moves: Move[]) => Promise<void>>(() =>
      Promise.resolve(),
    );
    const release = vi.fn();
    const { container } = render(
      canvasOf(
        boardView({ placements: [placedElement("kept", { pinned: true })] }),
        move,
        () => {},
        release,
      ),
    );
    measure();
    measure();
    const dragged = box(container, "kept");

    act(() => {
      dragged.dispatchEvent(mouse("mousedown", 10, 10));
    });
    act(() => {
      window.dispatchEvent(mouse("mousemove", 100, 10));
    });
    act(() => {
      window.dispatchEvent(mouse("mousemove", 250, 15));
    });
    await act(async () => {
      window.dispatchEvent(mouse("mouseup", 250, 15));
      await Promise.resolve();
    });
    fireEvent.click(screen.getByRole("button", { name: "Release pin" }));

    const position = move.mock.calls[0]?.[0][0]?.position;
    expect(position).toBeDefined();
    expect(position).not.toEqual({ x: 0, y: 0 });
    expect(release).toHaveBeenCalledExactlyOnceWith("kept", position);
  });
});

describe("moving a note with the arrow keys", () => {
  it("takes it off its box when its centre leaves the box, as a drag does", () => {
    const view = boardView({
      placements: [
        placedBox("b", {
          shown: { kind: "element", type_ref: "t", name: "B", summary: null },
        }),
        // The note's centre lies on the right edge of `b`.
        placedBox("n", {
          x: 90,
          y: 0,
          shown: { kind: "note", name: "Ask", body: "" },
          attached_to: "b",
        }),
      ],
    });
    const move = vi.fn<(moves: Move[]) => Promise<void>>(() =>
      Promise.resolve(),
    );
    const { container } = render(canvasOf(view, move));
    const note = container.querySelector('.react-flow__node[data-id="n"]');
    if (note === null) {
      throw new Error("no note");
    }

    fireEvent.keyDown(note, { key: "Enter" });
    fireEvent.keyDown(note, { key: "ArrowRight" });

    expect(move.mock.calls).toEqual([
      [[{ element: "n", position: { x: 95, y: 0 }, hang: "detach" }]],
    ]);
  });
});

describe("a box whose file is missing", () => {
  it("cannot be moved with the arrow keys on any board", () => {
    for (const mode of [null, "free", "mirrored"] as const) {
      const move = vi.fn(() => Promise.resolve());
      const { container, unmount } = render(
        canvasOf(boardView({ mode, placements: [placedBox("gone")] }), move),
      );
      const gone = box(container, "gone");

      fireEvent.keyDown(gone, { key: "Enter" });
      fireEvent.keyDown(gone, { key: "ArrowRight" });

      expect(move).not.toHaveBeenCalled();
      expect(gone.classList.contains("draggable")).toBe(false);
      unmount();
    }
  });
});

describe("the marks on a box's top edge", () => {
  it("are both cut to half of the edge, and the release keeps its name", () => {
    const { container } = render(
      <ReviewMarks.Provider
        value={
          new Map([
            ["kept", "task"],
            ["gone", "hint"],
          ])
        }
      >
        {canvasOf(
          boardView({
            placements: [
              placedElement("kept", { width: 100, pinned: true }),
              placedBox("gone", { x: 200, width: 100, pinned: true }),
            ],
          }),
          () => Promise.resolve(),
        )}
      </ReviewMarks.Provider>,
    );

    for (const element of ["kept", "gone"]) {
      const marks = box(container, element).querySelectorAll(
        ".element-pin, .element-flag",
      );
      expect(marks).toHaveLength(2);
      for (const mark of marks) {
        expect(mark.classList.contains("element-mark")).toBe(true);
      }
    }
    expect(
      screen.getByRole("button", { name: "Release pin" }).classList,
    ).toContain("element-mark");
  });
});

describe("the zoom bar", () => {
  let undoMeasuring: () => void = () => {};

  beforeEach(() => {
    undoMeasuring = measureBoxes();
  });

  afterEach(() => {
    // Unmounted first, so that no late size report meets jsdom's own globals.
    cleanup();
    undoMeasuring();
  });

  /** The zoom the canvas draws with, from the transform of its viewport. */
  function drawnZoom(container: HTMLElement): number {
    const viewport = container.querySelector<HTMLElement>(
      ".react-flow__viewport",
    );
    const scale = /scale\(([^)]+)\)/.exec(viewport?.style.transform ?? "");
    return Number(scale?.[1]);
  }

  /** The percentage the bar shows. */
  function shownPercent(): string {
    return screen.getByTitle("Zoom to 100 %").textContent ?? "";
  }

  /**
   * A canvas of `view`, by default two small boxes, once React Flow has
   * measured its boxes and fitted it into view on opening, which moves the
   * zoom away from 100 %; React Flow's pane is 500 × 500 in jsdom.
   */
  async function zoomCanvas(view: BoardView = boardIn("free")) {
    const rendered = render(canvasOf(view, () => Promise.resolve()));
    await waitFor(() => {
      expect(drawnZoom(rendered.container)).not.toBe(1);
    });
    return rendered;
  }

  /** A board many times wider than the pane, which fits only below 10 %. */
  function wideBoard(): BoardView {
    return boardView({
      placements: [
        placedElement("orders"),
        placedElement("payments", { x: 100_000 }),
      ],
    });
  }

  it("shows the zoom the canvas draws with as a whole percentage", async () => {
    const { container } = await zoomCanvas();

    await waitFor(() => {
      expect(shownPercent()).toBe(
        `${Math.round(drawnZoom(container) * 100)} %`,
      );
    });
    expect(shownPercent()).toMatch(/^\d+ %$/);
  });

  it("zooms to 100 % on a click on the percentage", async () => {
    const { container } = await zoomCanvas();

    fireEvent.click(screen.getByTitle("Zoom to 100 %"));

    await waitFor(() => {
      expect(drawnZoom(container)).toBe(1);
    });
    expect(shownPercent()).toBe("100 %");
  });

  it("zooms in with + and out with −, and the percentage follows", async () => {
    const { container } = await zoomCanvas();
    fireEvent.click(screen.getByTitle("Zoom to 100 %"));
    await waitFor(() => {
      expect(shownPercent()).toBe("100 %");
    });

    fireEvent.click(screen.getByRole("button", { name: "Zoom in" }));
    await waitFor(() => {
      expect(drawnZoom(container)).toBeGreaterThan(1);
    });
    expect(shownPercent()).toBe(`${Math.round(drawnZoom(container) * 100)} %`);

    fireEvent.click(screen.getByRole("button", { name: "Zoom out" }));
    fireEvent.click(screen.getByRole("button", { name: "Zoom out" }));
    await waitFor(() => {
      expect(drawnZoom(container)).toBeLessThan(1);
    });
    expect(shownPercent()).toBe(`${Math.round(drawnZoom(container) * 100)} %`);
  });

  it("fits the board into the pane, with React Flow's margin of a tenth", async () => {
    const { container } = await zoomCanvas();
    fireEvent.click(screen.getByTitle("Zoom to 100 %"));
    await waitFor(() => {
      expect(drawnZoom(container)).toBe(1);
    });

    fireEvent.click(
      screen.getByRole("button", { name: "Fit the board into view" }),
    );

    // The two boxes span 380 × 90 in a pane of 500 × 500: the width decides.
    await waitFor(() => {
      expect(drawnZoom(container)).toBeCloseTo(500 / (380 * 1.1));
    });
    expect(shownPercent()).toBe("120 %");
  });

  it("fits a board too wide for the pane at the lowest zoom, 10 %", async () => {
    const { container } = await zoomCanvas(wideBoard());
    fireEvent.click(screen.getByTitle("Zoom to 100 %"));
    await waitFor(() => {
      expect(drawnZoom(container)).toBe(1);
    });

    fireEvent.click(
      screen.getByRole("button", { name: "Fit the board into view" }),
    );

    await waitFor(() => {
      expect(drawnZoom(container)).toBeCloseTo(0.1);
    });
    expect(shownPercent()).toBe("10 %");
  });

  it("zooms in no further than 200 %", async () => {
    const { container } = await zoomCanvas();
    const zoomIn = screen.getByRole("button", { name: "Zoom in" });

    for (let click = 0; click < 10; click += 1) {
      fireEvent.click(zoomIn);
    }

    await waitFor(() => {
      expect(drawnZoom(container)).toBe(2);
    });
    expect(shownPercent()).toBe("200 %");
  });

  it("zooms with the keys +, -, and 0, with Shift where + needs it", async () => {
    const { container } = await zoomCanvas();
    fireEvent.click(screen.getByTitle("Zoom to 100 %"));
    await waitFor(() => {
      expect(drawnZoom(container)).toBe(1);
    });

    const plus = fireEvent.keyDown(window, { key: "+", shiftKey: true });
    await waitFor(() => {
      expect(drawnZoom(container)).toBeGreaterThan(1);
    });
    const zoomedIn = drawnZoom(container);

    const minus = fireEvent.keyDown(window, { key: "-" });
    await waitFor(() => {
      expect(drawnZoom(container)).toBeLessThan(zoomedIn);
    });

    fireEvent.click(screen.getByTitle("Zoom to 100 %"));
    await waitFor(() => {
      expect(drawnZoom(container)).toBe(1);
    });
    const zero = fireEvent.keyDown(window, { key: "0" });
    await waitFor(() => {
      expect(drawnZoom(container)).not.toBe(1);
    });

    // fireEvent returns false when a handler prevented the default.
    expect([plus, minus, zero]).toEqual([false, false, false]);
  });

  it("leaves the zoom and the key alone while a note's text field has the focus", async () => {
    const { container } = await zoomCanvas();
    fireEvent.click(screen.getByTitle("Zoom to 100 %"));
    await waitFor(() => {
      expect(drawnZoom(container)).toBe(1);
    });
    fireEvent.keyDown(window, { key: "n" });
    const field = screen.getByRole("textbox", { name: "Text of the note" });
    expect(document.activeElement).toBe(field);

    const notPrevented = ["+", "-", "0"].map((key) =>
      fireEvent.keyDown(field, { key }),
    );

    expect(notPrevented).toEqual([true, true, true]);
    await act(async () => {
      await new Promise((resolve) => setTimeout(resolve, 50));
    });
    expect(drawnZoom(container)).toBe(1);
  });

  it.each([
    ["Ctrl", { key: "+", ctrlKey: true }],
    ["Meta", { key: "-", metaKey: true }],
    ["Alt", { key: "0", altKey: true }],
  ])(
    "leaves the zoom and the key alone with %s held",
    async (_modifier, init) => {
      const { container } = await zoomCanvas();
      fireEvent.click(screen.getByTitle("Zoom to 100 %"));
      await waitFor(() => {
        expect(drawnZoom(container)).toBe(1);
      });

      const notPrevented = fireEvent.keyDown(window, init);

      expect(notPrevented).toBe(true);
      await act(async () => {
        await new Promise((resolve) => setTimeout(resolve, 50));
      });
      expect(drawnZoom(container)).toBe(1);
    },
  );

  it("keeps React Flow's attribution link, at the bottom left", async () => {
    const { container } = await zoomCanvas();

    const attribution = container.querySelector(".react-flow__attribution");
    const link = attribution?.querySelector("a");

    expect(attribution?.classList).toContain("bottom");
    expect(attribution?.classList).toContain("left");
    expect(link?.getAttribute("href")).toMatch(/^https:\/\/reactflow\.dev/);
    expect(link?.getAttribute("rel")).toBe("noopener noreferrer");
    expect(
      container.querySelector(".zoom-bar")?.closest(".react-flow__panel")
        ?.classList,
    ).toContain("right");
  });
});

describe("highlighting a selection", () => {
  /**
   * `orders` uses `payments` and is used by `shop`; `payments` uses `ledger`,
   * which has nothing to do with `orders`; the note `ask` hangs on `ledger`.
   */
  function linkedBoard(mode: BoardView["mode"] = "free"): BoardView {
    return boardView({
      mode,
      placements: [
        placedElement("orders"),
        placedElement("payments", { x: 300 }),
        placedElement("shop", { y: 300 }),
        placedElement("ledger", { x: 300, y: 300 }),
        placedBox("ask", {
          x: 10,
          y: -40,
          shown: { kind: "note", name: "Ask", body: "" },
          attached_to: "ledger",
        }),
      ],
      relations: [
        { from: "ask", kind: "concerns", to: "ledger", label: null },
        { from: "orders", kind: "uses", to: "payments", label: null },
        { from: "payments", kind: "uses", to: "ledger", label: null },
        { from: "shop", kind: "see_also", to: "orders", label: null },
      ],
    });
  }

  /** The spies of every call a canvas makes that writes something. */
  function writes() {
    return {
      move: vi.fn(() => Promise.resolve()),
      release: vi.fn(),
      writeNote: vi.fn(() => Promise.resolve(true)),
      createNote: vi.fn(() => Promise.resolve("new-note")),
      deleteNotes: vi.fn(),
    };
  }

  function highlightCanvas(
    view: BoardView,
    spies: ReturnType<typeof writes> = writes(),
  ) {
    return render(
      canvasOf(
        view,
        spies.move,
        () => {},
        spies.release,
        spies.writeNote,
        spies.createNote,
        spies.deleteNotes,
      ),
    );
  }

  /** Selects the box of `element` from the keyboard, as a person can. */
  async function select(container: HTMLElement, element: string) {
    const node = box(container, element);
    fireEvent.keyDown(node, { key: "Enter" });
    fireEvent.keyUp(node, { key: "Enter" });
    await waitFor(() => expect(node.classList.contains("selected")).toBe(true));
  }

  /** Adds the box of `element` to the selection, with the multi-select key held. */
  async function addToSelection(container: HTMLElement, element: string) {
    const held = navigator.userAgent.includes("Mac") ? "Meta" : "Control";
    fireEvent.keyDown(window, { key: held });
    await select(container, element);
    fireEvent.keyUp(window, { key: held });
  }

  function arrow(
    container: HTMLElement,
    from: string,
    kind: string,
    to: string,
  ) {
    const found = container.querySelector(
      `[data-testid="rf__edge-${from}|${kind}|${to}"]`,
    );
    if (!(found instanceof SVGElement)) {
      throw new Error(`no arrow ${from} ${kind} ${to}`);
    }
    return found;
  }

  function roleOf(container: HTMLElement, element: string): string[] {
    const wrapper = box(container, element).querySelector(".element-box");
    return [...(wrapper?.classList ?? [])];
  }

  function strokeOf(edge: SVGElement): string {
    return (
      edge.querySelector<SVGPathElement>(".react-flow__edge-path")?.style
        .stroke ?? ""
    );
  }

  it("looks as today with nothing selected", () => {
    const { container } = highlightCanvas(linkedBoard());

    for (const element of ["orders", "payments", "shop", "ledger", "ask"]) {
      expect(roleOf(container, element)).toEqual(["element-box"]);
    }
    for (const edge of container.querySelectorAll(".react-flow__edge")) {
      expect(edge.classList.contains("edge-dimmed")).toBe(false);
      expect(edge.classList.contains("edge-leaving")).toBe(false);
      expect(edge.classList.contains("edge-entering")).toBe(false);
    }
  });

  it("colors the arrows leaving and entering a selected box and the boxes at their ends", async () => {
    const { container } = highlightCanvas(linkedBoard());

    await select(container, "orders");

    const leaving = arrow(container, "orders", "uses", "payments");
    const entering = arrow(container, "shop", "see_also", "orders");
    expect(leaving.classList).toContain("edge-leaving");
    expect(strokeOf(leaving)).toBe("rgb(217, 72, 15)");
    expect(
      leaving
        .querySelector(".react-flow__edge-path")
        ?.getAttribute("marker-end"),
    ).toContain("d9480f");
    expect(entering.classList).toContain("edge-entering");
    expect(strokeOf(entering)).toBe("rgb(43, 138, 62)");
    expect(
      entering
        .querySelector(".react-flow__edge-path")
        ?.getAttribute("marker-end"),
    ).toContain("2b8a3e");
    for (const edge of [leaving, entering]) {
      expect(
        edge.querySelector<SVGPathElement>(".react-flow__edge-path")?.style
          .strokeWidth,
      ).toBe("2.5");
    }
    // The dash of a see_also stays.
    expect(
      entering.querySelector<SVGPathElement>(".react-flow__edge-path")?.style
        .strokeDasharray,
    ).toBe("6 4");
    expect(roleOf(container, "orders")).toContain("box-selected");
    expect(roleOf(container, "payments")).toContain("box-leaving");
    expect(roleOf(container, "shop")).toContain("box-entering");
  });

  it("dims every other arrow and box", async () => {
    const { container } = highlightCanvas(linkedBoard());

    await select(container, "orders");

    expect(arrow(container, "payments", "uses", "ledger").classList).toContain(
      "edge-dimmed",
    );
    expect(roleOf(container, "ledger")).toContain("box-dimmed");
    expect(roleOf(container, "ask")).toContain("box-dimmed");
  });

  it("counts an arrow between two selected boxes as leaving, and joins their arrows", async () => {
    const { container } = highlightCanvas(linkedBoard());

    await select(container, "orders");
    await addToSelection(container, "payments");

    expect(arrow(container, "orders", "uses", "payments").classList).toContain(
      "edge-leaving",
    );
    expect(arrow(container, "payments", "uses", "ledger").classList).toContain(
      "edge-leaving",
    );
    expect(arrow(container, "shop", "see_also", "orders").classList).toContain(
      "edge-entering",
    );
    expect(roleOf(container, "orders")).toContain("box-selected");
    expect(roleOf(container, "payments")).toContain("box-selected");
    expect(roleOf(container, "ledger")).toContain("box-leaving");
    expect(roleOf(container, "shop")).toContain("box-entering");
  });

  it("gives a box at both kinds of ends the leaving color", async () => {
    const view = linkedBoard();
    const { container } = highlightCanvas({
      ...view,
      relations: [
        ...view.relations,
        { from: "payments", kind: "depends_on", to: "orders", label: null },
      ],
    });

    await select(container, "orders");

    expect(roleOf(container, "payments")).toContain("box-leaving");
    expect(
      arrow(container, "payments", "depends_on", "orders").classList,
    ).toContain("edge-entering");
  });

  it("keeps the box a selected note hangs on undimmed", async () => {
    const { container } = highlightCanvas(linkedBoard());

    await select(container, "ask");

    expect(roleOf(container, "ask")).toContain("box-selected");
    expect(roleOf(container, "ledger")).toContain("box-kept");
    expect(roleOf(container, "orders")).toContain("box-dimmed");
    expect(arrow(container, "payments", "uses", "ledger").classList).toContain(
      "edge-dimmed",
    );
  });

  it("keeps the notes hanging on a selected box undimmed", async () => {
    const { container } = highlightCanvas(linkedBoard());

    await select(container, "ledger");

    expect(roleOf(container, "ledger")).toContain("box-selected");
    expect(roleOf(container, "ask")).toContain("box-kept");
    expect(roleOf(container, "shop")).toContain("box-dimmed");
  });

  it("keeps a new note's text field undimmed while a box stays selected", async () => {
    const { container } = highlightCanvas(linkedBoard());
    await select(container, "orders");

    fireEvent.click(screen.getByRole("button", { name: "New note" }));

    const field = screen.getByRole("textbox", { name: "Text of the note" });
    expect(document.activeElement).toBe(field);
    expect(box(container, "orders").classList).toContain("selected");
    const wrapper = field.closest(".element-box");
    expect(wrapper?.classList).not.toContain("box-dimmed");
    expect(roleOf(container, "ledger")).toContain("box-dimmed");
  });

  it("keeps highlighted arrows in their layer below the boxes", async () => {
    const { container } = highlightCanvas(linkedBoard());

    await select(container, "orders");

    const edges = [...container.querySelectorAll(".react-flow__edge")];
    expect(
      edges.filter((edge) => edge.classList.contains("edge-leaving")),
    ).toHaveLength(1);
    for (const edge of edges) {
      const layer = edge.closest("svg");
      expect(["", "0"]).toContain(layer?.style.zIndex);
    }
  });

  it("ends the highlighting on a click on an empty spot", async () => {
    const { container } = highlightCanvas(linkedBoard());
    await select(container, "orders");

    const pane = container.querySelector(".react-flow__pane");
    if (pane === null) {
      throw new Error("no pane");
    }
    fireEvent.click(pane);

    await waitFor(() =>
      expect(roleOf(container, "ledger")).toEqual(["element-box"]),
    );
    expect(
      arrow(container, "payments", "uses", "ledger").classList.contains(
        "edge-dimmed",
      ),
    ).toBe(false);
  });

  it("leaves every arrow locked: not selectable, focusable, deletable, or reconnectable", async () => {
    const { container } = highlightCanvas(linkedBoard());

    await select(container, "orders");

    const edges = [...container.querySelectorAll(".react-flow__edge")];
    expect(edges.length).toBe(3);
    for (const edge of edges) {
      expect(edge.classList.contains("selectable")).toBe(false);
      expect(edge.getAttribute("tabindex")).toBeNull();
      expect(edge.querySelector(".react-flow__edge-interaction")).toBeNull();
      expect(edge.querySelector(".react-flow__edgeupdater")).toBeNull();
    }
  });

  it.each([
    ["a free board", "free"],
    ["a mirrored board", "mirrored"],
  ] as const)(
    "writes nothing while selecting and deselecting on %s",
    async (_board, mode) => {
      const spies = writes();
      const { container } = highlightCanvas(linkedBoard(mode), spies);

      await select(container, "orders");
      await addToSelection(container, "ask");
      await select(container, "payments");
      const pane = container.querySelector(".react-flow__pane");
      if (pane !== null) {
        fireEvent.click(pane);
      }
      await waitFor(() =>
        expect(roleOf(container, "orders")).toEqual(["element-box"]),
      );

      for (const spy of Object.values(spies)) {
        expect(spy).not.toHaveBeenCalled();
      }
    },
  );

  it("puts no text from the repository into a class or a style", async () => {
    const hostile = 'x" ; } <b> y';
    const { container } = highlightCanvas(
      boardView({
        placements: [
          placedElement("orders", {
            shown: {
              kind: "element",
              type_ref: hostile,
              name: hostile,
              summary: hostile,
            },
          }),
          placedElement("payments", { x: 300 }),
        ],
        relations: [
          {
            from: "orders",
            kind: "uses",
            to: "payments",
            label: "<img src=x onerror=alert(1)>",
          },
        ],
      }),
    );

    await select(container, "orders");

    const roles = new Set([
      "element-box",
      "box-selected",
      "box-kept",
      "box-leaving",
      "box-entering",
      "box-dimmed",
    ]);
    for (const wrapper of container.querySelectorAll(".element-box")) {
      const classes = [...wrapper.classList];
      expect(classes.filter((name) => !roles.has(name))).toEqual([]);
      expect(classes.filter((name) => name !== "element-box")).toHaveLength(1);
    }
    for (const element of container.querySelectorAll("[class], [style]")) {
      expect(element.getAttribute("class") ?? "").not.toContain("<b>");
      expect(element.getAttribute("style") ?? "").not.toContain("<b>");
      expect(element.getAttribute("style") ?? "").not.toContain("onerror");
    }
    expect(container.querySelector("img")).toBeNull();
    expect(container.textContent).toContain("<img src=x onerror=alert(1)>");
  });

  it("drops a selected box the next view no longer holds without failing", async () => {
    const { container, rerender } = highlightCanvas(linkedBoard());
    await select(container, "orders");
    await addToSelection(container, "payments");

    const view = linkedBoard();
    rerender(
      canvasOf(
        {
          ...view,
          placements: view.placements.filter(
            (placement) => placement.element !== "orders",
          ),
          relations: view.relations.filter(
            (relation) =>
              relation.from !== "orders" &&
              relation.to !== "orders" &&
              relation.from !== "shop",
          ),
        },
        () => Promise.resolve(),
      ),
    );

    await waitFor(() =>
      expect(container.querySelector('[data-id="orders"]')).toBeNull(),
    );
    expect(arrow(container, "payments", "uses", "ledger").classList).toContain(
      "edge-leaving",
    );
    expect(roleOf(container, "ledger")).toContain("box-leaving");
  });

  it("selects a note whose box is missing from the board without failing", async () => {
    const { container } = highlightCanvas(
      boardView({
        placements: [
          placedElement("orders"),
          placedBox("ask", {
            x: 300,
            shown: { kind: "note", name: "Ask", body: "" },
            attached_to: "ghost",
          }),
        ],
      }),
    );

    await select(container, "ask");

    expect(roleOf(container, "ask")).toContain("box-selected");
    expect(roleOf(container, "orders")).toContain("box-dimmed");
  });
});
