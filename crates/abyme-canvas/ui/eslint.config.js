import js from "@eslint/js";
import { defineConfig } from "eslint/config";
import tseslint from "typescript-eslint";

export default defineConfig(
  { ignores: ["dist/"] },
  js.configs.recommended,
  tseslint.configs.strict,
  {
    rules: {
      // Text from the repository is shown as text only, never parsed as HTML or code.
      "no-restricted-syntax": [
        "error",
        {
          selector: "JSXAttribute[name.name='dangerouslySetInnerHTML']",
          message: "Render text as React text children, never as HTML.",
        },
      ],
      "no-restricted-properties": [
        "error",
        {
          property: "innerHTML",
          message: "Render text as React text children, never as HTML.",
        },
        {
          property: "outerHTML",
          message: "Render text as React text children, never as HTML.",
        },
        {
          property: "insertAdjacentHTML",
          message: "Render text as React text children, never as HTML.",
        },
        {
          object: "document",
          property: "write",
          message: "Never write markup into the page.",
        },
      ],
      "no-eval": "error",
      "no-implied-eval": "error",
      "no-new-func": "error",
    },
  },
);
