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
  void client.apply([{ op: "delete_element", element }]);
  void client.apply([
    // @ts-expect-error The canvas deletes only an element that exists.
    { op: "delete_element", element: { new: "e" } },
  ]);
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
  const create = {
    op: "create_element",
    key: "n",
    type_ref: "note",
    name: "N",
    body: "",
    statement: "idea",
  } as const;
  const place = {
    op: "place_element",
    element: { new: "n" },
    board,
  } as const;
  const pin = {
    op: "move_element",
    element: { new: "n" },
    board,
    x: 1,
    y: 2,
    pinned: true,
  } as const;
  void client.apply([create, place, pin]);
  void client.apply([
    // @ts-expect-error The canvas creates only notes.
    { ...create, type_ref: "c4/container" },
    place,
    pin,
  ]);
  void client.apply([
    // @ts-expect-error A new note is of the statement kind idea.
    { ...create, statement: "current" },
    place,
    pin,
  ]);
  // @ts-expect-error A new note is created, placed, and pinned, in one request.
  void client.apply([create]);
  // @ts-expect-error A new note is created, placed, and pinned, in one request.
  void client.apply([create, place]);
  // @ts-expect-error A new note comes with nothing else in its request.
  void client.apply([create, place, pin, pin]);
  void client.apply([
    // @ts-expect-error The canvas may not create elements of any other kind.
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
  const note = { existing: "ask" };
  void client.apply([
    { op: "move_element", element: note, board, x: 1, y: 2, pinned: true },
    { op: "attach_note", note, to: element, board, pinned: true },
  ]);
  void client.apply([{ op: "detach_note", note, board }]);
  void client.apply([
    // @ts-expect-error An attachment from the canvas always confirms the pin.
    { op: "attach_note", note, to: element, board },
  ]);
  void client.apply([
    // @ts-expect-error Attaching never releases a pin.
    { op: "attach_note", note, to: element, board, pinned: false },
  ]);
  void client.apply([
    {
      op: "attach_note",
      // @ts-expect-error The canvas attaches only a note that exists.
      note: { new: "n" },
      to: element,
      board,
      pinned: true,
    },
  ]);
  void client.apply([
    {
      op: "attach_note",
      note,
      // @ts-expect-error The canvas attaches only to an element that exists.
      to: { new: "e" },
      board,
      pinned: true,
    },
  ]);
  void client.apply([
    // @ts-expect-error The canvas detaches only on a board that exists.
    { op: "detach_note", note, board: { new: "b" } },
  ]);
  void client.apply([
    // @ts-expect-error A detachment takes no box.
    { op: "detach_note", note, board, to: element },
  ]);
}
