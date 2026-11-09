import {
  type KeyboardEvent,
  type MouseEvent,
  type ReactNode,
  useEffect,
  useRef,
  useState,
} from "react";

import {
  COLLAPSED,
  COLLAPSED_KEY,
  DEFAULT_WIDTH,
  MAX_WIDTH,
  MIN_WIDTH,
  WIDTH_KEY,
  clampWidth,
  readCollapsed,
  readWidth,
  store,
  storedValue,
} from "./sidebarState";

/** How far an arrow key moves the handle, in rem. */
const KEY_STEP = 1;

/** The size of a rem in pixels, to turn a drag into rem; 16 where none is known. */
function remInPixels(): number {
  const size = Number.parseFloat(
    getComputedStyle(document.documentElement).fontSize,
  );
  return Number.isFinite(size) && size > 0 ? size : 16;
}

/**
 * The sidebar with the list of boards: a handle on its edge changes its width
 * by a drag or the arrow keys, a double-click on it sets the default width, and
 * a button collapses it so the board takes the whole width. Width and state
 * stay in the webview's storage for the next start. Collapsed, its content
 * stays mounted, only hidden, so the list keeps what the user opened.
 */
export function Sidebar({ children }: { children: ReactNode }) {
  const [width, setWidth] = useState(() => readWidth(storedValue(WIDTH_KEY)));
  const [collapsed, setCollapsed] = useState(() =>
    readCollapsed(storedValue(COLLAPSED_KEY)),
  );
  // The button that took the click goes with the toggle, so the focus moves to
  // the one that undoes it; not when the sidebar first shows.
  const collapseButton = useRef<HTMLButtonElement>(null);
  const expandButton = useRef<HTMLButtonElement>(null);
  const toggled = useRef(false);
  useEffect(() => {
    if (toggled.current) {
      toggled.current = false;
      (collapsed ? expandButton : collapseButton).current?.focus();
    }
  }, [collapsed]);
  // Ends a drag going on: its listeners go on release, on leaving the window,
  // and when the sidebar goes.
  const endDrag = useRef<(() => void) | null>(null);
  useEffect(() => () => endDrag.current?.(), []);

  const resize = (next: number) => {
    const clamped = clampWidth(next);
    setWidth(clamped);
    store(WIDTH_KEY, String(clamped));
  };

  const collapse = (next: boolean) => {
    toggled.current = true;
    setCollapsed(next);
    store(COLLAPSED_KEY, next ? COLLAPSED : "");
  };

  const startDrag = (event: MouseEvent<HTMLDivElement>) => {
    if (event.button !== 0) {
      return;
    }
    // Keeps the drag from selecting text on its way.
    event.preventDefault();
    endDrag.current?.();
    const startX = event.clientX;
    const startWidth = width;
    const rem = remInPixels();
    let latest = startWidth;
    const move = (moved: globalThis.MouseEvent) => {
      latest = clampWidth(startWidth + (moved.clientX - startX) / rem);
      setWidth(latest);
    };
    const end = () => {
      window.removeEventListener("mousemove", move);
      window.removeEventListener("mouseup", end);
      window.removeEventListener("blur", end);
      endDrag.current = null;
      store(WIDTH_KEY, String(latest));
    };
    window.addEventListener("mousemove", move);
    window.addEventListener("mouseup", end);
    window.addEventListener("blur", end);
    endDrag.current = end;
  };

  const onHandleKey = (event: KeyboardEvent<HTMLDivElement>) => {
    if (event.key === "ArrowLeft") {
      event.preventDefault();
      resize(width - KEY_STEP);
    } else if (event.key === "ArrowRight") {
      event.preventDefault();
      resize(width + KEY_STEP);
    }
  };

  return (
    <div className="sidebar">
      {collapsed && (
        <button
          ref={expandButton}
          type="button"
          className="sidebar-expand"
          aria-label="Expand sidebar"
          aria-expanded={false}
          title="Expand sidebar"
          onClick={() => collapse(false)}
        >
          »
        </button>
      )}
      <nav hidden={collapsed} style={{ width: `${width}rem` }}>
        <div className="sidebar-header">
          <h1>Boards</h1>
          <button
            ref={collapseButton}
            type="button"
            className="sidebar-collapse"
            aria-label="Collapse sidebar"
            aria-expanded={true}
            title="Collapse sidebar"
            onClick={() => collapse(true)}
          >
            «
          </button>
        </div>
        {children}
      </nav>
      {!collapsed && (
        <div
          role="separator"
          aria-orientation="vertical"
          aria-label="Width of the sidebar"
          aria-valuenow={width}
          aria-valuemin={MIN_WIDTH}
          aria-valuemax={MAX_WIDTH}
          tabIndex={0}
          className="sidebar-handle"
          onMouseDown={startDrag}
          onDoubleClick={() => resize(DEFAULT_WIDTH)}
          onKeyDown={onHandleKey}
        />
      )}
    </div>
  );
}
