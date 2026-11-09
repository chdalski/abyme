---
id: "canvas-commands"
type: "c4/component"
name: "Canvas commands"
summary: "The Rust side of the IPC boundary: the commands list_boards, open_board, apply, and review_flags, the window, and the project-changed event"
anchor: "crates/abyme-canvas/src/app.rs"
verified: "032c51a82c809c558fc3c0b576a62f150778cf27"
verified_by: "ai"
verified_content:
  - {path: "crates/abyme-canvas/src/admission.rs", object: "7634e1b108a4c5b27810e5199345bee9c8243771"}
  - {path: "crates/abyme-canvas/src/app.rs", object: "04c3df1e0bd91d79b3fd1133650141dd4fbfab69"}
  - {path: "crates/abyme-canvas/src/projection.rs", object: "5b33aa1360549c1713713527f29150560a150bec"}
  - {path: "crates/abyme-canvas/src/watcher.rs", object: "a8badd25c98381dec02941177aa8c95d28eab210"}
relations:
  - {kind: "uses", to: "admission"}
  - {kind: "uses", to: "file-watcher"}
  - {kind: "uses", to: "projection"}
---
