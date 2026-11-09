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
- A single view that carries both the deletion of a note whose field is open and a new element under its ID never leaves the note out, so the open field carries over onto a new note and its write lands there (a new element of another kind closes the field instead, see Bugs); a write already on its way when the view that leaves the note out arrives lands on whatever holds the ID too: `crates/abyme-canvas/ui/src/nodes.ts`, `keepUiState` (security engineer, backlog bugs Task 13, Low).

## Bugs

- A board opened by Enter while a note write is pending marks no mouse double click in flight when the write answers, so that double click's rest can act on the new board; `opened()` could set `openedByPress` without the `click >= 1` condition, which drops nothing else: `crates/abyme-canvas/ui/src/App.tsx`, `useOpenAfterNoteWrite` (Reviewer and security engineer, backlog bugs Task 10, Low).
- After a failed note write, any keydown ends the hold on the press that ended the input, also a key held down or a bare modifier during a double click, so its second click can open the board; clearing only on a keydown without `repeat` that is no bare modifier would close it: `crates/abyme-canvas/ui/src/App.tsx`, `useOpenAfterNoteWrite` (security engineer, backlog bugs Task 9, Low).
- A view that lists an open note's ID as anything but a note closes its field, and the typed text goes without a message: a hand deletion of the note's file, which the board still places, shows it `missing`, and one view can carry the deletion and a non-note element under its ID: `crates/abyme-canvas/ui/src/nodes.ts`, `keepUiState` (Reviewer, backlog bugs Task 13, Low).
- The user's own new note whose ID equals a held note's counts as listed, so it vanishes until the held note's field closes and the latest view brings it back; nothing wrong is written: `crates/abyme-canvas/ui/src/boardNodes.ts`, `draftWritten`, and `keepUiState` (security engineer and Reviewer, backlog bugs Task 13, Low).
- A note a view lists afresh but hanging on a held note's ID is held back with that box, so if its own field was open, its field and text go: `crates/abyme-canvas/ui/src/nodes.ts`, `keepUiState` (Reviewer, backlog bugs Task 13, Low).
- The anchors of the components `project-store` (`crates/abyme-core/src/project`) and `layout` (`crates/abyme-core/src/layout`) leave out their module roots `project.rs` and `layout.rs`, so a change there flags only `abyme-core`, not the component or the notes that describe it; one anchor per element cannot cover both: `abyme/elements/project-store.md`, `abyme/elements/layout.md` (lead and Reviewer, docs map Task 2).

## Tests

- A full `cargo test --workspace` ended the test process `element_properties` twice, once with SIGKILL and once with the signal unconfirmed (sticky notes Task 1); it passed alone and in the next full runs, cause unknown, and the cgroup's `oom_kill` counter stands above zero: `crates/abyme-model/tests/element_properties.rs` (plan D Task 4).
- The arrow-key test asserts element, x, and y but not `pinned`; `expect(move?.pinned).toBe(true)` would make it explicit: `crates/abyme-canvas/ui/src/main.test.tsx` (Reviewer, auto-layout Task 4)
- The order test of `Board::placed` compares with `placements.keys()`, so it pins the map's order but not the element-ID order its doc comment promises; assert the IDs explicitly: `crates/abyme-model/src/board.rs` (Reviewer, attach notes Task 1).
- The "arrows in a laid-out canvas" fixture measures every element as 180×90 through one `offsetWidth`/`offsetHeight` mock, handles included; a per-element mock would match a browser fully: `crates/abyme-canvas/ui/src/BoardCanvas.test.tsx` (Reviewer, backlog bugs Task 4).
- `place_element`'s `UnknownElement` for a missing element cannot be reached, since `resolve_element` looked it up before, so the `placement` case of `knows_a_deleted_element_no_longer` covers that lookup and not this branch: `crates/abyme-core/src/apply.rs` (Reviewer, auto-layout Task 5)
- In the BoardPane test "keeps a written note through a view asked for before the write answered", `waitFor(() => expect(client.openCalls).toHaveLength(2))` is already true when it runs; the `setTimeout(0)` after it does the waiting, so waiting on something the view shows would state the intent: `crates/abyme-canvas/ui/src/BoardPane.test.tsx` (Reviewer, backlog bugs Task 11).
- A test fails now and then, cause unknown: `crates/abyme-canvas/ui/src/main.test.tsx`, "shows the boards, opens one with its boxes, and moves a box", which presses ArrowRight right after Enter; waiting for the box's `selected` class may fix it, untested (Reviewer, canvas display Task 8).
- The canvas display has not run in a real WebKitGTK window: arrows, breadcrumb, `▸` buttons, the pin's marker and its click, review marks, and `visibilityState` when minimized are checked only in jsdom and from CSS values; for the user, together with the webview checklist in the Task Notes of `.ai/plans/completed/2026-11-03-backlog-bugs.md` (canvas display; backlog bugs Task 13, the user deferred the checklist).
- Three loops in the server tests send each case through `call_refused`, whose message shows the result but not the case's arguments, so a case that is wrongly accepted no longer names its arguments when it fails; a context argument for the call helpers would bring them back: `crates/abyme-mcp/tests/server/protocol.rs` and `notes.rs`, `answers_arguments_that_do_not_fit_with_tool_errors_and_keeps_serving`, `read_board_takes_coordinates_only_as_an_optional_boolean`, and the attachment refusals (Reviewer, consolidate tests Task 6, Low).

