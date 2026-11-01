import {
  cleanup,
  fireEvent,
  render,
  screen,
  waitFor,
  within,
} from "@testing-library/react";
import { afterEach, describe, expect, it } from "vitest";

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
    await waitFor(() => expect(client.listening).toBe(1));

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
    await waitFor(() => expect(client.listening).toBe(2));

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
    await waitFor(() => expect(client.listening).toBe(1));

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
