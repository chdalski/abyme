**Repository:** root
**Status:** NotStarted
**Created:** 2026-10-27

## Goal

A devcontainer with a memory limit has no swap on top of it, and a cargo
build with one job per CPU links several large test binaries at once and
exceeds the limit, which restarts the container. Document how to match
cargo's number of jobs to the memory limit, so developers and agents set it
once instead of passing `-j` to every run.

## Context

- `.devcontainer/docker-compose.yml` sets `mem_limit` and `memswap_limit`
  from `DEVCONTAINER_MEMORY` in the personal `.devcontainer/.env` (commit
  9793198), so the limit covers memory and swap together.
- Cargo reads the number of parallel jobs from the environment variable
  `CARGO_BUILD_JOBS`: https://doc.rust-lang.org/cargo/reference/config.html#buildjobs.
  `.devcontainer/.env` passes its keys into the container's environment.

## Steps

- [ ] Document the cargo job limit for a devcontainer with a memory limit

## Tasks

### Task 1: Document the cargo job limit for a devcontainer with a memory limit

Keep cargo within the container's memory cap, which allows no swap.

Review path: Reviewer-only

- [ ] A developer who sets `DEVCONTAINER_MEMORY` learns from the
      devcontainer README that the cap includes swap and that cargo then
      needs `CARGO_BUILD_JOBS` in `.devcontainer/.env`. The README's
      settings list and example `.env` show `CARGO_BUILD_JOBS=3` next to
      `DEVCONTAINER_MEMORY=8g`, the value that runs this workspace's builds
      and tests within 8 GiB, and give no rule for other caps. The comment
      on the limits in `docker-compose.yml` says the memory cap includes
      swap.
- [ ] The root `CLAUDE.md` Conventions tell an agent that in a container
      with a memory limit, cargo takes its job limit from `CARGO_BUILD_JOBS`
      in `.devcontainer/.env`, and that an agent which finds it unset passes
      `-j 3` to its cargo runs and asks the user to set it.

## Decisions

- **The job limit is personal, in `.devcontainer/.env`:** it belongs to the
  memory limit, which is personal too; a committed default would slow
  containers without a limit.

## Non-Goals

- Computing the job limit from the memory limit automatically.
