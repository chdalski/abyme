import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it } from "vitest";

import { BoardList } from "./BoardList";

afterEach(cleanup);

const MARKUP = ["<img src=x onerror=alert(1)>", "<b>x</b>"];

describe("BoardList", () => {
  it("notes that a project has no boards yet", () => {
    render(<BoardList state={{ kind: "loaded", boards: [] }} />);

    expect(screen.getByText("This project has no boards yet.")).toBeDefined();
  });

  it.each(MARKUP)("shows the title %s as text", (title) => {
    const { container } = render(
      <BoardList
        state={{ kind: "loaded", boards: [{ id: "board", title }] }}
      />,
    );

    expect(screen.getByRole("listitem").textContent).toBe(title);
    expect(container.querySelector("img, b")).toBeNull();
  });

  it.each(MARKUP)("shows the message %s as text", (message) => {
    const { container } = render(
      <BoardList state={{ kind: "failed", message }} />,
    );

    expect(screen.getByRole("alert").textContent).toBe(message);
    expect(container.querySelector("img, b")).toBeNull();
  });
});
