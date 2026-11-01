**Repository:** root
**Status:** InProgress
**Created:** 2026-11-01

## Goal

An element takes its statement kind from its home board's zone, so a question or
a decision on a board of current facts cannot say what it is. Let an element carry
a statement kind of its own, which takes precedence over the zone, and let everything
that reads statement kinds follow it.

## Context

- The concept: `docs/2 Concepts/Boards and Zones.md` ("Inheritance", whose Proposal
  this plan builds for elements only), `docs/5 Architecture/Data Model.md`,
  `docs/5 Architecture/Storage in Git.md`, and `docs/Decisions.md`.
- The element: `crates/abyme-model/src/element.rs`, `element/yaml.rs`, and
  `element/frontmatter.rs`. The kinds: `crates/abyme-model/src/zone.rs`.
- The operations: `crates/abyme-core/src/operation.rs` and `apply.rs`, where
  `create_element` and `update_element` stand at clippy's limit of arguments.
- The readers of a kind: `crates/abyme-core/src/statement.rs`, called from
  `outline.rs` (`read_board`, `search`, `status`, `read_element`) and from
  `crates/abyme-index/src/index.rs` (the filter of `search`).
- Risk categories: untrusted input (element files and `apply` gain a field), and
  data persistence (the writer of element files changes).

## Steps

- [x] `Element` holds its fields
- [x] The order of an element's errors is pinned
- [x] Element parameters as one value
- [ ] An element file and a request can hold a statement kind
- [ ] The element's own kind takes precedence

## Tasks

### Task 1: `Element` holds its fields

`Element` and `ElementFields` repeat each other, so a new field is entered at five
places. Let `Element` hold an `ElementFields`. Behavior is unchanged, and the bytes of
every element file with it; `crates/abyme-model/src/element/tests.rs` and
`crates/abyme-model/tests/element_properties.rs` pin it.

Review path: Security-Hybrid

- [x] `Element` holds an `ElementFields`, and adding a field to it needs no other list
      of fields in `element.rs` or `element/frontmatter.rs`.
- [x] No public signature of `abyme-model` changes.
- [x] The named tests pass with no assertion changed.
- [x] `cargo test --workspace`, `cargo clippy --workspace --all-targets`, and
      `cargo fmt --all --check` pass.

### Task 2: The order of an element's errors is pinned

The next task moves the parameters of `create_element` and `update_element`, whose checks
run in a fixed order, and no test covers a request with several invalid values. Pin which
error such a request gets today.

Review path: Reviewer-only

- [x] Tests in `crates/abyme-core/src/apply/tests.rs` pin, through `apply`, the error of a
      `create_element` with a taken key, an invalid type, and an invalid anchor at once, of
      one with an invalid type and an invalid anchor, and of an `update_element` with an
      invalid type and an invalid anchor.
- [x] No production code changes.
- [x] `cargo test --workspace`, `cargo clippy --workspace --all-targets`, and
      `cargo fmt --all --check` pass.

### Task 3: Element parameters as one value

`create_element` and `update_element` take `self` and six more arguments, and one more
would pass clippy's limit. Give each one private value that holds its parameters, built
in the dispatch of `apply`. Behavior is unchanged, also which error comes first;
`crates/abyme-core/src/apply/tests.rs`, with the tests of the preceding task, and
`crates/abyme-core/tests/apply_properties.rs` pin it.

Review path: Reviewer-only

- [x] `create_element` and `update_element` in `crates/abyme-core/src/apply.rs` each take
      one private value for those parameters. `Operation` and its schema do not change,
      and the other operations stay positional.
- [x] The named tests pass with no assertion changed, including an update that names no
      field and the errors of a request with several invalid values.
- [x] `cargo test --workspace`, `cargo clippy --workspace --all-targets`, and
      `cargo fmt --all --check` pass.

### Task 4: An element file and a request can hold a statement kind

An element gets an optional statement kind of its own, stored in its file and set through
`apply`. Nothing reads it for an element's kind yet; Task 5 does.

Review path: Security-Hybrid

- [ ] An element file holds an optional `statement` with one of the kinds `current`,
      `target`, `decision`, `idea`, `knowledge`, and `question`, after `anchor` and before
      `verified`, written only when set, so a file without one reads and writes byte for
      byte as before. Any other value, an empty one, and `null` are rejected with an error
      that shows the value escaped and shortened, and another unknown key stays rejected.