## Cleanup

- The doc of `Fallback::objects` ends "review looks up only the former", which makes the reader work out what "the former" is; "A looked-up path that is missing held nothing; every other path is left out." says it directly: `crates/abyme-core/src/review.rs` (Reviewer, file lists after a rewrite Task 3).
- The doc of `recorded_matches` has one line past the comment's wrap width: `crates/abyme-core/src/review.rs` (Reviewer, file lists after a rewrite Task 3).
- `tree_objects` collects every entry into an owned `Vec` before filtering; mapping and filtering in one `Result` pipeline still fails on any broken entry without the copy: `crates/abyme-core/src/project/git.rs` (Reviewer, file lists after a rewrite Task 3).
- `fails_on_a_broken_entry_kept_or_not` picks each case's record by a string label in a `match` with a catch-all arm; a builder function per case would keep the cases and the records together: `crates/abyme-core/src/project/git/tests.rs` (Reviewer, file lists after a rewrite Task 3).
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
- `ElementContent` takes `editing` as optional, defaulting to false, only for the standalone tests in `ElementBox.test.tsx`; making it required keeps the prop with one source: `crates/abyme-canvas/ui/src/ElementBox.tsx` (Reviewer, backlog bugs Task 12).
- Three reads in the apply tests take a board's placements through `before.boards[&board_id(..)].placements`, the read-only twin of `placements_on`; a second helper for three reads did not pay: `crates/abyme-core/src/apply/tests/` (`layout.rs`, `hierarchy.rs`, `placement.rs`) (Reviewer, consolidate tests Task 3, Low).
- Three imports of the server test root are used by one module each, `TcpListener` by `environment.rs` and `ErrorCode` and `JsonObject` by `protocol.rs`; they could move into those modules: `crates/abyme-mcp/tests/server/main.rs` (Reviewer, consolidate tests Task 7, Low).
- The two loops that fill `ends` in `focusOf` could be one declarative `Map` whose later entries win; about the same size: `crates/abyme-canvas/ui/src/edges.ts` (Reviewer, canvas usability Task 3, Low).
- The board list's row keys take the index of the visible row, so a toggle remounts every row after the branch; the index in the full list would stay stable: `crates/abyme-canvas/ui/src/BoardList.tsx` with `visibleRows` in `boardTree.ts` (Reviewer, canvas usability Task 6, Low).
- `hasInner`'s check `index + 1 < rows.length` is redundant, since a missing row counts as level 0: `crates/abyme-canvas/ui/src/boardTree.ts` (Reviewer, canvas usability Task 6, Low).
- The bare block after the `if` in the `loaded` case could be a `case "loaded": { … }` block: `crates/abyme-canvas/ui/src/BoardList.tsx` (Reviewer, canvas usability Task 6, Low).
- Two assertions in "gives a toggle only to a board with boards below it" check nothing, since `getByRole` throws already: `crates/abyme-canvas/ui/src/App.test.tsx` (Reviewer, canvas usability Task 6, Low).
- At the sidebar's minimum width of 10rem, a board at depth 7 or 8 gets nearly all of it as indent, so its title shrinks to nothing and the list scrolls sideways: `crates/abyme-canvas/ui/src/canvas.css` with `INDENT_LEVELS` in `BoardList.tsx` (Reviewer, canvas usability Task 7, Low).
- `host !== undefined` in `notesOnTheSelection` is redundant, since `attached_to` is `string | null`: `crates/abyme-canvas/ui/src/edges.ts` (Reviewer, canvas arranging and notes Task 1, Low).
- The test "keeps the notes hanging on a selected box undimmed" has one note on the box; a second would pin "every note": `crates/abyme-canvas/ui/src/BoardCanvas.test.tsx` (Reviewer, canvas arranging and notes Task 1, Low).
- `boxHolding` builds its own map of nodes on each call, so `onTheBoardAndHung` builds one per moved note; negligible for a drag, and taking the caller's map would widen its signature: `crates/abyme-canvas/ui/src/nodes.ts` (Reviewer, canvas arranging and notes Task 3, Low).

## Documentation

- The canvas row of `CLAUDE.md` names "when the files change" for the flags and again "it asks again whenever a file watcher sees the project's files change" at its end, whose "it" is now less clear; say that it asks for the list and the open board there (Reviewer, backlog bugs Task 7).
- The clause "it authorizes nothing" in the `apply` paragraph of `docs/5 Architecture/Canvas.md` is reasoning no reader acts on, and could go. From canvas display, Task 7, security engineer.
- The Decisions entry "Minimum zoom 10 %" of `.ai/plans/2026-11-09-canvas-usability.md` says boards up to ten times the window fit; with React Flow's fitting margin of a tenth it is about nine times (Reviewer, canvas usability Task 2).

## Tooling & CI

- Move `toml` to 1.x in a small plan of its own; it parses repository files, so Security-Hybrid: root `Cargo.toml` (user decision while building the skeleton).

## Open decisions

- Remaining MVP plan: the rlsp replay (MVP roadmap the user approved; plan D's Non-Goals).
