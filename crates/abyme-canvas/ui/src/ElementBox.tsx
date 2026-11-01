import { Handle, type NodeProps, Position } from "@xyflow/react";
import { createContext, useContext } from "react";

import type { ElementData, ElementNode } from "./nodes";

/** Opens a board: how a box reaches the app's choice of the open board. */
export const OpenBoard = createContext<(board: string) => void>(() => {});

/**
 * The buttons of the boards `boards` that an element owns. They neither start
 * a drag nor move a selected box with the arrow keys, and a title is text only.
 */
function InnerBoards({ boards }: { boards: ElementData["innerBoards"] }) {
  const open = useContext(OpenBoard);
  return (
    <div className="element-inner-boards">
      {boards.map((board) => (
        <button
          key={board.id}
          type="button"
          className="nodrag nopan nokey"
          onClick={() => open(board.id)}
          onDoubleClick={(event) => event.stopPropagation()}
        >
          ▸ {board.title}
        </button>
      ))}
    </div>
  );
}

/**
 * The box of an element on the canvas. Its handles are hidden and connect
 * nothing: React Flow measures them, and without them it forgets where arrows
 * start and end after it measures the box.
 */
export function ElementBox({ data }: NodeProps<ElementNode>) {
  return (
    <>
      <Handle
        type="target"
        position={Position.Top}
        isConnectable={false}
        className="element-handle"
      />
      <ElementContent element={data.element} shown={data.shown} />
      {data.innerBoards.length > 0 && <InnerBoards boards={data.innerBoards} />}
      <Handle
        type="source"
        position={Position.Bottom}
        isConnectable={false}
        className="element-handle"
      />
    </>
  );
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
