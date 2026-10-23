import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it } from "vitest";

import type { BoardView } from "./bindings";
import { BoardCanvas } from "./BoardCanvas";

afterEach(cleanup);

describe("BoardCanvas", () => {
  it("shows one box per placement with what the projection says of it", () => {
    const view: BoardView = {
      id: "context",
      title: "Context",
      placements: [
        {
          element: "orders",
          x: 0,
          y: 0,
          width: 180,
          height: 90,
          shown: { kind: "missing" },
        },
        {
          element: "payments",
          x: 200,
          y: 0,
          width: 180,
          height: 90,
          shown: {
            kind: "element",
            type_ref: "c4/container",
            name: "Payments",
            summary: null,
          },
        },
      ],
    };

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
