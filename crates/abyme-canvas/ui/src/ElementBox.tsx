import {
  Handle,
  type NodeProps,
  Position,
  type XYPosition,
} from "@xyflow/react";
import { createContext, useContext } from "react";

import { type FlagMarks, NO_MARKS } from "./flags";
import type { Shown } from "./bindings";
import type { ElementData, ElementNode } from "./nodes";
import { NoteContent } from "./NoteContent";

/** Opens a board: how a box reaches the app's choice of the open board. */
export const OpenBoard = createContext<(board: string) => void>(() => {});

/**
 * Releases the pin of an element at its position, or `null` on a board where
 * a person may not.
 */
export const ReleasePin = createContext<
  ((element: string, position: XYPosition) => void) | null
>(null);

/** The levels of the review flags the boxes show. */
export const ReviewMarks = createContext<FlagMarks>(NO_MARKS);

/**
 * The mark of an element's review flag: its level in words, and no reason. It
 * takes no clicks, so it stands in the way of nothing.
 */
function FlagMark({ element }: { element: string }) {
  switch (useContext(ReviewMarks).get(element)) {
    case undefined:
      return null;
    case "task":
      return <span className="element-flag element-mark">⚠ task</span>;
    case "hint":
      return <span className="element-flag element-mark">⚠ hint</span>;
  }
}

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
 * The marker of a pinned box: a button that releases the pin where a person
 * may and the box shows an element whose file exists, which the core can
 * release, or plain text. Like the buttons of inner boards, it neither starts
 * a drag nor moves a selected box with the arrow keys.
 */
function PinMarker({
  element,
  shown,
  position,
}: {
  element: string;
  shown: Shown;
  position: XYPosition;
}) {
  const release = useContext(ReleasePin);
  if (release === null || shown.kind !== "element") {
    return <span className="element-pin element-mark">pinned</span>;
  }
  return (
    <button
      type="button"
      className="element-pin element-mark nodrag nopan nokey"
      aria-label="Release pin"
      title="Release pin"
      onClick={() => release(element, position)}
      onDoubleClick={(event) => event.stopPropagation()}
    >
      pinned ×
    </button>
  );
}

/**
 * The box of an element on the canvas. Its handles are hidden and connect
 * nothing: React Flow measures them, and without them it forgets where arrows
 * start and end after it measures the box. A release of its pin goes by where
 * the box lies now, so it follows a drag still on its way to the board file.
 */
export function ElementBox({
  data,
  positionAbsoluteX,
  positionAbsoluteY,
}: NodeProps<ElementNode>) {
  return (
    <>
      <Handle
        type="target"
        position={Position.Top}
        isConnectable={false}
        className="element-handle"
      />
      <ElementContent
        element={data.element}
        shown={data.shown}
        editing={data.editing}
        leftOut={data.leftOut}
        draftAt={data.draft === null ? undefined : data.home}
      />
      {data.pinned && data.shown.kind !== "note" && (
        <PinMarker
          element={data.element}
          shown={data.shown}
          position={{ x: positionAbsoluteX, y: positionAbsoluteY }}
        />
      )}
      <FlagMark element={data.element} />
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
 * What a box shows: the element's type, name, and summary, a sticky note's
 * whole text, or a note that its file is missing. Text from the repository is rendered as text only.
 */
export function ElementContent({
  element,
  shown,
  editing = false,
  leftOut = false,
  draftAt,
}: Pick<ElementData, "element" | "shown"> & {
  editing?: boolean;
  leftOut?: boolean;
  draftAt?: XYPosition | undefined;
}) {
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
    case "note":
      return (
        <NoteContent
          element={element}
          name={shown.name}
          body={shown.body}
          editing={editing}
          leftOut={leftOut}
          draftAt={draftAt}
        />
      );
    case "missing":
      return (
        <div className="element element-missing">Missing element {element}</div>
      );
  }
}
