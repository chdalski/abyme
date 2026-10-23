import { readFileSync } from "node:fs";

import react from "@vitejs/plugin-react";
import { defineConfig } from "vitest/config";

/** The page's content security policy, from its meta element in index.html. */
function contentSecurityPolicy(): string {
  const page = readFileSync(new URL("index.html", import.meta.url), "utf8");
  const policy =
    /http-equiv="Content-Security-Policy"\s+content="([^"]+)"/.exec(page)?.[1];
  if (policy === undefined) {
    throw new Error("index.html has no Content-Security-Policy meta element");
  }
  return policy;
}

export default defineConfig({
  plugins: [react()],
  // Keeps the Tauri CLI's output on screen while it runs the dev server.
  clearScreen: false,
  server: {
    port: 5173,
    // The webview trusts the page at this port as the canvas's own; another
    // port must never serve the canvas in its place.
    strictPort: true,
    // As a header, the policy also covers the scripts Vite puts into the page
    // ahead of the meta element.
    headers: { "Content-Security-Policy": contentSecurityPolicy() },
  },
  test: {
    environment: "jsdom",
    setupFiles: ["src/testSetup.ts"],
  },
});
