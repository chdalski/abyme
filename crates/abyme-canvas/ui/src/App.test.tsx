import { cleanup, render, screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it } from "vitest";

import { App } from "./App";
import { FakeClient } from "./fakeClient";

afterEach(cleanup);

const NO_PROJECT =
  "no Abyme project in this directory or above it: run `abyme-mcp init` in the directory that is to become the project's root";

describe("App", () => {
  it("shows the titles of the boards the client lists", async () => {
    const client = new FakeClient([
      { id: "containers", title: "Containers" },
      { id: "context", title: "Context" },
    ]);

    render(<App client={client} />);

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
    const client = new FakeClient(NO_PROJECT);

    render(<App client={client} />);

    expect((await screen.findByRole("alert")).textContent).toBe(NO_PROJECT);
  });

  it("stops listening when it is closed", async () => {
    const client = new FakeClient([]);
    const { unmount } = render(<App client={client} />);
    await waitFor(() => expect(client.listening).toBe(1));

    unmount();

    await waitFor(() => expect(client.listening).toBe(0));
  });
});
