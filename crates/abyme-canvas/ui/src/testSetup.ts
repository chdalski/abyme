// jsdom has no layout, so React Flow's size observer gets a stand-in that
// never reports; the boxes keep the sizes from the board file.
class ResizeObserverStandIn {
  observe(): void {}
  unobserve(): void {}
  disconnect(): void {}
}

globalThis.ResizeObserver ??= ResizeObserverStandIn;
