---
tags:
  - vision
---
# Guiding Principles

1. **The board is a view and a working medium, not the source of data.** Elements have an identity that does not depend on their position (see [[Elements]]).
2. **Honesty before completeness.** The current state shows only what is really so. What is pending, outdated, or unchecked is shown as such (see [[Change Requests]], [[Checks]]).
3. **The system knows where it lags behind.** Deviations are detected automatically and promptly, even when they are not fixed right away (see [[Reconciliation#Detecting Changes]]).
4. **The AI describes *what* it wants, and Abyme decides *where* it goes.** For that there are semantic operations and auto-layout instead of pixel arithmetic (see [[Placement and Layout]]).
5. **Open to anything that can be shown.** The AI interprets any content and brings it into the Abyme format. Extensions fill in where the AI alone is not enough (see [[Hourglass#Inputs]], [[Extensions]]).
6. **Keeping things current also means deleting.** History comes from Git, not from content that is simply left lying around (see [[Storage in Git]]).
7. **People and the AI are equals.** Both read and write with the same operations, the person through the [[Canvas]], the AI through the [[MCP Server]].
