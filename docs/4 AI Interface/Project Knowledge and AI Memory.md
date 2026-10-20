---
tags:
  - ai
aliases:
  - AI Memory
  - Hooks
---
# Project Knowledge and AI Memory

## The Rule

- **Abyme holds the project knowledge:** what is, what is to be, and why.
- **The AI's own memory holds the way of working:** how it should work with this user, that is preferences and behavior. That has nothing to do with project content.

## How the AI Follows It

1. **Instructions from the MCP server.** MCP servers can supply instructions that the client takes into the system prompt. They state the general part: what Abyme is for and when to use it.
2. **An example section for the CLAUDE.md**, shipped with Abyme:

   ```markdown
   ## Abyme
   Project knowledge lives in Abyme (MCP server `abyme`), not in docs/ or in your own memory.
   - Abyme: structure of the system (current), target pictures (target), decisions, open questions.
   - Your own memory: only the user's way of working and preferences, no project content.
   - Before a task: fetch the status and read the affected boards.
   - After code changes: work through the server's reports on deviations and dependencies.
   ```

3. **Hooks for the “promptly”**, for example after a commit or at the end of a task: trigger [[Reconciliation]]. A rule is only a request to the AI; a hook always runs.

Optionally there is a skill for recurring workflows, such as “create C4 for this repo” or “reconcile the current state”.
