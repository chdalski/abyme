# Backlog

Open findings and cleanup wishes that no plan holds yet, one
line each: what, where, and where it came from. The lead adds
entries and names them in the completion report. An entry goes
when it becomes a plan or the user drops it.

This is no status log and no home for facts. A missed plan
target goes here only after the user has lowered the target.

## Open
- `apply` requests accept `RelationKind` in the object form (`"kind": {"uses": null}`), unlike `Mode` and `StatementKind`; files read only the string and get the normal form, so this is about consistency: `crates/abyme-model/src/relation.rs` (security engineer, plan D Task 1, rated Low; fix with a later change there).
- Consolidate the test files that grew across plans (`crates/abyme-core/src/apply/tests.rs`, `crates/abyme-core/src/outline/tests.rs`, `crates/abyme-mcp/tests/server.rs`) in a plan of its own before the dogfooding plan, with a builder for test elements such as `ElementFixture::new("id").summary("s").build()` behind a test feature of `abyme-model`, replacing the `ElementFields` literals in the fixtures (plan reviewer, plan D; the user chose a later plan, and this builder in the statement kind plan).
- A full `cargo test --workspace` ended the test process `element_properties` twice, once with SIGKILL and once with the signal unconfirmed (sticky notes Task 1); it passed alone and in the next full runs, cause unknown, and the cgroup's `oom_kill` counter stands above zero: `crates/abyme-model/tests/element_properties.rs` (plan D Task 4).
- Remaining MVP plans, in this order: sticky notes on the canvas (the Non-Goal of the canvas display plan); dogfooding on Abyme and the rlsp replay (MVP roadmap the user approved; plan D's Non-Goals).
- Move `toml` to 1.x in a small plan of its own; it parses repository files, so Security-Hybrid: root `Cargo.toml` (user decision while building the skeleton).
- Make `RECIPE_VERSION` and maybe `ProjectKey` crate-private if nothing outside needs them: `crates/abyme-index/src/lib.rs` (reviewer, search skeleton).
- `below` checks only the bottom edge, not the right, though `place` promises `None` when no position fits in `i32`; not reachable while the width stays 180: `crates/abyme-core/src/layout/free_space.rs` (Reviewer, auto-layout Task 3)
- `#[derive(Clone, Copy)]` on `Cell` looks unused, and `raise_bottom` has one caller and could stand inline: `crates/abyme-core/src/layout/free_space.rs` (Reviewer, auto-layout Task 3)
- The doc comment of `draft_board` is wrapped unevenly: `crates/abyme-core/src/apply.rs` (Reviewer, auto-layout Task 3)
- The arrow-key test asserts element, x, and y but not `pinned`; `expect(move?.pinned).toBe(true)` would make it explicit: `crates/abyme-canvas/ui/src/main.test.tsx` (Reviewer, auto-layout Task 4)
- `lines.min(limit)` at the end of `wrapped_lines` changes nothing, since the loop stops at the limit and raises `lines` by at most one per step; it stays as a guard of `MAX_LINES - lines` until someone wants it gone: `crates/abyme-core/src/layout/box_size.rs` (Reviewer, auto-layout Task 5)
- `place_element`'s `UnknownElement` for a missing element cannot be reached, since `resolve_element` looked it up before, so `place_refuses_an_element_the_request_deleted` covers that lookup and not this branch: `crates/abyme-core/src/apply.rs` (Reviewer, auto-layout Task 5)
- `the_anchor_of_those_requests_is_invalid_on_its_own` accepts any `InvalidElement`; `ElementProblem::InvalidAnchor { .. }` names its intent: `crates/abyme-core/src/apply/tests.rs` (Reviewer, statement kind Task 2)
- `STATEMENT_DETAIL_LIMIT` could be `SHOWN_LIMIT` like its neighbors in `anchor.rs`, `verification.rs`, and `id.rs`: `crates/abyme-model/src/element.rs` (Reviewer, statement kind Task 4)
- `Verified:` and the markers are said to show that content was checked, but a stamp holds only against the anchor, and a hand edit of `statement` keeps it; say "when it was last checked against its anchor": descriptions of `read_board` and `search` in `crates/abyme-mcp/src/server.rs`, `docs/4 AI Interface/MCP Server.md` (security engineer, statement kind Task 5, rated Low)
- In `BoardPane.test.tsx` the `./testBoards` import sits before `./nodes`, and `VIEW: BoardView = boardView(...)` there and in `nodes.test.ts` repeats the fixture's return type: `crates/abyme-canvas/ui/src/` (Reviewer, canvas display Task 1)
- The doc of `outline` repeats the rule of `relations_on` in prose and could point to it: `crates/abyme-core/src/outline.rs` (Reviewer, canvas display Task 2)
- The return type `{ label: string } | object` of `shownLabel` says nothing: `crates/abyme-canvas/ui/src/edges.ts` (Reviewer, canvas display Task 4)
- A node's `handles` are 1×1 while the DOM handles are 6×6, so an arrow's end may shift 2 to 3 pixels after a measure: `crates/abyme-canvas/ui/src/nodes.ts` (Reviewer, canvas display Task 4)
- A real double click on `▸` opens the board on the first click, and the rest of it can land on the new view and go one level deeper; nothing is written, and it is untested in a real webview: `crates/abyme-canvas/ui/src/ElementBox.tsx` (Reviewer, canvas display Task 5)
- The `▸` button and the double click are tested only on a free board; a case with `mirrored` would pin them there too: `crates/abyme-canvas/ui/src/BoardCanvas.test.tsx` (Reviewer, canvas display Task 5)
- A pinned box whose element file is missing shows the release button, and the core refuses the release with `no element has the ID ...`, as it refuses a drag; a plain marker there would promise nothing. `crates/abyme-canvas/ui/src/ElementBox.tsx`, `PinMarker`. From canvas display, Task 7.
- The clause "it authorizes nothing" in the `apply` paragraph of `docs/5 Architecture/Canvas.md` is reasoning no reader acts on, and could go. From canvas display, Task 7, security engineer.
- A test fails now and then, cause unknown: `crates/abyme-canvas/ui/src/main.test.tsx`, "shows the boards, opens one with its boxes, and moves a box", which presses ArrowRight right after Enter; waiting for the box's `selected` class may fix it, untested (Reviewer, canvas display Task 8).
- On a box narrower than about 110 px (estimated from the CSS), the review mark paints over part of the pin's marker: `crates/abyme-canvas/ui/src/canvas.css` (Reviewer, canvas display Task 8).
- Git runs without a timeout, so a hung `git` binds a blocking thread: the canvas stays at "Checking review flags…" on a first run, and old marks stay without a note on a later one. Its buffered output has no limit either: `crates/abyme-core/src/project/git.rs` (security engineer, canvas display Task 8).
- A window that turns visible without getting focus, as on a workspace switch, asks only at the next tick, so its marks can be older than the "at most one interval behind" of Canvas until then; a `visibilitychange` listener in `whileVisible` would close it: `crates/abyme-canvas/ui/src/flags.ts` (Reviewer, canvas display Task 9).
- The canvas row of `CLAUDE.md` does not name that flags are also asked for on focus and every 30 s while visible (Reviewer, canvas display Task 9).
- The canvas display has not run in a real WebKitGTK window: arrows, breadcrumb, `▸` buttons, the pin's marker and its click, review marks, and `visibilityState` when minimized are checked only in jsdom and from CSS values; for the user (canvas display).
