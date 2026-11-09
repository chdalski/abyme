import { describe, expect, it } from "vitest";

import {
  COLLAPSED,
  DEFAULT_WIDTH,
  MAX_WIDTH,
  MIN_WIDTH,
  clampWidth,
  readCollapsed,
  readWidth,
} from "./sidebarState";

describe("the sidebar's stored state", () => {
  it.each([
    ["none", null],
    ["text", "abc"],
    ["NaN", "NaN"],
    ["Infinity", "Infinity"],
    ["-Infinity", "-Infinity"],
    ["a number too large for a double", "1e400"],
    ["an empty value", ""],
    ["blanks", "  "],
    ["a unit", "16rem"],
    ["markup", "</style><img src=x>"],
  ])("reads %s as the default width", (_value, stored) => {
    expect(readWidth(stored)).toBe(DEFAULT_WIDTH);
  });

  it.each([
    ["5", MIN_WIDTH],
    ["-5", MIN_WIDTH],
    ["100", MAX_WIDTH],
    ["12.5", 12.5],
    [" 20 ", 20],
  ])("clamps the stored width %s to %s", (stored, width) => {
    expect(readWidth(stored)).toBe(width);
  });

  it("clamps any width it is given, and gives the default for no number", () => {
    expect(clampWidth(3)).toBe(MIN_WIDTH);
    expect(clampWidth(55)).toBe(MAX_WIDTH);
    expect(clampWidth(Number.NaN)).toBe(DEFAULT_WIDTH);
  });

  it.each([
    ["none", null],
    ["TRUE", "TRUE"],
    ["1", "1"],
    ["the value with a blank", ` ${COLLAPSED}`],
    ["an object", "{}"],
    ["an empty value", ""],
  ])("reads %s as not collapsed", (_value, stored) => {
    expect(readCollapsed(stored)).toBe(false);
  });

  it("reads only the value it writes as collapsed", () => {
    expect(readCollapsed(COLLAPSED)).toBe(true);
  });
});