- [ ] `create_element` takes an optional `statement`. `update_element` takes one that sets
      the kind, and `null` removes it; on `create_element`, `null` means no kind, as for its
      other optional fields. An update of `statement` removes the element's verification
      stamp as every `update_element` does, and `verify` keeps the kind.
- [ ] The JSON schema of `apply` describes both, as the core reads them.
- [ ] Unit and property tests cover the file and both operations; an integration test
      through the MCP tool `apply` creates an element with a kind, changes and removes it,
      and reads the file back.
- [ ] A reader of Storage in Git, the operations in Data Model, the `apply` description, and
      the MCP README can tell how to give an element a kind of its own, and that the zone
      stays the normal source and an own kind the exception.
- [ ] `cargo test --workspace`, `cargo clippy --workspace --all-targets`, and
      `cargo fmt --all --check` pass.

### Task 5: The element's own kind takes precedence

An element's own kind wins over its home board's zone and needs no home. One place decides
this, and every reader of a kind follows.

Review path: Security-Hybrid

- [ ] An element's kind is its own if it has one, otherwise as before the kind of its home
      board's zone, otherwise none. An element without an own kind reads as before in every
      output.
- [ ] `read_element` shows `Statement kind: <kind>, from the element` for an own kind.
      `read_board`, `search`, and `status` show the own kind in their kind column, and that
      column also appears on a board that is no zone when one of its elements has an own
      kind, `none` for the others.
- [ ] `search` with `statement` finds an element by its own kind, also without a home; after
      an own kind is set, changed, or removed, the next `search` filters by the new kind, with
      no board change.
- [ ] Tests cover each state: an own kind without a home, on a home that is no zone, on a zone
      of another kind, on a zone of the same kind, and on an element several boards place.
      A test in `crates/abyme-index/tests/index.rs` covers setting, changing, and removing an
      own kind across refreshes of `search`. An integration test through the MCP tools `apply`,
      `read_board`, and `read_element` covers an own kind on a zone of another kind.
- [ ] A reader of Boards and Zones, Data Model, MCP Server, Search and Index, the Decisions
      table, the descriptions of the read tools and `search`, and the MCP README can tell
      where an element's kind comes from, and sees that frames and element types bringing a
      kind are still a proposal.
- [ ] The doc comments in `abyme-core`, `abyme-index`, and `abyme-mcp` that say where an
      element's kind comes from state the new rule, and "own kind" means only a kind the element
      itself holds.
- [ ] `cargo test --workspace`, `cargo clippy --workspace --all-targets`,
      `cargo fmt --all --check`, and `pnpm --dir crates/abyme-canvas/ui check` pass.

## Task Notes

- **Task 3:** the order of the anchor against the checks of `Element::new` on
  `create_element`, and of resolving the element against the type on `update_element`,
  has no test; keep them where they are.

## Decisions

- **`Element` holds `ElementFields`, as a task of its own first:** the new field would edit
  the repetition at five places, and later fields would again.
- **Both `create_element` and `update_element` get one value:** an exception for clippy was
  declined, as eight positional arguments of alike types are easily swapped. The other
  operations gain no argument, so they stay positional.
- **Test fixtures gain the new field by hand:** a builder for test elements goes into the
  later plan that consolidates the tests (`.ai/backlog.md`), since it would touch the same
  files; a constructor here was declined.
- **Changing an own kind removes the stamp, changing a zone does not:** turning an idea into
  a current fact claims something nobody checked, and the stamp vouches for the element's file.
  A zone change writes only the board file, as it does today. Keeping the stamp was
  declined, since a changed claim would still count as checked.
- **All six kinds on any element, with or without a home:** an own kind is a deliberate
  exception, visible in the diff, so nothing becomes a fact by accident. Allowing it only with
  a home was declined: it needs a new check on delete and on files from elsewhere, and notes
  without a board would have no kind.
- **`null` falls back to the zone:** there is no way to say "no kind" on an element in a zone,
  since nothing needs it.
- **The kind stays out of the indexed text:** it is a filter, so the index recipe does not
  change; a changed kind replaces the row as a zone change does today.

## Non-Goals

- Frames with settings of their own, and element types that bring a kind.
- A builder for test elements (a later plan, in `.ai/backlog.md`).
- Showing a statement kind on the canvas.
- Moving other operations away from positional arguments.
