---
id: "abyme-mcp"
type: "c4/container"
name: "abyme-mcp"
summary: "The binary that serves Abyme's tools to the AI agent over MCP on stdio and initializes a project"
anchor: "crates/abyme-mcp"
verified: "b77e8a0729c297cd71bd4452a24da34a88d17262"
verified_by: "ai"
verified_content:
  - {path: "crates/abyme-core", object: "0a469e1ed2190def5e59b153a1e2f8860ddf2570"}
  - {path: "crates/abyme-index", object: "d66ee4eb756a692cd1fcb85506f1081b42df7b79"}
  - {path: "crates/abyme-mcp", object: "95f1e01bb8bb9df9fd3f5b177e48d892d4254404"}
  - {path: "crates/abyme-model", object: "bd2383395459ab71952a8102b5d45d5ef66c8360"}
relations:
  - {kind: "uses", to: "abyme-core"}
  - {kind: "uses", to: "abyme-index"}
  - {kind: "uses", to: "abyme-model"}
---
