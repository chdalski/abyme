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
import type { Move } from "./nodes";
import type { NoteParts } from "./notes";
import { boardView, placedBox } from "./testBoards";

afterEach(cleanup);

/** A board of two boxes in the given mode. */
function boardIn(mode: BoardView["mode"]): BoardView {
  return boardView({
    mode,
    placements: [placedBox("orders"), placedBox("payments", { x: 200 })],
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
) {
  return (
    <div style={{ width: 800, height: 600 }}>
      <BoardCanvas
        view={view}
        move={move}
        release={release}
        writeNote={writeNote}
        onMoveFailed={() => {}}
        onOpen={onOpen}
      />
    </div>
  );
}

const DRAGGABLE_BOX = ".react-flow__node.draggable";

/** Two boxes in a row and one relation between them with `label`. */
function boardWithArrow(label: string | null, x = 0): BoardView {
  return boardView({
    placements: [placedBox("orders", { x }), placedBox("payments", { y: 300 })],
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
          onOpen={() => {}}
          onMoveFailed={() => {}}
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

    expect(path).toMatch(/^M90\.5,91\b/);
    expect(path).toMatch(/\b90\.5,300$/);
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
      placedBox("plain", { x: 400 }),
      placedBox("shop", {
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
  });

  it("is plain text with no button on a mirrored board", () => {
    const { container } = render(
      canvasOf(
        boardView({
          mode: "mirrored",
          placements: [placedBox("kept", { pinned: true })],
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
              placedBox("kept", {
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
      canvasOf(boardView({ placements: [placedBox("free")] }), () =>
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
        placedBox("plain"),
        placedBox("ask", {
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
    return rect(87, -3, 6, 6);
  }
  if (element.classList.contains("react-flow__handle-bottom")) {
    return rect(87, 87, 6, 6);
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
        boardView({ placements: [placedBox("kept", { pinned: true })] }),
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
