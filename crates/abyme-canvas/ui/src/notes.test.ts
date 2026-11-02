import { describe, expect, it } from "vitest";

import { noteText } from "./notes";

describe("noteText", () => {
  it("is the name alone when the body is empty", () => {
    expect(noteText("Ask why", "")).toBe("Ask why");
  });

  it("puts the body on the lines after the name", () => {
    expect(noteText("Ask why", "It retries.\nTwice.")).toBe(
      "Ask why\nIt retries.\nTwice.",
    );
  });
});
