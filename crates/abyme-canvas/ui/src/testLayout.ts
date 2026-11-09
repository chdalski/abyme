import { vi } from "vitest";

/**
 * The size an element's inline style gives it in pixels, or 0: jsdom lays
 * nothing out, and React Flow gives each box its size as an inline style.
 */
function styledSize(element: HTMLElement, side: "width" | "height"): number {
  const value = element.style[side];
  return value.endsWith("px") ? Number.parseFloat(value) : 0;
}

/** Reports each observed element once, at its styled size, as a browser would. */
class ReportingResizeObserver {
  readonly #callback: ResizeObserverCallback;

  constructor(callback: ResizeObserverCallback) {
    this.#callback = callback;
  }

  observe(target: Element): void {
    setTimeout(() => {
      if (target instanceof HTMLElement && target.isConnected) {
        const entry = {
          target,
          contentRect: {
            width: styledSize(target, "width"),
            height: styledSize(target, "height"),
          },
        } as unknown as ResizeObserverEntry;
        this.#callback([entry], this as unknown as ResizeObserver);
      }
    }, 0);
  }

  unobserve(): void {}
  disconnect(): void {}
}

/** The one part of a transform React Flow reads when it measures a box: its scale. */
class ScaleOnlyMatrix {
  readonly m22: number;

  constructor(transform?: string) {
    const scale = /scale\(([^)]+)\)/.exec(transform ?? "")?.[1];
    this.m22 = scale === undefined ? 1 : Number(scale);
  }
}

/**
 * Lets React Flow measure the boxes in jsdom, so that it fits a board into
 * view: each box measures at the size it is given, and the pane, which has no
 * size of its own here, at React Flow's stand-in of 500 × 500. Returns the
 * undo. A test that needs the measurement at a moment it chooses uses the
 * on-demand `MeasuringObserver` in `BoardCanvas.test.tsx` instead.
 */
export function measureBoxes(): () => void {
  vi.stubGlobal("ResizeObserver", ReportingResizeObserver);
  vi.stubGlobal("DOMMatrixReadOnly", ScaleOnlyMatrix);
  const width = vi
    .spyOn(HTMLElement.prototype, "offsetWidth", "get")
    .mockImplementation(function (this: HTMLElement) {
      return styledSize(this, "width");
    });
  const height = vi
    .spyOn(HTMLElement.prototype, "offsetHeight", "get")
    .mockImplementation(function (this: HTMLElement) {
      return styledSize(this, "height");
    });
  return () => {
    width.mockRestore();
    height.mockRestore();
    vi.unstubAllGlobals();
  };
}
