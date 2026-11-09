import {
  cleanup,
  fireEvent,
  render,
  screen,
  within,
} from "@testing-library/react";
import { afterEach, describe, expect, it } from "vitest";

import { BoardList, INDENT_LEVELS } from "./BoardList";

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
            { id: "containers", title: "Containers", depth: 0 },
            { id: "context", title: "Context", depth: 0 },
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
        state={{ kind: "loaded", boards: [{ id: "board", title, depth: 0 }] }}
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

describe("the board list as a tree", () => {
  /** The left padding of the row of the board titled `title`. */
  function indentOf(title: string): string {
    return (
      screen.getByRole("button", { name: title }).closest("li")?.style
        .paddingInlineStart ?? ""
    );
  }

  it("indents each board by its depth, below the board it lies inside", () => {
    render(
      <BoardList
        state={{
          kind: "loaded",
          boards: [
            { id: "context", title: "Context", depth: 0 },
            { id: "containers", title: "Containers", depth: 1 },
            { id: "components", title: "Components", depth: 2 },
            { id: "docs", title: "Docs", depth: 0 },
          ],
        }}
        open="components"
        onOpen={ignore}
      />,
    );

    expect(
      screen
        .getAllByRole("listitem")
        .map((item) => within(item).getAllByRole("button").at(-1)?.textContent),
    ).toEqual(["Context", "Containers", "Components", "Docs"]);
    expect(
      ["Context", "Containers", "Components", "Docs"].map(indentOf),
    ).toEqual(["0rem", "1rem", "2rem", "0rem"]);
  });

  it("indents a board deeper than the cap no further than the cap", () => {
    render(
      <BoardList
        state={{
          kind: "loaded",
          boards: [
            { id: "at-cap", title: "At cap", depth: INDENT_LEVELS },
            { id: "deep", title: "Deep", depth: 10_000 },
          ],
        }}
        open="deep"
        onOpen={ignore}
      />,
    );

    expect(indentOf("Deep")).toBe(`${INDENT_LEVELS}rem`);
    expect(indentOf("Deep")).toBe(indentOf("At cap"));
  });
});
