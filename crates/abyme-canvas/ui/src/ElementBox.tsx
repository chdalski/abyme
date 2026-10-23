import type { NodeProps } from "@xyflow/react";

import type { ElementData, ElementNode } from "./nodes";

/** The box of an element on the canvas. */
export function ElementBox({ data }: NodeProps<ElementNode>) {
  return <ElementContent element={data.element} shown={data.shown} />;
}

/**
 * What a box shows: the element's type, name, and summary, or a note that
 * its file is missing. Text from the repository is rendered as text only.
 */
export function ElementContent({
  element,
  shown,
}: Pick<ElementData, "element" | "shown">) {
  switch (shown.kind) {
    case "element":
      return (
        <div className="element">
          <span className="element-type">{shown.type_ref}</span>
          <strong className="element-name">{shown.name}</strong>
          {shown.summary !== null && (
            <p className="element-summary">{shown.summary}</p>
          )}
        </div>
      );
    case "missing":
      return (
        <div className="element element-missing">Missing element {element}</div>
      );
  }
}
