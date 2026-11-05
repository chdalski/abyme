---
id: "abyme-mcp"
type: "c4/container"
name: "abyme-mcp"
summary: "The binary that serves Abyme's tools to the AI agent over MCP on stdio and initializes a project"
anchor: "crates/abyme-mcp"
verified: "6dacb8c2fd3633fb2c68bbf3b5b9bb809b612333"
verified_by: "ai"
verified_content:
  - {path: "crates/abyme-core", object: "7c347842b27ad8cba44ca12166a938964e6397a0"}
  - {path: "crates/abyme-index", object: "d66ee4eb756a692cd1fcb85506f1081b42df7b79"}
  - {path: "crates/abyme-mcp", object: "a319864a9628c553dbbb61550b11731748a68982"}
  - {path: "crates/abyme-model", object: "bd2383395459ab71952a8102b5d45d5ef66c8360"}
relations:
  - {kind: "uses", to: "abyme-core"}
  - {kind: "uses", to: "abyme-index"}
  - {kind: "uses", to: "abyme-model"}
---
