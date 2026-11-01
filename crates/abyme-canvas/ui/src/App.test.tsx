import {
  cleanup,
  fireEvent,
  render,
  screen,
  waitFor,
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

  it("stops listening when it is closed", async () => {
    const client = new FakeClient([]);
    const { unmount } = render(<App client={client} />);
    await waitFor(() => expect(client.listening).toBe(1));

    unmount();

    await waitFor(() => expect(client.listening).toBe(0));
  });
});
