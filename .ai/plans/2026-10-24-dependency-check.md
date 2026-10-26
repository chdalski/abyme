**Repository:** root
**Status:** InProgress
**Created:** 2026-10-24

## Goal

The Rust dependencies get no automated check today: known
vulnerabilities go unseen, and the rule that every dependency is
MIT-compatible is checked by hand. Add cargo-deny to the devcontainer
and configure it to check the whole dependency graph for RustSec
advisories, licenses, sources, and the features the manifests forbid,
and present the findings of its first run to the user, who decides each
one afterwards.

## Context

- cargo-deny 0.20.2: https://github.com/EmbarkStudios/cargo-deny; its
  configuration reference: https://embarkstudios.github.io/cargo-deny/
- Risk categories: a third-party binary that runs in the devcontainer;
  supply-chain configuration that decides which dependencies pass.

## Steps

- [x] Install cargo-deny in the devcontainer
- [x] Check advisories, licenses, and sources
- [ ] Enforce the forbidden features

## Tasks

### Task 1: Install cargo-deny in the devcontainer

The devcontainer gets cargo-deny 0.20.2 from its GitHub release, the
musl build for amd64 and arm64. Rust arrives through a devcontainer
feature after the image build, so the Dockerfile cannot run
`cargo install`; it installs the Tauri CLI the same way, as a prebuilt
release binary pinned per architecture by SHA-256.

Review path: Security-Hybrid

- [x] The Dockerfile installs cargo-deny 0.20.2, verified against a
      SHA-256 per architecture written in the Dockerfile, and fails
      before extracting an archive whose hash does not match, and on an
      architecture other than amd64 and arm64.
- [x] `cargo deny --version` in the running container reports 0.20.2.
- [x] Each pinned hash equals the release's own `.sha256` file and the
      digest GitHub reports for the asset; the handoff names all three
      per architecture.
- [x] A run of the install commands with a wrong hash and one with an
      unknown architecture both exit non-zero, and the handoff shows
      both.
- [x] The root `CLAUDE.md` names cargo-deny among the devcontainer's
      tools.

### Task 2: Check advisories, licenses, and sources

A `deny.toml` at the root configures `cargo deny check` for the whole
workspace, and its first run shows what the dependencies violate today.

Review path: Security-Hybrid

- [x] The check covers the whole workspace with all features, normal,
      build, and dev dependencies, for x86_64 and aarch64 Linux.
- [x] A vulnerability, an unsound or unmaintained crate anywhere in the
      graph, and a yanked version each fail the check; `deny.toml`
      ignores no advisory.
- [x] The license allow-list is exactly: MIT, MIT-0, Apache-2.0,
      Apache-2.0 WITH LLVM-exception, BSD-2-Clause, BSD-3-Clause,
      0BSD, ISC, Zlib, Unicode-3.0, Unlicense, CC0-1.0, and BSL-1.0.
      MPL-2.0 is allowed only for `cssparser`, `cssparser-macros`,
      `dtoa-short`, `selectors`, and `option-ext`.
- [x] Only crates.io is an allowed source: a git dependency added for
      the test fails the check; the handoff shows it, and the test
      change is not committed.
- [x] `cargo deny --locked check` leaves `Cargo.lock` byte-identical.
- [x] From the root `CLAUDE.md`, a reader learns how to run the check
      and what it checks.
- [x] The handoff lists every finding of the first run with its check,
      crate, and advisory ID where the check has one, and the commit of
      the advisory database it used. No finding is ignored or fixed in
      this plan.

### Task 3: Enforce the forbidden features

Comments in the root `Cargo.toml` forbid features of six crates. They
become bans, so that a manifest change that enables one fails the
check.

Review path: Security-Hybrid

- [ ] These features are denied, and nothing else is: `fastembed`'s
      `hf-hub` and every feature whose name starts with `hf-hub`,
      `online`, `qwen3`, and `nomic-v2-moe`; `ort`'s `load-dynamic`;
      `tauri`'s `devtools`, `protocol-asset`, `webview-data-url`,
      `isolation`, `tray-icon`, `unstable`, and `macos-private-api`;
      `toml`'s `unbounded`; `ureq`'s `gzip`; `ts-rs`'s `format`. The
      crate `hf-hub` is denied.
- [ ] Enabling a forbidden feature makes the check fail, shown in the
      handoff for one feature of each of the six crates and for a
      dependency on `hf-hub`; none of these test changes is committed.
- [ ] The check of the committed manifests reports no finding in bans.
- [ ] A reader of a forbidding comment in `Cargo.toml` learns that
      `deny.toml` enforces it, and a reader of a ban in `deny.toml`
      learns where the reason for it stands.

## Decisions

- **cargo-deny, not cargo-audit:** it checks the same RustSec
  advisories and also licenses and sources, so the MIT-compatibility
  rule needs no hand check.
- **The license allow-list holds only licenses that permit use in an
  MIT project without conditions beyond keeping notices.** Any other
  license a crate declares, such as CDLA-Permissive-2.0, is a finding
  of the first run for the user to decide.
- **MPL-2.0 per crate:** a new crate under MPL-2.0 fails the check and
  comes to the user, instead of passing unseen. The four build-time
  crates come with Tauri, `option-ext` with lancedb and Tauri.
- **Entries in `deny.toml` that let a dependency pass** (licenses,
  license exceptions, advisory ignores, sources) are added only when
  the user decides; otherwise they may only be removed.
- **Findings are presented, not fixed:** each finding of the first run
  goes to the user, and each update or ignore becomes a change of its
  own. Until then, `cargo deny --locked check` fails on those findings.
- **The advisory database is fetched on every run,** without a token:
  a stale database fails silently. A run without network is not the
  documented default.
- **Hashes pinned in the repository,** cross-checked at pinning: the
  release's `.sha256` comes from the same place as the binary, so it
  guards only the transport. The release has no signature to verify.
- **Linux targets only:** abyme-core builds on Unix only. Advisories in
  crates used only on macOS or Windows stay unseen until the canvas
  targets them.
- **Multiple versions of a crate are allowed:** lancedb and Tauri pull
  in many, and a ban would only add noise.

## Non-Goals

- License and vulnerability checks of the npm packages, which
  `pnpm audit` and `pnpm licenses list` cover by hand
- A CI pipeline
