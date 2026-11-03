# Backlog

Open findings and cleanup wishes that no plan holds yet, one
line each: what, where, and where it came from. The lead adds
entries and names them in the completion report. An entry goes
when it becomes a plan or the user drops it.

This is no status log and no home for facts. A missed plan
target goes here only after the user has lowered the target.

Each entry sits in the section for its kind. Related entries
share a parent line with one sub-item each.

## Security

- `Verified:` and the markers are said to show that content was checked, but a stamp holds only against the anchor, and a hand edit of `statement` keeps it; say "when it was last checked against its anchor": descriptions of `read_board` and `search` in `crates/abyme-mcp/src/server.rs`, `docs/4 AI Interface/MCP Server.md` (security engineer, statement kind Task 5, rated Low)
- Git runs without a timeout, so a hung `git` binds a blocking thread: the canvas stays at "Checking review flags…" on a first run, and old marks stay without a note on a later one. Its buffered output has no limit either: `crates/abyme-core/src/project/git.rs` (security engineer, canvas display Task 8).
- A NUL or another control character in a note's body stays possible over MCP and by hand, which hides the file from `git diff`; the canvas refuses it: `crates/abyme-core/src/apply.rs` (security engineer, sticky notes Task 5).
- While another process holds the project's lock, a note's write and the way to another board wait without a limit: `crates/abyme-core/src/project/store.rs`, `crates/abyme-canvas/ui/src/App.tsx` (security engineer, sticky notes Task 5).
- A note whose slug equals a relation's target without a file takes over that incoming relation, and then cannot be deleted while it holds; `taken_elements` holds no relation targets: `crates/abyme-core/src/apply.rs` (security engineer, sticky notes Task 6).
- `listed_id` ignores entries that are no regular file or have no valid ID as a name, so a symlink `note.md` counts as free and is replaced by the rename, and on a case-insensitive file system `Note.md` too: `crates/abyme-core/src/project/store.rs` (security engineer, sticky notes Task 6).
- When a box moves, a note whose attachment did not hold only because of the range can start to hold, and the request's free space does not count it where it now lies, so a later `place_element` of the request can overlap it; only a hand-edited board file gets there: `crates/abyme-core/src/apply.rs`, `move_element` (security engineer, attach notes Task 2, accepted Low).

## Bugs

- A board opened by Enter while a note write is pending marks no mouse double click in flight when the write answers, so that double click's rest can act on the new board; `opened()` could set `openedByPress` without the `click >= 1` condition, which drops nothing else: `crates/abyme-canvas/ui/src/App.tsx`, `useOpenAfterNoteWrite` (Reviewer and security engineer, backlog bugs Task 10, Low).
- After a failed note write, any keydown ends the hold on the press that ended the input, also a key held down or a bare modifier during a double click, so its second click can open the board; clearing only on a keydown without `repeat` that is no bare modifier would close it: `crates/abyme-canvas/ui/src/App.tsx`, `useOpenAfterNoteWrite` (security engineer, backlog bugs Task 9, Low).
- When the AI deletes a note while its text is being typed, the typed text goes without a message: `crates/abyme-canvas/ui/src/NoteContent.tsx` (Reviewer, sticky notes Task 5).

## Tests

- Consolidate the test files that grew across plans (`crates/abyme-core/src/apply/tests.rs`, `crates/abyme-core/src/outline/tests.rs`, `crates/abyme-mcp/tests/server.rs`) in a plan of its own before the dogfooding plan, with a builder for test elements such as `ElementFixture::new("id").summary("s").build()` behind a test feature of `abyme-model`, replacing the `ElementFields` literals in the fixtures (plan reviewer, plan D; the user chose a later plan, and this builder in the statement kind plan).
- A full `cargo test --workspace` ended the test process `element_properties` twice, once with SIGKILL and once with the signal unconfirmed (sticky notes Task 1); it passed alone and in the next full runs, cause unknown, and the cgroup's `oom_kill` counter stands above zero: `crates/abyme-model/tests/element_properties.rs` (plan D Task 4).
- The arrow-key test asserts element, x, and y but not `pinned`; `expect(move?.pinned).toBe(true)` would make it explicit: `crates/abyme-canvas/ui/src/main.test.tsx` (Reviewer, auto-layout Task 4)
- The order test of `Board::placed` compares with `placements.keys()`, so it pins the map's order but not the element-ID order its doc comment promises; assert the IDs explicitly: `crates/abyme-model/src/board.rs` (Reviewer, attach notes Task 1).
- The "arrows in a laid-out canvas" fixture measures every element as 180×90 through one `offsetWidth`/`offsetHeight` mock, handles included; a per-element mock would match a browser fully: `crates/abyme-canvas/ui/src/BoardCanvas.test.tsx` (Reviewer, backlog bugs Task 4).
- No test shows that `detach_note` keeps a pinned note's pin; pin `n` in `detaching_keeps_where_the_note_lies_and_the_relation_and_writes_no_element` and assert it after the detach: `crates/abyme-core/src/apply/tests.rs` (Reviewer, attach notes Task 3).
- The doc comment of `a_note_beside_the_element` calls the note pinned, but the request places it unpinned: `crates/abyme-mcp/tests/server.rs` (Reviewer, attach notes Task 3).
- `place_element`'s `UnknownElement` for a missing element cannot be reached, since `resolve_element` looked it up before, so `place_refuses_an_element_the_request_deleted` covers that lookup and not this branch: `crates/abyme-core/src/apply.rs` (Reviewer, auto-layout Task 5)
- `the_anchor_of_those_requests_is_invalid_on_its_own` accepts any `InvalidElement`; `ElementProblem::InvalidAnchor { .. }` names its intent: `crates/abyme-core/src/apply/tests.rs` (Reviewer, statement kind Task 2)
- In the BoardPane test "keeps a written note through a view asked for before the write answered", `waitFor(() => expect(client.openCalls).toHaveLength(2))` is already true when it runs; the `setTimeout(0)` after it does the waiting, so waiting on something the view shows would state the intent: `crates/abyme-canvas/ui/src/BoardPane.test.tsx` (Reviewer, backlog bugs Task 11).
- A test fails now and then, cause unknown: `crates/abyme-canvas/ui/src/main.test.tsx`, "shows the boards, opens one with its boxes, and moves a box", which presses ArrowRight right after Enter; waiting for the box's `selected` class may fix it, untested (Reviewer, canvas display Task 8).
- The canvas display has not run in a real WebKitGTK window: arrows, breadcrumb, `▸` buttons, the pin's marker and its click, review marks, and `visibilityState` when minimized are checked only in jsdom and from CSS values; for the user (canvas display).

