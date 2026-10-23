import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it } from "vitest";

import { BoardList } from "./BoardList";

afterEach(cleanup);

const MARKUP = ["<img src=x onerror=alert(1)>", "<b>x</b>"];

const ignore = () => {};

describe("BoardList", () => {
  it("notes that a project has no boards yet", () => {
    render(
      <BoardList
        state={{ kind: "loaded", boards: [] }}
        open={null}
        onOpen={ignore}
      />,
    );

    expect(screen.getByText("This project has no boards yet.")).toBeDefined();
  });

  it("opens the board a click names and marks the open one", () => {
    const opened: string[] = [];
    render(
      <BoardList
        state={{
          kind: "loaded",
          boards: [
            { id: "containers", title: "Containers" },
            { id: "context", title: "Context" },
          ],
        }}
        open="containers"
        onOpen={(board) => opened.push(board)}
      />,
    );

    fireEvent.click(screen.getByRole("button", { name: "Context" }));

    expect(opened).toEqual(["context"]);
    expect(screen.getByRole("button", { current: "page" }).textContent).toBe(
      "Containers",
    );
  });

  it.each(MARKUP)("shows the title %s as text", (title) => {
    const { container } = render(
      <BoardList
        state={{ kind: "loaded", boards: [{ id: "board", title }] }}
        open={null}
        onOpen={ignore}
      />,
    );

    expect(screen.getByRole("listitem").textContent).toBe(title);
    expect(container.querySelector("img, b")).toBeNull();
  });

  it.each(MARKUP)("shows the message %s as text", (message) => {
    const { container } = render(
      <BoardList
        state={{ kind: "failed", message }}
        open={null}
        onOpen={ignore}
      />,
    );

    expect(screen.getByRole("alert").textContent).toBe(message);
    expect(container.querySelector("img, b")).toBeNull();
  });
});
