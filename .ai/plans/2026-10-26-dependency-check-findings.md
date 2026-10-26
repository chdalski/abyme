**Repository:** root
**Status:** NotStarted
**Created:** 2026-10-26

## Goal

`cargo deny --locked check` fails on the five findings of its first run,
none of which Abyme can fix itself today. Record the decision on each
in `deny.toml`, so that the check passes and fails again on anything
new: allow the license CDLA-Permissive-2.0, and ignore four advisories,
each with its reason.

## Context

- The findings, with advisory database commit
  21d909e4c8bd4e14c7aea58bf980c881a0053ab1:
  - licenses: `webpki-roots` 1.0.10 under CDLA-Permissive-2.0, Mozilla's
    root certificates, through `ureq`, which Abyme and ort's download
    of ONNX Runtime use;
  - RUSTSEC-2024-0429, unsound: `glib` 0.18.5, through gtk-rs 0.18,
    which Tauri uses on Linux; it concerns `VariantStrIter`, which
    Abyme does not call; glib 0.20 fixes it;
  - RUSTSEC-2024-0436, unmaintained: `paste`, a build-time proc-macro
    of arrow and lance in lancedb;
  - RUSTSEC-2024-0370, unmaintained: `proc-macro-error`, a build-time
    proc-macro of glib-macros 0.18 in gtk-rs;
  - RUSTSEC-2024-0449, unmaintained: `unsafe-libyaml`, through
    `serde_yaml_ng` in lindera, lance's default tokenizer, which reads
    its own bundled configuration.
- Risk category: supply-chain configuration that decides which
  dependencies pass.

## Steps

- [ ] Record the decisions on the first run's findings

## Tasks

### Task 1: Record the decisions on the first run's findings

`deny.toml` records the decision on each finding, so that the check
passes on today's graph and fails on any finding that is not one of
these five.

Review path: Security-Hybrid

- [ ] The license allow-list in `deny.toml` also holds
      CDLA-Permissive-2.0, and nothing else in it changes.
- [ ] `deny.toml` ignores exactly RUSTSEC-2024-0429, RUSTSEC-2024-0436,
      RUSTSEC-2024-0370, and RUSTSEC-2024-0449, each by its ID with a
      reason that names the crate that brings it in and when the entry
      can go.
- [ ] `cargo deny --locked check` on the committed manifests passes all
      four checks, and `Cargo.lock` stays byte-identical.
- [ ] With the committed `deny.toml`, a crate with another
      vulnerability and a crate with another unmaintained advisory each
      still fail the check; the handoff shows both, and neither test
      change is committed.

## Decisions

- **CDLA-Permissive-2.0 allowed for every crate:** it asks only that
  the license text travel with the data, as MIT asks for its notice, so
  it fits the allow-list's rule.
- **Each advisory ignored by its ID, with a reason:** lowering the
  unmaintained scope to the workspace would also hide every future
  finding in a transitive crate. An entry goes once its reason no
  longer holds.
