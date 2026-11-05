---
id: "project-store"
type: "c4/component"
name: "Project store"
summary: "The project on disk: finds and initializes it, reads and writes its untrusted files safely, and asks Git what changed"
anchor: "crates/abyme-core/src/project"
verified: "f3839b091e351b888ac78986575f938be3808745"
verified_by: "ai"
verified_content:
  - {path: "crates/abyme-core/src/apply.rs", object: "3c10e50ed7beb9b1cf685498296e6874ca98338c"}
  - {path: "crates/abyme-core/src/operation.rs", object: "87912b132f2386f40e7baefe191c10acf5358678"}
  - {path: "crates/abyme-core/src/project", object: "75811c6c67454e8a78cc1ec02778d77e5d85c85a"}
  - {path: "crates/abyme-core/src/review.rs", object: "45a0a3e39ef461a1e3e5eac541fa7253236e2ee0"}
relations:
  - {kind: "uses", to: "apply"}
  - {kind: "uses", to: "operations"}
  - {kind: "uses", to: "review-flags"}
---
