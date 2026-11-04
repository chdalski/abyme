---
id: "ai-agent"
type: "c4/system"
name: "AI agent"
summary: "An AI coding agent, such as Claude Code, that reads and changes the boards through Abyme's MCP server"
relations:
  - {kind: "uses", to: "abyme"}
  - {kind: "uses", to: "abyme-mcp"}
  - {kind: "uses", to: "project-repository"}
---
