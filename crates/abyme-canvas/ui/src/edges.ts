import { type Edge, MarkerType } from "@xyflow/react";

import type { BoardView } from "./bindings";

/** How many characters of a label an arrow shows. */
const LABEL_LIMIT = 40;

/** The dash of a `see_also`, the loosest of the relations. */
const DASHED = { strokeDasharray: "6 4" };

/**
 * `label` as an arrow shows it: as text, cut by whole characters, so that no
 * cut leaves half a surrogate pair. An empty label shows nothing.
 */
function shownLabel(label: string | null): { label: string } | object {
  if (label === null || label === "") {
    return {};
  }
  const characters = Array.from(label);
  return {
    label:
      characters.length > LABEL_LIMIT
        ? `${characters.slice(0, LABEL_LIMIT).join("")}…`
        : label,
  };
}

/**
 * The arrows of `view`, one per relation, built from fixed fields: only the
 * validated IDs and the label come from the repository, and the kind only
 * decides the dash. An arrow cannot be selected, deleted, or reconnected. The
 * relation `concerns` from an attached note to the box it hangs on has none:
 * the note hangs on the box visibly.
 */
export function toEdges(view: BoardView): Edge[] {
  const hangsOn = new Map(
    view.placements.map((placement) => [
      placement.element,
      placement.attached_to,
    ]),
  );
  const shown = view.relations.filter(
    (relation) =>
      relation.kind !== "concerns" ||
      hangsOn.get(relation.from) !== relation.to,
  );
  return shown.map((relation) => ({
    id: `${relation.from}|${relation.kind}|${relation.to}`,
    source: relation.from,
    target: relation.to,
    type: "default",
    markerEnd: { type: MarkerType.ArrowClosed },
    selectable: false,
    focusable: false,
    deletable: false,
    reconnectable: false,
    interactionWidth: 0,
    ...(relation.kind === "see_also" ? { style: DASHED } : {}),
    ...shownLabel(relation.label),
  }));
}

/** The color of an arrow leaving the selection, and of the box it ends at. */
const LEAVING_COLOR = "#d9480f";

/** The color of an arrow entering the selection, and of the box it starts at. */
const ENTERING_COLOR = "#2b8a3e";

/** How thick a highlighted arrow is drawn. */
const HIGHLIGHT_WIDTH = 2.5;

/** What a selection makes of an arrow. */
export type EdgeRole = "leaving" | "entering" | "dimmed";

/** What a selection makes of a box. */
export type BoxRole =
  "none" | "selected" | "kept" | "leaving" | "entering" | "dimmed";

/**
 * What the selection highlights: nothing, or the arrows that leave and enter
 * it, the boxes at their other ends, and the boxes kept as they are, the
 * selection, the boxes its notes hang on, and the notes hanging on it;
 * everything else is dimmed.
 */
export type Focus =
  | { readonly kind: "none" }
  | {
      readonly kind: "focus";
      readonly edges: ReadonlyMap<string, EdgeRole>;
      readonly selected: ReadonlySet<string>;
      readonly ends: ReadonlyMap<string, "leaving" | "entering">;
      readonly kept: ReadonlySet<string>;
    };

/** A focus that highlights nothing. */
export const NO_FOCUS: Focus = { kind: "none" };

/**
 * What the boxes `selected` highlight among the arrows `edges` drawn on `view`:
 * an arrow from a selected box leaves, one to a selected box enters, and one
 * between two selected boxes leaves; a box at both kinds of ends takes the
 * leaving color. A selected note's box, and the notes hanging on a selected
 * box, which hang together visibly without an arrow, stay undimmed. A selected
 * ID the view does not hold is ignored.
 */
export function focusOf(
  view: BoardView,
  edges: readonly Edge[],
  selected: ReadonlySet<string>,
): Focus {
  const hangsOn = new Map(
    view.placements.map((placement) => [
      placement.element,
      placement.attached_to,
    ]),
  );
  const chosen = new Set([...selected].filter((id) => hangsOn.has(id)));
  if (chosen.size === 0) {
    return NO_FOCUS;
  }
  const roles = new Map<string, EdgeRole>(
    edges.map((edge) => [
      edge.id,
      chosen.has(edge.source)
        ? "leaving"
        : chosen.has(edge.target)
          ? "entering"
          : "dimmed",
    ]),
  );
  const ends = new Map<string, "leaving" | "entering">();
  for (const edge of edges) {
    if (roles.get(edge.id) === "leaving" && !chosen.has(edge.target)) {
      ends.set(edge.target, "leaving");
    }
  }
  for (const edge of edges) {
    if (roles.get(edge.id) === "entering" && !ends.has(edge.source)) {
      ends.set(edge.source, "entering");
    }
  }
  const hosts = [...chosen].flatMap((id) => {
    const host = hangsOn.get(id);
    return host === null || host === undefined ? [] : [host];
  });
  const notesOnTheSelection = [...hangsOn]
    .filter(
      ([, host]) => host !== null && host !== undefined && chosen.has(host),
    )
    .map(([note]) => note);
  return {
    kind: "focus",
    edges: roles,
    selected: chosen,
    ends,
    kept: new Set([...hosts, ...notesOnTheSelection]),
  };
}

/** How each role draws an arrow: a fixed class, color, and width. */
const EDGE_LOOKS: Record<
  EdgeRole,
  { className: string; color: string | null }
> = {
  leaving: { className: "edge-leaving", color: LEAVING_COLOR },
  entering: { className: "edge-entering", color: ENTERING_COLOR },
  dimmed: { className: "edge-dimmed", color: null },
};

/**
 * `edges` as `focus` draws them: each keeps everything `toEdges` gave it and
 * gains only a class, a style, and an arrowhead color for its role. It keeps its
 * layer, below the boxes, so no highlighted arrow or label takes a box's clicks.
 */
export function focusedEdges(edges: Edge[], focus: Focus): Edge[] {
  if (focus.kind === "none") {
    return edges;
  }
  return edges.map((edge) => {
    const look = EDGE_LOOKS[focus.edges.get(edge.id) ?? "dimmed"];
    if (look.color === null) {
      return { ...edge, className: look.className };
    }
    return {
      ...edge,
      className: look.className,
      style: {
        ...edge.style,
        stroke: look.color,
        strokeWidth: HIGHLIGHT_WIDTH,
      },
      markerEnd: { type: MarkerType.ArrowClosed, color: look.color },
    };
  });
}

/**
 * The role of the box of `element` under `focus`: the selection first, then an
 * end of a highlighted arrow, then a box or note kept with the selection. A box
 * whose text field is open, `editing`, is never dimmed, so a note being typed
 * stays readable while another box is selected.
 */
export function boxRoleOf(
  focus: Focus,
  element: string,
  editing: boolean,
): BoxRole {
  if (focus.kind === "none") {
    return "none";
  }
  if (focus.selected.has(element)) {
    return "selected";
  }
  const role =
    focus.ends.get(element) ?? (focus.kept.has(element) ? "kept" : "dimmed");
  return editing && role === "dimmed" ? "kept" : role;
}
