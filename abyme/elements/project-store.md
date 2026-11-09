---
id: "project-store"
type: "c4/component"
name: "Project store"
summary: "The project on disk: finds and initializes it, reads and writes its untrusted files safely, and asks Git what changed"
anchor: "crates/abyme-core/src/project"
verified: "cfc1f45b67b2f397db00375949736a58bdea9fd4"
verified_by: "ai"
verified_content:
  - {path: "crates/abyme-core/src/apply.rs", object: "3c10e50ed7beb9b1cf685498296e6874ca98338c"}
  - {path: "crates/abyme-core/src/operation.rs", object: "87912b132f2386f40e7baefe191c10acf5358678"}
  - {path: "crates/abyme-core/src/project", object: "96ba70b3010de5616e3a94154146d80c35dda660"}
  - {path: "crates/abyme-core/src/review.rs", object: "c8c433c005fc6a38ac5d406af2ee86efbe47e37a"}
relations:
  - {kind: "uses", to: "apply"}
  - {kind: "uses", to: "operations"}
  - {kind: "uses", to: "review-flags"}
---
