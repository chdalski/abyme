import "@xyflow/react/dist/style.css";

import {
  type NodeMouseHandler,
  Panel,
  ReactFlow,
  ReactFlowProvider,
  type XYPosition,
  useReactFlow,
} from "@xyflow/react";
import {
  type MouseEvent,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useRef,
} from "react";

import type { BoardView } from "./bindings";
import { useBoardNodes } from "./boardNodes";
import { toEdges } from "./edges";
import { ElementBox, OpenBoard, ReleasePin } from "./ElementBox";
import type { ElementNode, Move } from "./nodes";
import { NewNotes, WriteNote } from "./NoteContent";
import type { NoteParts } from "./notes";
import { RestOfOpeningPress } from "./press";

/** The one node type the canvas knows. */
const NODE_TYPES = { element: ElementBox };

/** The keys that delete the selected notes; React Flow ignores them in a text field. */
const DELETE_KEYS = ["Delete", "Backspace"];

/** Whether a key pressed in `target` belongs to typing. */
function isTyping(target: EventTarget | null): boolean {
  return (
    target instanceof HTMLInputElement ||
    target instanceof HTMLTextAreaElement ||
    (target instanceof HTMLElement && target.isContentEditable)
  );
}

type BoardCanvasProps = {
  view: BoardView;
  /** When `view` was asked for, as a stamp of `nextStamp`; none if unknown. */
  askedAt?: number | null;
  move: (moves: Move[]) => Promise<void>;
  release: (element: string, position: XYPosition) => void;
  writeNote: (element: string, parts: NoteParts) => Promise<boolean>;
  createNote: (
    position: XYPosition,
    parts: NoteParts,
  ) => Promise<string | null>;
  deleteNotes: (elements: string[]) => void;
  onMoveFailed: (message: string) => void;
  onOpen: (board: string) => void;
};

/**
 * A board's boxes and the arrows between them on the canvas. Moving boxes
 * moves their elements; `Delete` and `Backspace` delete the selected notes, and
 * never a box or an arrow. Arrows follow from the view and hold no state. A
 * mirrored board offers no release of a pin, and a move only of a note. The key
 * `N`, a double-click on an empty spot, and the toolbar start a new note, on
 * every board.
 */
export function BoardCanvas(props: BoardCanvasProps) {
  return (
    <ReactFlowProvider>
      <Flow {...props} />
    </ReactFlowProvider>
  );
}

function Flow({
  view,
  askedAt = null,
  move,
  release,
  writeNote,
  createNote,
  deleteNotes,
  onMoveFailed,
  onOpen,
}: BoardCanvasProps) {
  const { nodes, onNodesChange, addDraft, dropDraft, draftWritten } =
    useBoardNodes(view, askedAt, move, onMoveFailed, deleteNotes);
  const edges = useMemo(() => toEdges(view), [view]);
  const { screenToFlowPosition } = useReactFlow();
  const surface = useRef<HTMLDivElement>(null);
  const mouse = useRef<XYPosition | null>(null);

  const addAtTheMiddle = useCallback(() => {
    const area = surface.current?.getBoundingClientRect();
    if (area !== undefined) {
      addDraft(
        screenToFlowPosition({
          x: area.left + area.width / 2,
          y: area.top + area.height / 2,
        }),
      );
    }
  }, [addDraft, screenToFlowPosition]);

  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      if (
        event.key.toLowerCase() !== "n" ||
        event.repeat ||
        event.ctrlKey ||
        event.metaKey ||
        event.altKey ||
        event.shiftKey ||
        isTyping(event.target)
      ) {
        return;
      }
      // Otherwise the letter lands in the text field that opens.
      event.preventDefault();
      if (mouse.current === null) {
        addAtTheMiddle();
      } else {
        addDraft(screenToFlowPosition(mouse.current));
      }
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [addAtTheMiddle, addDraft, screenToFlowPosition]);

  const isRestOfOpeningPress = useContext(RestOfOpeningPress);
  const onDoubleClick = useCallback(
    (event: MouseEvent<HTMLDivElement>) => {
      // The rest of the press that opened this board starts no note on it.
      if (
        event.target instanceof Element &&
        event.target.classList.contains("react-flow__pane") &&
        !isRestOfOpeningPress()
      ) {
        addDraft(screenToFlowPosition({ x: event.clientX, y: event.clientY }));
      }
    },
    [addDraft, screenToFlowPosition, isRestOfOpeningPress],
  );

  const openFirstInnerBoard = useCallback<NodeMouseHandler<ElementNode>>(
    (_event, node) => {
      const [first] = node.data.innerBoards;
      // On a note, a double-click opens its text.
      if (first !== undefined && node.data.shown.kind !== "note") {
        onOpen(first.id);
      }
    },
    [onOpen],
  );

  const newNotes = useMemo(
    () => ({
      create: async (draft: string, position: XYPosition, parts: NoteParts) => {
        const created = await createNote(position, parts);
        if (created !== null) {
          draftWritten(draft, created);
        }
        return created !== null;
      },
      drop: dropDraft,
    }),
    [createNote, draftWritten, dropDraft],
  );

  return (
    <OpenBoard.Provider value={onOpen}>
      <ReleasePin.Provider value={view.mode === "mirrored" ? null : release}>
        <WriteNote.Provider value={writeNote}>
          <NewNotes.Provider value={newNotes}>
            <div
              ref={surface}
              className="board-surface"
              onMouseMove={(event) => {
                mouse.current = { x: event.clientX, y: event.clientY };
              }}
              onMouseLeave={() => {
                mouse.current = null;
              }}
              onDoubleClick={onDoubleClick}
            >
              <ReactFlow
                nodes={nodes}
                edges={edges}
                nodeTypes={NODE_TYPES}
                onNodesChange={onNodesChange}
                onNodeDoubleClick={openFirstInnerBoard}
                nodesConnectable={false}
                deleteKeyCode={DELETE_KEYS}
                zoomOnDoubleClick={false}
                fitView
              >
                <Panel position="top-left">
                  <button type="button" onClick={addAtTheMiddle}>
                    New note
                  </button>
                </Panel>
              </ReactFlow>
            </div>
          </NewNotes.Provider>
        </WriteNote.Provider>
      </ReleasePin.Provider>
    </OpenBoard.Provider>
  );
}
