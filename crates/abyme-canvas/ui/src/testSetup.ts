// jsdom has no layout, so React Flow's size observer gets a stand-in that
// never reports; the boxes keep the sizes from the board file.
class ResizeObserverStandIn {
  observe(): void {}
  unobserve(): void {}
  disconnect(): void {}
}

globalThis.ResizeObserver ??= ResizeObserverStandIn;

// jsdom has no SVG layout either; React Flow asks it for the size of a label.
Object.defineProperty(SVGElement.prototype, "getBBox", {
  value: () => ({ x: 0, y: 0, width: 10, height: 10 }),
});
