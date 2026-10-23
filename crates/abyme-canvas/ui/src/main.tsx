import { StrictMode } from "react";
import { createRoot } from "react-dom/client";

import { App } from "./App";
import { tauriClient } from "./client";

// Repository text reaches this page, so no code may add to the prototype of
// every object once that text can arrive. The freeze stands here, after the
// imports: d3-color, which React Flow loads, sets `constructor` on its own
// prototypes while it loads, which a prototype frozen before it would refuse.
// Repository data arrives only through IPC, after the first render.
Object.freeze(Object.prototype);

const root = document.getElementById("root");
if (root === null) {
  throw new Error("index.html has no element with the ID root");
}
createRoot(root).render(
  <StrictMode>
    <App client={tauriClient} />
  </StrictMode>,
);
