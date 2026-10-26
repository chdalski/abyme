**Repository:** root
**Status:** NotStarted
**Created:** 2026-10-26

## Goal

Review flags will come from what changed in the repository under an
element's anchor since its verification stamp. Give elements both, without
any Git access yet: an anchor, the path in the project that the element
stands for, and a verification stamp, the commit it was last checked against
and by whom. The AI sets anchors through `apply` and reads anchors and stamps
through `read_element`; a later plan computes the flags and adds `verify`,
the only way to set a stamp.

## Context

- Element files and requests come from untrusted sources: clones, pull
  requests, and the AI over MCP. Risk categories: untrusted input (two new
  frontmatter fields, a new field in two operations) and data persistence
  (the store rewrites element files whose stamp an update removes).
- The concept: `docs/2 Concepts/Elements.md` ("Anchors"),
  `docs/3 Reconciliation/Reconciliation.md` ("Detecting Changes"),
  `docs/3 Reconciliation/Checks.md` ("Verification Stamp"),
  `docs/5 Architecture/Data Model.md`, `docs/5 Architecture/Storage in Git.md`
  ("Element File"). They write anchors as `file://` URIs.

## Steps

- [ ] Anchors and stamps in the element file
- [ ] Anchors in requests and in the element detail

## Tasks

### Task 1: Anchors and stamps in the element file

An element can carry an anchor and a verification stamp in its frontmatter,
read and written with the same strictness as its other fields.

Review path: Security-Hybrid

- [ ] An element file may hold `anchor`, a path relative to the project
      root such as `"crates/payment-service"`, and the stamp as two keys,
      `verified`, a full commit ID, and `verified_by`, `"ai"` or
      `"person"`. Abyme writes them after `summary` and before
      `relations`, double-quoted; an element without them has none of the
      keys, so every existing element file reads and writes as before,
      byte for byte.
- [ ] An anchor is rejected when it is empty, starts with `/`, has an empty,
      `.`, or `..` segment, ends with `/`, holds a backslash, a control
      character, a bidi control, or another character a name may not hold,
      or is longer than 4096 bytes or has a segment longer than 255 bytes.
      It is also rejected when its first segment is `abyme` or `.abyme`, or
      when any segment is `.git`, compared without regard to ASCII case;
      an `abyme` segment deeper in the path is accepted.
- [ ] A commit ID is rejected unless it is exactly 40 or 64 lowercase hex
      digits. `verified` and `verified_by` come together or not at all;
      any other value of `verified_by` is rejected. A stamp is valid with
      or without an anchor.
- [ ] Errors about an anchor or a stamp show the value only escaped and
      shortened, as for the other fields.
- [ ] The property tests of element files generate anchors and stamps:
      every element written reads back equal, and arbitrary text never
      makes reading panic.
- [ ] No document in `docs/` writes a path in the project as a `file://`
      anchor or defines an anchor of such a path as a URI; a reader of
      Storage in Git and Data Model can tell how anchors and stamps are
      stored and which values they take, and Elements, Reconciliation,
      Extensions, and the Glossary show anchors as project-relative paths
      that cover the path and everything below it. A scheme such as
      `jira://` for a later fetcher stays as it is.
- [ ] `cargo test --workspace`, `cargo clippy --workspace
      --all-targets`, and `cargo fmt --all --check` pass.

### Task 2: Anchors in requests and in the element detail

The AI sets, changes, and removes an element's anchor through `apply` and
reads anchor and stamp through `read_element`. No request can set a stamp.

Review path: Security-Hybrid

- [ ] `create_element` takes an optional `anchor`. `update_element` takes
      `anchor`: a string sets or replaces it and `null` removes it, as for
      `summary`. The value passes the checks of Task 1.
- [ ] An update that gives the element a different anchor, also one where
      it had none, or removes its anchor, also removes the element's stamp;
      an update that leaves the anchor as it is keeps the stamp. No
      operation takes `verified` or `verified_by`.
- [ ] `read_element` shows, after `Type:`, the lines
      `Anchor: crates/payment-service` or `Anchor: none`, and
      `Verified: <full commit ID> by ai` (or `by person`) or
      `Verified: none`.
- [ ] The canvas still refuses `create_element` and `update_element`.
- [ ] An MCP test creates an anchored element through `apply`, reads it
      back through `read_element`, changes the anchor, and sees the stamp
      of a hand-written file removed.
- [ ] Data Model, the `apply` and `read_element` tool descriptions, the
      Tools section of `crates/abyme-mcp/README.md`, and MCP Server's
      element detail describe the fields as delivered; `docs/Decisions.md`
      records the anchor form and that only `verify` sets a stamp.
- [ ] `cargo test --workspace`, `cargo clippy --workspace
      --all-targets`, and `cargo fmt --all --check` pass.

## Decisions

- **Plan B is split in two:** this plan adds anchors and stamps without
  Git; the next one adds Git access, which changes the security baseline,
  the review flags, `verify`, and `status`, on the types this plan settles.
- **An anchor is a plain path relative to the project root,** not a URI:
  the documented `file://crates/x/` form parses `crates` as a host.
- **An anchor covers its path and everything below it;** there is no
  trailing-slash convention, so a folder anchor written without the slash
  cannot silently miss changes.
- **Anchors are rejected, not normalized,** and compared byte for byte
  later; normalizing would make some file names impossible to anchor.
- **Reserved names:** only the project root's `abyme` and `.abyme` hold
  Abyme's files, so they are reserved as the first segment only; a stamp
  written into an element file must never flag the element that holds it.
  `.git` is reserved at every depth, since repositories nest in a project.
  Case does not count for them, because some file systems ignore it.
- **The length caps, 4096 bytes and 255 bytes per segment,** are the
  common limits of paths and file names.
- **The stamp is two flat keys,** `verified` and `verified_by`, so the
  frontmatter stays a mapping of plain values except `relations`.
  `verified_by` is a label, not a checked identity.
- **A stamp needs no anchor:** an element without one is verified too,
  as the base for the flags its relations pass on.
- **Only `verify` sets a stamp,** with the commit the server reads itself;
  a request that could set one could hide a change. This plan therefore has
  no way to set a stamp through MCP; hand-written stamps are read.
- **A changed, new, or removed anchor drops the stamp:** the element then
  stands for something else, which nobody has checked.
- **Whether an anchor names something Git knows is not checked here:**
  `apply` stays free of Git; the next plan reports such an anchor as a
  flag.

## Non-Goals

- Git access, the security baseline's change, review flags, `verify`,
  `status`, and changes in nested repositories: the next plan.
- Showing anchors in `read_board` or the canvas.
