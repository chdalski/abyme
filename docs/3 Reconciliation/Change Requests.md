---
tags:
  - reconciliation
aliases:
  - Change Request
  - marked for deletion
---
# Change Requests

A change request arises when someone changes something in a synced zone that has to be implemented in the code. The flow by example: a crate is deleted on the board.

```mermaid
sequenceDiagram
    actor D as Developer
    participant C as Canvas
    participant K as Core
    participant A as AI via MCP
    participant R as Repo

    D->>C: deletes representation “legacy-mailer”
    C->>K: change set
    K-->>C: “marked for deletion”
    Note over C: The current state stays honest:<br>struck through instead of gone
    A->>K: next call
    K-->>A: Pending: delete legacy-mailer.<br>order-service depends on it.
    A->>D: Shall I delete the crate?
    alt yes
        D->>A: yes
        A->>R: deletes the crate, adjusts order-service
        K->>R: git diff at anchors
        K-->>C: element really deleted
    else no
        D->>A: no
        A->>K: discard change request
        K-->>C: element restored
    end
```

1. The element does not disappear right away but is shown as **“marked for deletion”**, for example struck through. The current state stays honest, because the crate still exists in the code.
2. On the next call, the MCP server tells the AI: “Pending: delete legacy-mailer. order-service depends on it.” The AI asks.
3. If the answer is yes, the AI changes the code. Reconciliation notices it, and the element is really deleted. If it is no, the element is restored.

Change requests are carried out by the AI. Later, a [[Extensions#Fetchers|fetcher]] can also carry them out directly, for example change a ticket status through an API.

> [!todo] Open
> - How are “marked for deletion” and other pending changes shown?
> - In which format are change requests stored?
