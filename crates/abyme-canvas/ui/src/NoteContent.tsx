import type { XYPosition } from "@xyflow/react";
import { createContext, useContext, useEffect, useRef, useState } from "react";

import { type NoteParts, noteText, splitNoteText } from "./notes";

/**
 * Writes a note's text and says whether that worked, or `null` where nothing
 * can be written.
 */
export const WriteNote = createContext<
  ((element: string, parts: NoteParts) => Promise<boolean>) | null
>(null);

/**
 * What becomes of a new note: writing it at its position, which says whether that
 * worked, or dropping it.
 */
export const NewNotes = createContext<{
  create: (
    draft: string,
    position: XYPosition,
    parts: NoteParts,
  ) => Promise<boolean>;
  drop: (draft: string) => void;
} | null>(null);

/** The rows a text field needs for `text`, at least three. */
function rowsFor(text: string): number {
  return Math.max(3, text.split("\n").length);
}

/**
 * The text field of a note, which holds what is typed itself, so that a reload
 * of the board leaves it alone. `Esc` and a click next to it end the input, but
 * switching to another window does not. It stays open while the write is on its
 * way, read only, and when that fails. It neither starts a drag nor moves the
 * box with the arrow keys, and the mouse wheel scrolls it, not the canvas.
 */
function NoteField({
  element,
  text,
  write,
  onEnd,
}: {
  element: string;
  text: string;
  write: (element: string, parts: NoteParts) => Promise<boolean>;
  onEnd: (written: string | null) => void;
}) {
  const field = useRef<HTMLTextAreaElement>(null);
  // What the field opens with, whatever a reload brings while it is open.
  const [opening] = useState(text);
  const opened = useRef<NoteParts | null>(null);
  const writing = useRef(false);
  const [readOnly, setReadOnly] = useState(false);

  useEffect(() => {
    if (field.current !== null) {
      // The field's own value, since a text area normalizes line breaks.
      opened.current = splitNoteText(field.current.value);
      field.current.focus();
    }
  }, []);

  const end = (value: string) => {
    if (writing.current) {
      return;
    }
    const parts = splitNoteText(value);
    if (
      parts === null ||
      (parts.name === opened.current?.name &&
        parts.body === opened.current.body)
    ) {
      onEnd(null);
      return;
    }
    writing.current = true;
    // What is typed from here on would not be written.
    setReadOnly(true);
    void write(element, parts).then((written) => {
      writing.current = false;
      setReadOnly(false);
      if (written) {
        onEnd(noteText(parts.name, parts.body));
      }
    });
  };

  return (
    <div className="element element-note">
      <textarea
        ref={field}
        className="element-note-field nodrag nopan nokey nowheel"
        aria-label="Text of the note"
        defaultValue={opening}
        readOnly={readOnly}
        rows={rowsFor(opening)}
        onInput={(event) => {
          event.currentTarget.rows = rowsFor(event.currentTarget.value);
        }}
        onKeyDown={(event) => {
          if (event.key === "Escape") {
            event.preventDefault();
            end(event.currentTarget.value);
          }
        }}
        onBlur={(event) => {
          if (document.hasFocus()) {
            end(event.currentTarget.value);
          }
        }}
      />
    </div>
  );
}

/**
 * A sticky note's whole text, as text only, which a double-click opens for
 * editing. A written text stays until the board's next view brings a new one. A
 * new note, at `draftAt`, opens by itself, and once written it writes no more:
 * it only waits for the view that lists it.
 */
export function NoteContent({
  element,
  name,
  body,
  draftAt,
}: {
  element: string;
  name: string;
  body: string;
  draftAt?: XYPosition | undefined;
}) {
  const update = useContext(WriteNote);
  const newNotes = useContext(NewNotes);
  const write =
    draftAt === undefined
      ? update
      : newNotes &&
        ((id: string, parts: NoteParts) => newNotes.create(id, draftAt, parts));
  const [editing, setEditing] = useState(draftAt !== undefined);
  const fromFile = noteText(name, body);
  const [seen, setSeen] = useState(fromFile);
  const [written, setWritten] = useState<string | null>(null);
  if (fromFile !== seen) {
    setSeen(fromFile);
    setWritten(null);
  }
  const text = written ?? fromFile;

  if (editing && write !== null) {
    return (
      <NoteField
        element={element}
        text={text}
        write={write}
        onEnd={(ended) => {
          setEditing(false);
          if (ended !== null) {
            setWritten(ended);
          } else if (draftAt !== undefined) {
            newNotes?.drop(element);
          }
        }}
      />
    );
  }
  return (
    <div
      className="element element-note"
      onDoubleClick={draftAt === undefined ? () => setEditing(true) : undefined}
    >
      {text}
    </div>
  );
}
