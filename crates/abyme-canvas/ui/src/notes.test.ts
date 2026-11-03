import { describe, expect, it } from "vitest";

import {
  noteText,
  splitNoteText,
  toNewNote,
  toNoteDeletion,
  toNoteUpdate,
} from "./notes";

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

describe("splitNoteText", () => {
  it("makes the first line the name and the rest the body", () => {
    expect(splitNoteText("Ask why\nIt retries.\n\nTwice.")).toEqual({
      name: "Ask why",
      body: "It retries.\n\nTwice.",
    });
  });

  it("makes one line a name with an empty body", () => {
    expect(splitNoteText("Ask why")).toEqual({ name: "Ask why", body: "" });
  });

  it("drops blank lines at the start and whitespace at the end", () => {
    expect(splitNoteText("\n  \n  Ask why  \nIt retries.  \n\n")).toEqual({
      name: "Ask why",
      body: "It retries.",
    });
  });

  it.each(["", " ", "\n", " \n\t\n ", "\u0085", "\u2028\n"])(
    "calls %j blank",
    (text) => {
      expect(splitNoteText(text)).toBeNull();
    },
  );

  it.each(["\r\n", "\n", "\r", "\u0085", "\u2028", "\u2029"])(
    "ends the name at the line break %j",
    (lineBreak) => {
      expect(splitNoteText(`Ask${lineBreak}why`)).toEqual({
        name: "Ask",
        body: "why",
      });
    },
  );

  it.each([
    ["Ask why", ""],
    ["Ask why", "It retries."],
    ["Ask why", "\nIt retries.\n\n  Twice."],
  ])("gets back what noteText joined: %j, %j", (name, body) => {
    expect(splitNoteText(noteText(name, body))).toEqual({ name, body });
  });

  it("does not change with a line break added at the end", () => {
    expect(splitNoteText("Ask why\nIt retries.\n")).toEqual(
      splitNoteText("Ask why\nIt retries."),
    );
  });
});

describe("toNewNote", () => {
  it("creates, places, and pins a note of the kind idea, at integer coordinates", () => {
    expect(
      toNewNote("context", { x: 40.4, y: -80.6 }, { name: "Ask", body: "Why" }),
    ).toEqual([
      {
        op: "create_element",
        key: "note",
        type_ref: "note",
        name: "Ask",
        body: "Why",
        statement: "idea",
      },
      {
        op: "place_element",
        element: { new: "note" },
        board: { existing: "context" },
      },
      {
        op: "move_element",
        element: { new: "note" },
        board: { existing: "context" },
        x: 40,
        y: -81,
        pinned: true,
      },
    ]);
  });
});

describe("toNoteDeletion", () => {
  it("names only the element", () => {
    expect(toNoteDeletion("ask")).toEqual({
      op: "delete_element",
      element: { existing: "ask" },
    });
  });
});

describe("toNoteUpdate", () => {
  it("names the element, its name, and its body, and nothing else", () => {
    expect(toNoteUpdate("ask", { name: "Ask", body: "Why" })).toEqual({
      op: "update_element",
      element: { existing: "ask" },
      name: "Ask",
      body: "Why",
    });
  });
});
