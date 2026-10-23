import { describe, expect, it, vi } from "vitest";

import { coalesced, loadBoards } from "./boards";
import { FakeClient } from "./fakeClient";

describe("loadBoards", () => {
  it("turns a rejection into the message shown", async () => {
    expect(
      await loadBoards(new FakeClient("the board cannot be read")),
    ).toEqual({
      kind: "failed",
      message: "the board cannot be read",
    });
  });
});

describe("coalesced", () => {
  it("runs once more after a run, however many calls arrive during it", async () => {
    let runs = 0;
    const finishes: (() => void)[] = [];
    const run = coalesced(
      () =>
        new Promise<void>((resolve) => {
          runs += 1;
          finishes.push(resolve);
        }),
    );

    const first = run();
    void run();
    void run();
    finishes[0]?.();
    await vi.waitFor(() => expect(runs).toBe(2));
    finishes[1]?.();
    await first;

    expect(runs).toBe(2);
  });
});
