---
id: "canvas-commands"
type: "c4/component"
name: "Canvas commands"
summary: "The Rust side of the IPC boundary: the commands list_boards, open_board, apply, and review_flags, the window, and the project-changed event"
anchor: "crates/abyme-canvas/src/app.rs"
verified: "15c6cd545e757b1e81005cfedb2f46254b8ca324"
verified_by: "ai"
verified_content:
  - {path: "crates/abyme-canvas/src/admission.rs", object: "028773adb63bee17f517ea4098ad5562cb5ccd0e"}
  - {path: "crates/abyme-canvas/src/app.rs", object: "8d9d63e798b9b2d2edd2fd68d47d20fb47a026db"}
  - {path: "crates/abyme-canvas/src/projection.rs", object: "c1a60fd73e758ebb7d95ce7f1c6ec1412919753c"}
  - {path: "crates/abyme-canvas/src/watcher.rs", object: "a8badd25c98381dec02941177aa8c95d28eab210"}
relations:
  - {kind: "uses", to: "admission"}
  - {kind: "uses", to: "file-watcher"}
  - {kind: "uses", to: "projection"}
---
