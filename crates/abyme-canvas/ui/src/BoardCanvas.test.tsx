import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import type { BoardView } from "./bindings";
import { BoardCanvas } from "./BoardCanvas";
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
function canvasOf(view: BoardView, move: () => Promise<void>) {
  return (
    <div style={{ width: 800, height: 600 }}>
      <BoardCanvas view={view} move={move} onMoveFailed={() => {}} />
    </div>
  );
}

const DRAGGABLE_BOX = ".react-flow__node.draggable";

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
          onMoveFailed={() => {}}
        />
      </div>,
    );

    expect(container.querySelectorAll(".react-flow__node")).toHaveLength(2);
    expect(screen.getByText("Missing element orders")).toBeDefined();
    expect(screen.getByText("Payments")).toBeDefined();
  });
});
