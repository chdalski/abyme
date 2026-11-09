/** The sidebar's width when nothing else is stored, in rem: its width before it could change. */
export const DEFAULT_WIDTH = 16;

/** The narrowest the sidebar gets, in rem, so titles stay readable. */
export const MIN_WIDTH = 10;

/** The widest the sidebar gets, in rem, so the board keeps most of the window. */
export const MAX_WIDTH = 40;

/** Where the webview's storage keeps the sidebar's width. */
export const WIDTH_KEY = "abyme.sidebar.width";

/** Where the webview's storage keeps whether the sidebar is collapsed. */
export const COLLAPSED_KEY = "abyme.sidebar.collapsed";

/** The one value that stores a collapsed sidebar. */
export const COLLAPSED = "collapsed";

/** `width` within the sidebar's bounds; anything but a finite number gives the default. */
export function clampWidth(width: number): number {
  return Number.isFinite(width)
    ? Math.min(MAX_WIDTH, Math.max(MIN_WIDTH, width))
    : DEFAULT_WIDTH;
}

/**
 * The width a stored value gives, read as untrusted text: a finite number,
 * clamped to the bounds; anything else, a blank value and none included, gives
 * the default.
 */
export function readWidth(stored: string | null): number {
  if (stored === null || stored.trim() === "") {
    return DEFAULT_WIDTH;
  }
  return clampWidth(Number(stored));
}

/** Whether a stored value collapses the sidebar: only the value the canvas writes. */
export function readCollapsed(stored: string | null): boolean {
  return stored === COLLAPSED;
}

/**
 * The value stored under `key`, or none when there is none or the storage
 * cannot be read, as where the webview allows none.
 */
export function storedValue(key: string): string | null {
  try {
    return window.localStorage.getItem(key);
  } catch {
    return null;
  }
}

/** Stores `value` under `key`; a storage that refuses it is left as it is. */
export function store(key: string, value: string): void {
  try {
    window.localStorage.setItem(key, value);
  } catch {
    // The sidebar keeps working; it only starts as before next time.
  }
}
