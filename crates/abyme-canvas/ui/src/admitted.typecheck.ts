// Checked by `tsc` in `pnpm check`, never run: the client's `apply` takes the
// operations the Rust side admits and nothing else.
import type { AbymeClient } from "./client";

export function onlyAdmittedOperations(client: AbymeClient): void {
  const element = { existing: "payments" };
  const board = { existing: "context" };

  void client.apply([
    { op: "move_element", element, board, x: 1, y: 2, pinned: true },
  ]);
  void client.apply([
    // @ts-expect-error The canvas pins what a person moves.
    { op: "move_element", element, board, x: 1, y: 2 },
  ]);
  void client.apply([
    // @ts-expect-error The canvas may not release a pin.
    { op: "move_element", element, board, x: 1, y: 2, pinned: false },
  ]);
  void client.apply([
    // @ts-expect-error The canvas may not create elements.
    { op: "create_element", key: "e", type_ref: "c4/container", name: "E" },
  ]);
  void client.apply([
    // @ts-expect-error The canvas may not create boards.
    { op: "create_board", key: "b", title: "B" },
  ]);
  void client.apply([
    // @ts-expect-error The canvas may not place elements.
    { op: "place_element", element, board },
  ]);
  void client.apply([
    {
      op: "move_element",
      // @ts-expect-error The canvas may not move an element the request creates.
      element: { new: "e" },
      board,
      x: 1,
      y: 2,
      pinned: true,
    },
  ]);
  void client.apply([
    {
      op: "move_element",
      element,
      // @ts-expect-error The canvas may not move an element on a board the request creates.
      board: { new: "b" },
      x: 1,
      y: 2,
      pinned: true,
    },
  ]);
}
