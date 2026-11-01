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
 * decides the dash. An arrow cannot be selected, deleted, or reconnected.
 */
export function toEdges(view: BoardView): Edge[] {
  return view.relations.map((relation) => ({
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
