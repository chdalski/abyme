import type { BoardView, Placement } from "./bindings";

/** A board view for tests: the board `context`, with `overrides`. */
export function boardView(overrides: Partial<BoardView> = {}): BoardView {
  return {
    id: "context",
    title: "Context",
    mode: null,
    placements: [],
    relations: [],
    trail: [],
    ...overrides,
  };
}

/** A box of the default size at the origin whose file is missing, with `overrides`. */
export function placedBox(
  element: string,
  overrides: Partial<Placement> = {},
): Placement {
  return {
    element,
    x: 0,
    y: 0,
    width: 180,
    height: 90,
    shown: { kind: "missing" },
    inner_boards: [],
    pinned: false,
    attached_to: null,
    ...overrides,
  };
}

/** A box of the default size at the origin whose element file exists, with `overrides`. */
export function placedElement(
  element: string,
  overrides: Partial<Placement> = {},
): Placement {
  return placedBox(element, {
    shown: { kind: "element", type_ref: "t", name: element, summary: null },
    ...overrides,
  });
}
