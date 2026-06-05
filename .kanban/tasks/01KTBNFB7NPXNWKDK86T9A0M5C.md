---
assignees:
- claude-code
depends_on: []
position_column: todo
position_ordinal: '8480'
project: local-review
title: 'Teardown: delete the avp-cli app (hook-processor binary)'
---
## What
Delete the `avp-cli` application entirely. Per its own `Cargo.toml` it is the "Agent Validator Protocol - Claude Code hook processor CLI" — i.e. the binary the Claude `hooks` config invokes per tool call. Retiring hook execution means this binary has no reason to exist.

- Remove `apps/avp-cli/` (the whole crate: `src/main.rs`, `src/lib.rs`, `src/doctor.rs`, `src/model/*`, `README.md`, build script).
- Remove `avp-cli` from the workspace `members` and any `[workspace.dependencies]` entry in the root `Cargo.toml`.
- Remove its cargo-dist/release wiring (the `[package.metadata.dist]` consumer, any cask/formula, completions registration, CI matrix entries referencing `avp`).
- Remove the `avp` hook install/uninstall surface this app provided (`avp install project|user`) and any docs that tell users to run it.
- Leave `avp-common` in place for now (it is renamed in a later task); this task only removes the app.

## Acceptance Criteria
- [ ] `apps/avp-cli/` no longer exists; `cargo build` and `cargo metadata` succeed with no reference to the `avp` crate/binary.
- [ ] No workspace member, dependency, completion, or release-config entry references `avp-cli`/`avp` binary.
- [ ] `rg -n "avp install|avp-cli|bin.*avp" ` finds only historical references in docs being removed (none in build/config).

## Tests
- [ ] `cargo build --workspace` green after removal.
- [ ] `cargo test --workspace` green (no test referenced the deleted binary; if any did, remove/relocate it).
- [ ] A grep assertion in CI or a smoke check: workspace metadata contains no `avp` binary target.

## Workflow
- Mechanical deletion task. No `/tdd`. Verify with a full `cargo build --workspace` + `cargo test --workspace`. Coordinate ordering: this precedes the hook-machinery removal and the crate rename.