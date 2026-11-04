---
id: "project-store"
type: "c4/component"
name: "Project store"
summary: "The project on disk: finds and initializes it, reads and writes its untrusted files safely, and asks Git what changed"
anchor: "crates/abyme-core/src/project"
verified: "00450034b74e156bae4e7ab277d8daf7b686be15"
verified_by: "ai"
relations:
  - {kind: "uses", to: "apply"}
  - {kind: "uses", to: "operations"}
  - {kind: "uses", to: "review-flags"}
---
