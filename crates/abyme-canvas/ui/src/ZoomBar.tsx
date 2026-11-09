import { Panel, useReactFlow, useStore } from "@xyflow/react";
import { useEffect } from "react";

import { isTyping } from "./typing";

/** How far out the canvas zooms: far enough that fitting shows a large board. */
export const MIN_ZOOM = 0.1;

/** How far in the canvas zooms: React Flow's own maximum. */
export const MAX_ZOOM = 2;

/** The keys that zoom, by the character they type. */
const ZOOM_KEYS = ["+", "-", "0"] as const;

type ZoomKey = (typeof ZOOM_KEYS)[number];

function isZoomKey(key: string): key is ZoomKey {
  return (ZOOM_KEYS as readonly string[]).includes(key);
}

/**
 * The zoom bar at the bottom right of the board: zoom out, the zoom as a whole
 * percentage, which sets 100 % when clicked, zoom in, and fitting the board
 * into view. The keys `+`, `-`, and `0` zoom in, zoom out, and fit, unless the
 * user is typing or holds Ctrl, Meta, or Alt, which belong to the webview's own
 * zoom and to other shortcuts. Shift is allowed, since some keyboards need it
 * to type `+`.
 */
export function ZoomBar() {
  const { zoomIn, zoomOut, zoomTo, fitView } = useReactFlow();
  const zoom = useStore((state) => state.transform[2]);

  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      if (
        !isZoomKey(event.key) ||
        event.ctrlKey ||
        event.metaKey ||
        event.altKey ||
        isTyping(event.target)
      ) {
        return;
      }
      event.preventDefault();
      switch (event.key) {
        case "+":
          void zoomIn();
          return;
        case "-":
          void zoomOut();
          return;
        case "0":
          void fitView();
          return;
      }
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [zoomIn, zoomOut, fitView]);

  return (
    <Panel position="bottom-right" className="zoom-bar">
      <button
        type="button"
        aria-label="Zoom out"
        title="Zoom out (-)"
        onClick={() => void zoomOut()}
      >
        −
      </button>
      <button
        type="button"
        className="zoom-level"
        title="Zoom to 100 %"
        onClick={() => void zoomTo(1)}
      >
        {Math.round(zoom * 100)} %
      </button>
      <button
        type="button"
        aria-label="Zoom in"
        title="Zoom in (+)"
        onClick={() => void zoomIn()}
      >
        +
      </button>
      <button
        type="button"
        aria-label="Fit the board into view"
        title="Fit the board into view (0)"
        onClick={() => void fitView()}
      >
        ⤢
      </button>
    </Panel>
  );
}
