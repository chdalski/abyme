import { describe, expect, it, vi } from "vitest";

import { coalesced, inOrder, loadBoards } from "./boards";
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

describe("inOrder", () => {
  it("starts each run after the earlier ones have ended, in the order of the calls", async () => {
    const events: string[] = [];
    const finishes: (() => void)[] = [];
    const run = inOrder(
      (name: string) =>
        new Promise<void>((resolve) => {
          events.push(`start ${name}`);
          finishes.push(() => {
            events.push(`end ${name}`);
            resolve();
          });
        }),
    );

    const first = run("first");
    const second = run("second");
    await vi.waitFor(() => expect(finishes).toHaveLength(1));
    finishes[0]?.();
    await first;
    await vi.waitFor(() => expect(finishes).toHaveLength(2));
    finishes[1]?.();
    await second;

    expect(events).toEqual([
      "start first",
      "end first",
      "start second",
      "end second",
    ]);
  });

  it("runs the next call after a failed one, and keeps the failure for its caller", async () => {
    const ran: string[] = [];
    const run = inOrder((name: string) => {
      ran.push(name);
      return name === "first"
        ? Promise.reject("operation 1: x is out of range")
        : Promise.resolve();
    });

    const first = run("first");
    const second = run("second");

    await expect(first).rejects.toBe("operation 1: x is out of range");
    await expect(second).resolves.toBeUndefined();
    expect(ran).toEqual(["first", "second"]);
  });
});
