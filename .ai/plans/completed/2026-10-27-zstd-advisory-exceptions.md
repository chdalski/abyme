**Repository:** root
**Status:** Completed (2026-10-27)
**Created:** 2026-10-27

## Goal

`cargo deny --locked check` fails on thirteen RustSec advisories for `zstd`
0.13.3 and `zstd-safe` 7.3.0, which lancedb pulls in. No lance release uses
the fixed versions yet, and Abyme cannot trigger any of the thirteen flaws.
Name each advisory in `deny.toml` with the reason it does not apply, so the
check passes again and still catches every other advisory, and make it fail
when a `zstd-safe` or `zstd-sys` feature that a reason relies on gets enabled
or when a crate other than `arrow-ipc` and `lance-encoding` depends on
`zstd` or a crate other than `zstd` on `zstd-safe`. A change in how
`arrow-ipc` and `lance-encoding` call `zstd` stays a check by hand.

## Context

- `zstd` reaches the tree only through `arrow-ipc` 58.4.0, which lance uses
  for compressed deletion files, and `lance-encoding` 13.0.0, which
  compresses large values and blocks and decompresses them when reading. Both
  come with lancedb 0.40.0 and require `zstd` 0.13. The fixes are in `zstd`
  0.14.3 and `zstd-safe` 8.2.0; lancedb 0.40.0 and lance 13 are the newest
  releases, and no `zstd` 0.13 release carries the fixes.
- Seven advisories concern code that exists only with `zstd-safe` features no
  crate enables (`experimental`, `zstdmt`, `seekable`). Three concern
  functions nothing calls: the `with_context` constructors,
  `OutBuffer::around_pos`, and `train_from_buffer`. Three concern functions
  that run, but under conditions that never occur: a multithreaded frame, a
  stream left open on a context used for one-shot calls, and a
  `stream::write::Decoder`.
- Each existing ignore in `deny.toml` says why Abyme does not reach the
  affected code and when the entry goes.
- Risk categories: the security configuration of the dependency check, which
  here accepts known flaws in dependency code as unreachable. What `zstd`
  decompresses comes only from the index in the user's cache, which the
  Security Baseline trusts.

## Steps

- [x] Exceptions for the zstd advisories, with checks that keep them true

## Tasks

### Task 1: Exceptions for the zstd advisories, with checks that keep them true

Each of the thirteen advisories gets an exception that says why Abyme cannot
trigger it, and the check enforces the facts those reasons rest on.

Review path: Security-Hybrid

- [x] `deny.toml` names each of RUSTSEC-2026-0391 and RUSTSEC-2026-0395 to
      RUSTSEC-2026-0406 in an ignore entry of its own, whose reason says why
      Abyme cannot trigger the flaw and that the entry goes once lancedb's
      tree holds no `zstd` below 0.14.3 and no `zstd-safe` below 8.2.0.
- [x] `cargo deny --locked check` fails when a crate enables `zstd-safe`'s
      `experimental`, `zstdmt`, or `seekable` feature or `zstd-sys`'s
      `zstdmt` or `pkg-config` feature, when a crate other than `arrow-ipc`
      and `lance-encoding` depends on `zstd`, and when a crate other than
      `zstd` depends on `zstd-safe`; a scratch copy of the workspace outside
      the repository shows the failure for each of the five features and for
      each of the two kinds of crate.
- [x] What `deny.toml` says about its feature bans holds for the new bans
      too, and the reason of each new ban names the ignore entries it
      protects and says that the user decides whether it stays once those
      entries go.
- [x] A reader of `deny.toml` learns what to check again when lancedb,
      `arrow-ipc`, or `lance-encoding` changes, since the check cannot see a
      new `zstd` call inside those crates.
- [x] `cargo deny --locked check` passes and warns about none of the new
      entries.

## Decisions

- **Exceptions, not waiting, a fork, or turning `zstd` off:** a check that
  always fails hides the next advisory in any other crate; a fork of `zstd`
  0.13 means maintaining thirteen fixes in unsafe FFI code from a new git
  source; and lance's default features enable `zstd`, which Abyme cannot
  turn off.
- **One entry per advisory:** each flaw has its own reason, and an entry
  whose reason stops holding can go alone.
- **`zstd-sys`'s `pkg-config` is banned too:** with it, Abyme would link the
  system's libzstd, usually built with multithreading, against which one
  reason relies on the bundled library refusing worker threads. The
  environment variable `ZSTD_SYS_USE_PKG_CONFIG` does the same at build time
  and is outside what the check sees; that stays accepted.
- **Accepted:** element text from the repository is compressed with `zstd`
  0.13.3, on a one-shot path none of the advisories concerns; a process of
  the user's could make lance allocate without bound through a tampered
  index file, which the Security Baseline's trust in the cache already
  covers.
