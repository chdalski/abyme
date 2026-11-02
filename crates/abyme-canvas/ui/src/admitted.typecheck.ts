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
    { op: "move_element", element, board, x: 1, y: 2, pinned: false },
  ]);
  void client.apply([{ op: "update_element", element, name: "N", body: "" }]);
  void client.apply([
    // @ts-expect-error The canvas changes a note's name and body together.
    { op: "update_element", element, name: "N" },
  ]);
  void client.apply([
    // @ts-expect-error The canvas changes a note's name and body together.
    { op: "update_element", element, body: "" },
  ]);
  void client.apply([
    // @ts-expect-error The canvas changes no element's type.
    { op: "update_element", element, name: "N", body: "", type_ref: "t" },
  ]);
  void client.apply([
    // @ts-expect-error The canvas changes no element's summary.
    { op: "update_element", element, name: "N", body: "", summary: null },
  ]);
  void client.apply([
    // @ts-expect-error The canvas changes no element's anchor.
    { op: "update_element", element, name: "N", body: "", anchor: "src" },
  ]);
  void client.apply([
    // @ts-expect-error The canvas changes no element's statement kind.
    { op: "update_element", element, name: "N", body: "", statement: "idea" },
  ]);
  void client.apply([
    {
      op: "update_element",
      // @ts-expect-error The canvas changes only an element that exists.
      element: { new: "e" },
      name: "N",
      body: "",
    },
  ]);
  void client.apply([
    // @ts-expect-error The canvas says whether a move pins or releases.
    { op: "move_element", element, board, x: 1, y: 2 },
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
