import { describe, expect, it } from "vitest";

import { tailSide } from "./tail";

/** A note of 100 by 50 with its centre at 150, 125. */
const note = { x: 100, y: 100, width: 100, height: 50 };

/** A box of 40 by 20 with its centre at `x`, `y`. */
const boxAt = (x: number, y: number) => ({
  x: x - 20,
  y: y - 10,
  width: 40,
  height: 20,
});

describe("tailSide", () => {
  it.each([
    ["above", boxAt(160, 0), "top"],
    ["right of", boxAt(400, 140), "right"],
    ["below", boxAt(140, 300), "bottom"],
    ["left of", boxAt(0, 110), "left"],
  ])("points to a box whose centre lies %s the note's", (_where, box, side) => {
    expect(tailSide(note, box)).toBe(side);
  });

  it("goes by the centres where the rects overlap", () => {
    // The box covers the note's left half; its centre lies left of the note's.
    expect(tailSide(note, { x: 90, y: 90, width: 70, height: 70 })).toBe(
      "left",
    );
  });

  it("takes the vertical side on a diagonal tie", () => {
    expect(tailSide(note, boxAt(250, 25))).toBe("top");
    expect(tailSide(note, boxAt(50, 225))).toBe("bottom");
  });

  it("points down with equal centres", () => {
    expect(tailSide(note, boxAt(150, 125))).toBe("bottom");
  });

  it("points down without a box", () => {
    expect(tailSide(note, undefined)).toBe("bottom");
  });

  it.each([
    ["a box without a size", { x: 0, y: 0, width: Number.NaN, height: 20 }],
    [
      "an endless box",
      { x: Number.POSITIVE_INFINITY, y: 0, width: 40, height: 20 },
    ],
  ])("points down for %s", (_name, box) => {
    expect(tailSide(note, box)).toBe("bottom");
  });

  it("points down for a note without a size", () => {
    expect(tailSide({ ...note, height: Number.NaN }, boxAt(400, 140))).toBe(
      "bottom",
    );
  });
});
