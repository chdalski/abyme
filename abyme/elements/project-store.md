---
id: "project-store"
type: "c4/component"
name: "Project store"
summary: "The project on disk: finds and initializes it, reads and writes its untrusted files safely, and asks Git what changed"
anchor: "crates/abyme-core/src/project"
verified: "7dc4b5fc42bfc07e388de17a63bd083e747d495a"
verified_by: "ai"
relations:
  - {kind: "uses", to: "apply"}
  - {kind: "uses", to: "operations"}
  - {kind: "uses", to: "review-flags"}
---
