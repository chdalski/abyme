---
id: "project-store"
type: "c4/component"
name: "Project store"
summary: "The project on disk: finds and initializes it, reads and writes its untrusted files safely, and asks Git what changed"
anchor: "crates/abyme-core/src/project"
verified: "6dacb8c2fd3633fb2c68bbf3b5b9bb809b612333"
verified_by: "ai"
verified_content:
  - {path: "crates/abyme-core/src/apply.rs", object: "3c10e50ed7beb9b1cf685498296e6874ca98338c"}
  - {path: "crates/abyme-core/src/operation.rs", object: "87912b132f2386f40e7baefe191c10acf5358678"}
  - {path: "crates/abyme-core/src/project", object: "9942cf10ad83a323c984d7196bb01f148922c870"}
  - {path: "crates/abyme-core/src/review.rs", object: "482860ff1bf190d818a8d898185f2aff048c6435"}
relations:
  - {kind: "uses", to: "apply"}
  - {kind: "uses", to: "operations"}
  - {kind: "uses", to: "review-flags"}
---
