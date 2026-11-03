import { MarkerType } from "@xyflow/react";
import { describe, expect, it } from "vitest";

import type { Relation, RelationKind } from "./bindings";
import { toEdges } from "./edges";
import { boardView, placedBox } from "./testBoards";

function edgeOf(overrides: Partial<Relation> = {}) {
  const relation: Relation = {
    from: "a",
    kind: "uses",
    to: "b",
    label: null,
    ...overrides,
  };
  const [edge] = toEdges(boardView({ relations: [relation] }));
  return edge;
}

/** The fields of an arrow that do not depend on the relation's text. */
const FIXED = {
  type: "default",
  markerEnd: { type: MarkerType.ArrowClosed },
  selectable: false,
  focusable: false,
  deletable: false,
  reconnectable: false,
  interactionWidth: 0,
};

describe("toEdges", () => {
  it("draws a relation from its source to its target, with fixed fields", () => {
    expect(edgeOf({ label: "REST" })).toEqual({
      id: "a|uses|b",
      source: "a",
      target: "b",
      label: "REST",
      ...FIXED,
    });
  });

  it("gives a relation without a label no text", () => {
    expect(edgeOf()).toEqual({
      id: "a|uses|b",
      source: "a",
      target: "b",
      ...FIXED,
    });
    expect(edgeOf({ label: "" })).toEqual(edgeOf());
  });

  it("dashes only a see_also", () => {
    const kinds: RelationKind[] = [
      "concerns",
      "depends_on",
      "describes",
      "uses",
    ];
    for (const kind of kinds) {
      expect(edgeOf({ kind })?.style).toBeUndefined();
    }
    expect(edgeOf({ kind: "see_also" })?.style).toEqual({
      strokeDasharray: "6 4",
    });
  });

  it("keeps a label of 40 characters and cuts one of 41", () => {
    expect(edgeOf({ label: "x".repeat(40) })?.label).toBe("x".repeat(40));
    expect(edgeOf({ label: "x".repeat(41) })?.label).toBe(`${"x".repeat(40)}…`);
  });

  it("cuts by whole characters, so an emoji at the cut stays whole", () => {
    const label = `${"x".repeat(39)}😀😀`;

    expect(edgeOf({ label })?.label).toBe(`${"x".repeat(39)}😀…`);
  });
});

describe("the arrow of an attached note", () => {
  const relations: Relation[] = [
    { from: "n", kind: "concerns", to: "b", label: null },
    { from: "n", kind: "concerns", to: "c", label: null },
    { from: "n", kind: "see_also", to: "b", label: null },
    { from: "m", kind: "concerns", to: "b", label: null },
  ];

  it("is hidden for concerns to the box it hangs on, and no other", () => {
    const edges = toEdges(
      boardView({
        placements: [
          placedBox("b"),
          placedBox("c"),
          placedBox("m"),
          placedBox("n", { attached_to: "b" }),
        ],
        relations,
      }),
    );

    expect(edges.map((edge) => edge.id)).toEqual([
      "n|concerns|c",
      "n|see_also|b",
      "m|concerns|b",
    ]);
  });
});
