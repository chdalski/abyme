---
id: "abyme-index"
type: "c4/container"
name: "abyme-index"
summary: "The search index: ranks a project's elements by full text and vectors and downloads the embedding model"
anchor: "crates/abyme-index"
verified: "f3839b091e351b888ac78986575f938be3808745"
verified_by: "ai"
verified_content:
  - {path: "crates/abyme-core", object: "e595fefaca1dea21c4248f170488ff379decf700"}
  - {path: "crates/abyme-index", object: "d66ee4eb756a692cd1fcb85506f1081b42df7b79"}
  - {path: "crates/abyme-model", object: "bd2383395459ab71952a8102b5d45d5ef66c8360"}
relations:
  - {kind: "uses", to: "abyme-core"}
  - {kind: "uses", to: "abyme-model"}
  - {kind: "uses", to: "hugging-face-hub"}
  - {kind: "uses", to: "search-cache"}
---