## Cleanup

- The test "asks no more once the app is gone" repeats the focus and interval steps of the StrictMode test "...and stops when the app closes"; only turning visible is new, and could join that test: `crates/abyme-canvas/ui/src/App.test.tsx` (Reviewer, backlog bugs Task 7).
- `canvas.css` has two rules for the marks on a box's top edge, `.element-pin, .element-flag` and `.element-mark`; the shared properties could move to `.element-mark`: `crates/abyme-canvas/ui/src/canvas.css` (Reviewer, backlog bugs Task 6).
- `apply` requests accept `RelationKind` in the object form (`"kind": {"uses": null}`), unlike `Mode` and `StatementKind`; files read only the string and get the normal form, so this is about consistency: `crates/abyme-model/src/relation.rs` (security engineer, plan D Task 1, rated Low; fix with a later change there).
- Make `RECIPE_VERSION` and maybe `ProjectKey` crate-private if nothing outside needs them: `crates/abyme-index/src/lib.rs` (reviewer, search skeleton).
- The doc comment of `draft_board` is wrapped unevenly: `crates/abyme-core/src/apply.rs` (Reviewer, auto-layout Task 3)
- `lines.min(limit)` at the end of `wrapped_lines` changes nothing, since the loop stops at the limit and raises `lines` by at most one per step; it stays as a guard of `MAX_LINES - lines` until someone wants it gone: `crates/abyme-core/src/layout/box_size.rs` (Reviewer, auto-layout Task 5)
- `STATEMENT_DETAIL_LIMIT` could be `SHOWN_LIMIT` like its neighbors in `anchor.rs`, `verification.rs`, and `id.rs`: `crates/abyme-model/src/element.rs` (Reviewer, statement kind Task 4)
- In `BoardPane.test.tsx` the `./testBoards` import sits before `./nodes`, and `VIEW: BoardView = boardView(...)` there and in `nodes.test.ts` repeats the fixture's return type: `crates/abyme-canvas/ui/src/` (Reviewer, canvas display Task 1)
- The doc of `outline` repeats the rule of `relations_on` in prose and could point to it: `crates/abyme-core/src/outline.rs` (Reviewer, canvas display Task 2)
- The return type `{ label: string } | object` of `shownLabel` says nothing: `crates/abyme-canvas/ui/src/edges.ts` (Reviewer, canvas display Task 4)
- "a note that its file is missing" in the doc of `ElementContent` reads ambiguously next to sticky notes, and the line is long: `crates/abyme-canvas/ui/src/ElementBox.tsx` (Reviewer, sticky notes Task 2).
- Two comparisons with `NOTE_TYPE` could become one `TypeRef::is_note()` that `Element::is_note` delegates to: `crates/abyme-core/src/apply.rs`, `crates/abyme-model/src/element.rs` (Reviewer, sticky notes Task 2).
- The test name `the_projection_carries_exactly_the_named_fields_and_no_body` holds only for elements that are not notes: `crates/abyme-canvas/src/projection/tests.rs` (Reviewer, sticky notes Task 2).

## Documentation

- The canvas row of `CLAUDE.md` names "when the files change" for the flags and again "it asks again whenever a file watcher sees the project's files change" at its end, whose "it" is now less clear; say that it asks for the list and the open board there (Reviewer, backlog bugs Task 7).
- The clause "it authorizes nothing" in the `apply` paragraph of `docs/5 Architecture/Canvas.md` is reasoning no reader acts on, and could go. From canvas display, Task 7, security engineer.

## Tooling & CI

- Move `toml` to 1.x in a small plan of its own; it parses repository files, so Security-Hybrid: root `Cargo.toml` (user decision while building the skeleton).

## Open decisions

- Remaining MVP plans, in this order: attaching a sticky note to a box (the Non-Goal of the sticky notes plan); dogfooding on Abyme and the rlsp replay (MVP roadmap the user approved; plan D's Non-Goals).
