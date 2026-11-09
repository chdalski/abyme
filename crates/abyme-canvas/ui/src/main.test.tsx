import { act, fireEvent, screen, waitFor } from "@testing-library/react";
import { beforeAll, describe, expect, it, vi } from "vitest";

import { FakeClient } from "./fakeClient";

// The entry point runs as in the app, with the Tauri client replaced by a
// fake: it freezes Object.prototype, which stays frozen for this file only,
// since Vitest runs every test file in a process of its own.
const shop = vi.hoisted(() => ({
  boards: [
    { id: "online-shop", title: "Online Shop", depth: 0 },
    { id: "arrows", title: "Arrows", depth: 0 },
  ],
  arrows: {
    id: "arrows",
    title: "Arrows",
    mode: null,
    placements: [
      {
        element: "constructor",
        x: 0,
        y: 0,
        width: 180,
        height: 90,
        shown: { kind: "missing" as const },
        inner_boards: [],
        pinned: false,
        attached_to: null,
      },
      {
        element: "orders",
        x: 0,
        y: 300,
        width: 180,
        height: 90,
        shown: { kind: "missing" as const },
        inner_boards: [],
        pinned: false,
        attached_to: null,
      },
    ],
    relations: [
      {
        from: "constructor",
        kind: "uses" as const,
        to: "orders",
        label: "constructor",
      },
    ],
    trail: [],
  },
  view: {
    id: "online-shop",
    title: "Online Shop",
    mode: null,
    placements: [
      {
        element: "orders-database",
        x: 220,
        y: 0,
        width: 180,
        height: 90,
        shown: { kind: "missing" as const },
        inner_boards: [],
        pinned: false,
        attached_to: null,
      },
      {
        element: "web-shop",
        x: 0,
        y: 0,
        width: 180,
        height: 90,
        shown: {
          kind: "element" as const,
          type_ref: "c4/container",
          name: "Web Shop",
          summary: "Where customers order",
        },
        inner_boards: [],
        pinned: false,
        attached_to: null,
      },
    ],
    relations: [],
    trail: [],
  },
}));

vi.mock("./client", async () => {
  const { FakeClient: Fake } = await import("./fakeClient");
  const client = new Fake(shop.boards);
  client.views = { "online-shop": shop.view, arrows: shop.arrows };
  return { tauriClient: client };
});

async function fakeClient(): Promise<FakeClient> {
  const { tauriClient } = await import("./client");
  if (!(tauriClient instanceof FakeClient)) {
    throw new Error("the client is not the fake");
  }
  return tauriClient;
}

beforeAll(async () => {
  const root = document.createElement("div");
  root.id = "root";
  document.body.append(root);
  await import("./main");
});

describe("main", () => {
  it("freezes Object.prototype", () => {
    expect(Object.isFrozen(Object.prototype)).toBe(true);
  });

  it("refuses to add to the prototype of every object", () => {
    const prototype: Record<string, unknown> = Object.prototype as Record<
      string,
      unknown
    >;

    expect(() => {
      prototype.polluted = 1;
    }).toThrow(TypeError);
    expect(({} as Record<string, unknown>).polluted).toBeUndefined();
  });

  it("draws an arrow from a box whose ID names a property of every object", async () => {
    fireEvent.click(await screen.findByRole("button", { name: "Arrows" }));

    await waitFor(() =>
      expect(
        document.querySelector(".react-flow__edge-text")?.textContent,
      ).toBe("constructor"),
    );
  });

  it("shows the boards, opens one with its boxes, and moves a box, all with the prototype frozen", async () => {
    const client = await fakeClient();

    fireEvent.click(await screen.findByRole("button", { name: "Online Shop" }));
    expect(await screen.findByText("Web Shop")).toBeDefined();
    expect(document.querySelectorAll(".react-flow__node")).toHaveLength(2);
    const box = document.querySelector('.react-flow__node[data-id="web-shop"]');
    if (!(box instanceof HTMLElement)) {
      throw new Error("no box for web-shop");
    }
    fireEvent.keyDown(box, { key: "Enter" });
    fireEvent.keyDown(box, { key: "ArrowRight" });

    await waitFor(() => expect(client.applied).toHaveLength(1));
    const move = client.applied[0]?.[0];
    if (move?.op !== "move_element") {
      throw new Error("no move");
    }
    expect(move.element).toEqual({ existing: "web-shop" });
    expect(move.x).toBeGreaterThan(0);
    expect(move.y).toBe(0);

    // The write reloads the board, as a new answer like every IPC reply; the
    // box stays selected, so the next press moves it again.
    client.views = { "online-shop": structuredClone(shop.view) };
    await act(async () => {
      client.change();
      await Promise.resolve();
    });
    fireEvent.keyDown(box, { key: "ArrowRight" });
    await waitFor(() => expect(client.applied).toHaveLength(2));
    expect(client.applied[1]?.[0]).toMatchObject({
      element: { existing: "web-shop" },
    });
  });
});
