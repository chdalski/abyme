---
id: "project-store"
type: "c4/component"
name: "Project store"
summary: "The project on disk: finds and initializes it, reads and writes its untrusted files safely, and asks Git what changed"
anchor: "crates/abyme-core/src/project"
verified: "b77e8a0729c297cd71bd4452a24da34a88d17262"
verified_by: "ai"
verified_content:
  - {path: "crates/abyme-core/src/apply.rs", object: "3c10e50ed7beb9b1cf685498296e6874ca98338c"}
  - {path: "crates/abyme-core/src/operation.rs", object: "87912b132f2386f40e7baefe191c10acf5358678"}
  - {path: "crates/abyme-core/src/project", object: "fd281295d7e1ee21a7271f37e80f5167e2a75657"}
  - {path: "crates/abyme-core/src/review.rs", object: "bcf92ecd999c1bf7b705ab39055c36d412be8df6"}
relations:
  - {kind: "uses", to: "apply"}
  - {kind: "uses", to: "operations"}
  - {kind: "uses", to: "review-flags"}
---
