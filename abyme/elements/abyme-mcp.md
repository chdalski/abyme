---
id: "abyme-mcp"
type: "c4/container"
name: "abyme-mcp"
summary: "The binary that serves Abyme's tools to the AI agent over MCP on stdio and initializes a project"
anchor: "crates/abyme-mcp"
verified: "15c6cd545e757b1e81005cfedb2f46254b8ca324"
verified_by: "ai"
verified_content:
  - {path: "crates/abyme-core", object: "0a469e1ed2190def5e59b153a1e2f8860ddf2570"}
  - {path: "crates/abyme-index", object: "d66ee4eb756a692cd1fcb85506f1081b42df7b79"}
  - {path: "crates/abyme-mcp", object: "d87d231f4811b6d2dd5327f22628e924bae17402"}
  - {path: "crates/abyme-model", object: "5cf0581e4238a5f560438a2d02299a087f494b2f"}
relations:
  - {kind: "uses", to: "abyme-core"}
  - {kind: "uses", to: "abyme-index"}
  - {kind: "uses", to: "abyme-model"}
---
