# Backlog

Open findings and cleanup wishes that no plan holds yet, one
line each: what, where, and where it came from. The lead adds
entries and names them in the completion report. An entry goes
when it becomes a plan or the user drops it.

This is no status log and no home for facts. A missed plan
target goes here only after the user has lowered the target.

## Open
- `apply` requests accept `RelationKind` in the object form (`"kind": {"uses": null}`), unlike `Mode` and `StatementKind`; files read only the string and get the normal form, so this is about consistency: `crates/abyme-model/src/relation.rs` (security engineer, plan D Task 1, rated Low; fix with a later change there).
- Consolidate the test files that grew across plans (`crates/abyme-core/src/apply/tests.rs`, `crates/abyme-core/src/outline/tests.rs`, `crates/abyme-mcp/tests/server.rs`) in a plan of its own before the dogfooding plan (plan reviewer, plan D; the user chose a later plan).
- A full `cargo test --workspace` once ended the test process `element_properties` with SIGKILL; it did not recur alone or in later full runs, cause unknown: `crates/abyme-model/tests/element_properties.rs` (plan D Task 4).
- Remaining MVP plans, in this order: a statement kind on the element, starting with a refactor of the positional dispatch of `update_element` in `crates/abyme-core/src/apply.rs`, which is at clippy's argument limit; the canvas for the MVP (arrows, breadcrumb, review flags, pinning, sticky notes, maybe two plans); dogfooding on Abyme and the rlsp replay (MVP roadmap the user approved; plan D's Non-Goals).
- Move `toml` to 1.x in a small plan of its own; it parses repository files, so Security-Hybrid: root `Cargo.toml` (user decision while building the skeleton).
- Make `RECIPE_VERSION` and maybe `ProjectKey` crate-private if nothing outside needs them: `crates/abyme-index/src/lib.rs` (reviewer, search skeleton).
- `below` checks only the bottom edge, not the right, though `place` promises `None` when no position fits in `i32`; not reachable while the width stays 180: `crates/abyme-core/src/layout/free_space.rs` (Reviewer, auto-layout Task 3)
- `#[derive(Clone, Copy)]` on `Cell` looks unused, and `raise_bottom` has one caller and could stand inline: `crates/abyme-core/src/layout/free_space.rs` (Reviewer, auto-layout Task 3)
- The doc comment of `draft_board` is wrapped unevenly: `crates/abyme-core/src/apply.rs` (Reviewer, auto-layout Task 3)
- The arrow-key test asserts element, x, and y but not `pinned`; `expect(move?.pinned).toBe(true)` would make it explicit: `crates/abyme-canvas/ui/src/main.test.tsx` (Reviewer, auto-layout Task 4)
- "Boxes that lay on a board before pins existed are not pinned." reads as history; say "Only such a move pins a box; a box placed any other way is not pinned.": `docs/4 AI Interface/Placement and Layout.md`, Rules (Reviewer, auto-layout Task 4)
- `lines.min(limit)` at the end of `wrapped_lines` changes nothing, since the loop stops at the limit and raises `lines` by at most one per step; it stays as a guard of `MAX_LINES - lines` until someone wants it gone: `crates/abyme-core/src/layout/box_size.rs` (Reviewer, auto-layout Task 5)
- `place_element`'s `UnknownElement` for a missing element cannot be reached, since `resolve_element` looked it up before, so `place_refuses_an_element_the_request_deleted` covers that lookup and not this branch: `crates/abyme-core/src/apply.rs` (Reviewer, auto-layout Task 5)
