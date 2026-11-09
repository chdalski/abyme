---
id: "abyme-index"
type: "c4/container"
name: "abyme-index"
summary: "The search index: ranks a project's elements by full text and vectors and downloads the embedding model"
anchor: "crates/abyme-index"
verified: "15c6cd545e757b1e81005cfedb2f46254b8ca324"
verified_by: "ai"
verified_content:
  - {path: "crates/abyme-core", object: "0a469e1ed2190def5e59b153a1e2f8860ddf2570"}
  - {path: "crates/abyme-index", object: "d66ee4eb756a692cd1fcb85506f1081b42df7b79"}
  - {path: "crates/abyme-model", object: "5cf0581e4238a5f560438a2d02299a087f494b2f"}
relations:
  - {kind: "uses", to: "abyme-core"}
  - {kind: "uses", to: "abyme-model"}
  - {kind: "uses", to: "hugging-face-hub"}
  - {kind: "uses", to: "search-cache"}
---
