---
id: "abyme-index"
type: "c4/container"
name: "abyme-index"
summary: "The search index: ranks a project's elements by full text and vectors and downloads the embedding model"
anchor: "crates/abyme-index"
verified: "cfc1f45b67b2f397db00375949736a58bdea9fd4"
verified_by: "ai"
verified_content:
  - {path: "crates/abyme-core", object: "cc8525a2ec06c04679f741a23fb77ac52651b0c8"}
  - {path: "crates/abyme-index", object: "d66ee4eb756a692cd1fcb85506f1081b42df7b79"}
  - {path: "crates/abyme-model", object: "bd2383395459ab71952a8102b5d45d5ef66c8360"}
relations:
  - {kind: "uses", to: "abyme-core"}
  - {kind: "uses", to: "abyme-model"}
  - {kind: "uses", to: "hugging-face-hub"}
  - {kind: "uses", to: "search-cache"}
---
