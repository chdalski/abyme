**Repository:** root
**Status:** InProgress
**Created:** 2026-10-26

## Goal

Before the MVP work starts, fix what misleads agents in this repository:
`CLAUDE.md` does not say that Bash tool calls run in zsh while the
environment names fish, two test comments describe their tests wrongly,
and the TypeScript language server fails to start, so agents cannot use
`LSP` on the canvas UI's code.

## Context

- Risk category of Task 2: devcontainer configuration that installs
  packages from the npm registry.

## Steps

- [x] Correct the agent hints in `CLAUDE.md` and the two test comments
- [ ] Install a TypeScript with `tsserver` for the language server

## Tasks

### Task 1: Correct the shell hint and the two test comments

`CLAUDE.md` does not tell agents which shell their Bash tool calls run
in, and the doc comments of two integration test files describe their
ignored tests wrongly; this task corrects all three.

Review path: Reviewer-only

- [x] A Conventions line in the root `CLAUDE.md`, directly after the
      `.devcontainer/` line, tells agents that Bash tool calls run in
      zsh although `SHELL` and the session's environment name fish, and
      to write POSIX shell syntax, with fish's `set S dir` against
      `S=dir` as the example.
- [x] The doc comment of `crates/abyme-index/tests/model.rs` gives
      `cargo test --workspace --test model -- --ignored` as the command
      for its ignored test.
- [x] The doc comment of `crates/abyme-mcp/tests/server.rs` states that
      the ignored tests, both of them, download their models.
- [x] `cargo fmt --all --check` and `cargo clippy --workspace
      --all-targets` pass.

### Task 2: Install a TypeScript with tsserver for the language server

Claude Code's `typescript-lsp` plugin starts `typescript-language-server`
with `/workspace` as its root, where it fails to start today. With the
global TypeScript from the 6.x line, it starts both in a new container
and in the running one; `post-create.sh` runs only when a container is
created, so the running container gets the same install by hand.

Review path: Security-Hybrid

- [ ] `.devcontainer/post-create.sh` installs `typescript@6` globally.
      Its comment says two things: the language server uses the global
      TypeScript because `/workspace` has no `node_modules/typescript`,
      and TypeScript 7 ships no `tsserver`, which the server needs.
- [ ] The "Plugins and Language Servers" section of
      `.devcontainer/README.md` names TypeScript 6 among the packages
      `post-create.sh` installs.
- [ ] `bash -n .devcontainer/post-create.sh` passes.
- [ ] In the running container, `npm ls -g typescript` reports a 6.x
      version.
- [ ] `typescript-language-server --stdio` started with `/workspace` as
      its root answers `initialize` without an error and returns
      document symbols for `crates/abyme-canvas/ui/src/App.tsx`; the
      handoff shows both responses.

## Decisions

- **`typescript@6`, not an exact version:** the pin exists for the
  major version; an exact one would need bumps with no benefit for the
  language server.
- **No `package.json` at the repository root:** it would add a Node
  project to the root only so that the language server finds
  TypeScript, next to the UI's pnpm project.
- **No TypeScript 7 native language server:** the official
  `typescript-lsp` plugin cannot start it.
- **`typescript-language-server` and `pyright` stay unpinned:** only
  `typescript` broke. Pinning the other two would guard against a
  breakage that has not happened, and someone would have to bump the
  pins later.
- **The shell mismatch is documented, not removed:** neither a shell
  override in `containerEnv` nor a fish wrapper around `claude` would
  cover every way Claude is started.

## Non-Goals

- Enabling `typescript-lsp` in the project's `.claude/settings.json`.
  It is enabled in the user's own settings, and that file is a
  blueprint file.
