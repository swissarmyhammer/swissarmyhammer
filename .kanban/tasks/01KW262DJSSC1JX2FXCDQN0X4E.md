---
assignees:
- claude-code
depends_on:
- 01KW25YZ4MKNR09RXYR1B4S05T
- 01KW25ZW4NED0J1BD77HPK7DNX
position_column: todo
position_ordinal: aa80
project: expect
title: 'cli surface adapter: provision + drive + observe + teardown'
---
## What
The first surface adapter — `cli` — the deterministic, no-agent path. A `SurfaceAdapter` trait + a `CliAdapter` impl that provisions the SUT, drives it mechanically, and observes authoritative state. Per `ideas/expect.md` §"Surface adapters" (cli row) and §"Provisioning and Isolation".

- New `crates/swissarmyhammer-expect/src/surface/mod.rs` defining the `SurfaceAdapter` trait:
  - `provision(&self, setup, repo_root) -> ProvisionedSut` (build + ready the binary; uses `setup:` or falls back to detected build/launch).
  - `drive(&self, sut, when_step) -> ()` (cause the transition).
  - `observe(&self, sut) -> SurfaceState` (authoritative read).
  - `teardown(&self, sut)`.
- New `crates/swissarmyhammer-expect/src/surface/cli.rs` — `CliAdapter`: build via `detected-projects` (`crates/swissarmyhammer-project-detection`) → `ProjectType`, with expect's own `ProjectType → {build, launch}` command map (the structured commands don't exist in project-detection; mirror the consumer pattern in `code_context/detect.rs:229`). Drive = run argv (`std::process`); observe = capture stdout/stderr/exit code + named output files into a cli `SurfaceState`. Honor the spec `timeout`.
- Provisioning lifecycle owned by expect (build now, not "whatever's running").

## Acceptance Criteria
- [ ] Against a trivial fixture CLI (e.g. a tiny `echo`-like script or a built binary), `CliAdapter` provisions, runs an argv, and observes stdout/stderr/exit in a `SurfaceState`.
- [ ] `setup:` declaration overrides auto-detected build/launch; absent `setup:` falls back to detected commands.
- [ ] Teardown cleans up any provisioned scratch state.
- [ ] A run exceeding `timeout` is aborted and surfaced as an error (not a hang).

## Tests
- [ ] `crates/swissarmyhammer-expect/src/surface/cli.rs` integration test driving a real fixture command in a `tempfile` dir, asserting captured stdout/exit.
- [ ] Timeout test: a sleep-longer-than-timeout command returns a timeout error.
- [ ] `cargo nextest run -p swissarmyhammer-expect cli_adapter` passes.

## Workflow
- Use `/tdd`.