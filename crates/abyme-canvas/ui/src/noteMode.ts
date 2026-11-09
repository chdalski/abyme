import type {
  MouseEvent,
  PointerEvent,
  SyntheticEvent,
  TouchEvent,
} from "react";
import { useCallback, useRef, useState } from "react";

/**
 * Whether `target` lies on the board itself, its empty pane or one of its
 * boxes, where a press in note mode places a note; the toolbar and the zoom
 * bar lie elsewhere and keep working.
 */
function onTheBoard(target: EventTarget | null): boolean {
  return (
    target instanceof Element &&
    target.closest(".react-flow__pane, .react-flow__node") !== null
  );
}

/**
 * Stops a press or click that note mode took before React Flow or a box sees
 * it, and keeps the browser from acting on it, as by moving the focus.
 */
function swallow(event: SyntheticEvent): void {
  event.preventDefault();
  event.stopPropagation();
}

/**
 * Note mode: switched on, the next primary press on the board calls `place`
 * with the point pressed and ends the mode. That press, and the further clicks
 * of a double click it starts, reach neither React Flow nor a box, so they
 * move, select, pan, open, and release nothing. A press of another button
 * places nothing and keeps the mode on.
 */
export function useNoteMode(place: (point: { x: number; y: number }) => void) {
  const [placing, setPlacing] = useState(false);
  // Read by the handlers, which see the mode as of the last render otherwise.
  const on = useRef(false);
  // The press that placed a note, until its own mousedown or the next press.
  const pressed = useRef(false);
  // That press's clicks, and those of a double click it starts, until the
  // mousedown of a press that starts no double click.
  const swallowing = useRef(false);

  const set = useCallback((next: boolean) => {
    on.current = next;
    setPlacing(next);
  }, []);

  const onPointerDownCapture = useCallback(
    (event: PointerEvent) => {
      if (on.current && event.button === 0 && onTheBoard(event.target)) {
        // Stopped, not canceled: the browser sends a canceled pointerdown's
        // mousedown no more, and that mousedown is where this press ends.
        event.stopPropagation();
        pressed.current = true;
        swallowing.current = true;
        set(false);
        place({ x: event.clientX, y: event.clientY });
        return;
      }
      // A new press: the mousedown of the press that placed a note has come,
      // or comes no more.
      pressed.current = false;
    },
    [place, set],
  );

  const onMouseDownCapture = useCallback((event: MouseEvent) => {
    if (pressed.current) {
      // The mousedown of the press whose pointerdown placed the note.
      pressed.current = false;
      swallow(event);
      return;
    }
    if (swallowing.current && event.detail >= 2) {
      swallow(event);
      return;
    }
    swallowing.current = false;
  }, []);

  const onTouchStartCapture = useCallback((event: TouchEvent) => {
    // The touch of the press that placed a note follows its pointerdown.
    if (pressed.current || on.current) {
      if (onTheBoard(event.target)) {
        event.stopPropagation();
      }
    }
  }, []);

  const onClickCapture = useCallback((event: MouseEvent) => {
    // A click from the keyboard comes with no count and from no press.
    if (swallowing.current && event.detail >= 1) {
      swallow(event);
    }
  }, []);

  return {
    placing,
    toggle: useCallback(() => set(!on.current), [set]),
    end: useCallback(() => set(false), [set]),
    handlers: {
      onPointerDownCapture,
      onMouseDownCapture,
      onTouchStartCapture,
      onClickCapture,
      onDoubleClickCapture: onClickCapture,
    },
  };
}
