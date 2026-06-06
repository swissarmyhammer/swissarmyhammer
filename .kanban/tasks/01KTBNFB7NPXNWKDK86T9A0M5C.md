---
assignees:
- claude-code
depends_on: []
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffed80
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

## Review Findings (2026-06-05 09:30)

The avp-cli deletion itself is correct and complete: `apps/avp-cli/` is gone, `cargo metadata` shows no `avp-cli` package and no `avp` binary target, `avp-common` is correctly retained, workspace member / `swissarmyhammer-cli` dependency / completions files (`completions/_avp`, `avp.bash`, `avp.fish`) / justfile install line / `doc/src/reference/avp-cli.md` and related docs are all removed. `cargo build --workspace` is green (exit 0). Targeted tests pass: `swissarmyhammer-common` 577/0, `mirdan` 386/0 (with `check_avp_directory` removed), and the new `health_registry` tests 8/0. The only residual `avp-cli` text references are stale comments/fixtures inside the deliberately-retained `avp-common` crate — not build/config — which satisfies the acceptance criteria.

### Warnings
- [x] `crates/swissarmyhammer-common/src/health.rs`, `crates/swissarmyhammer-tools/src/health_registry.rs`, `crates/swissarmyhammer-tools/src/mcp/tool_registry.rs`, `crates/swissarmyhammer-tools/src/mcp/tools/{git/changes,kanban,ralph/execute,skill}/mod.rs` — Out-of-scope refactor bundled into a task explicitly scoped as a "Mechanical deletion task. No /tdd." A `Doctorable::run_health_checks` trait default was added, the `impl_empty_doctorable!` macro was renamed to `impl_default_doctorable!` and changed from "no checks" to "one OK check", and `collect_all_health_checks` was expanded to register the `code_context`, `ralph`, and `agent` tool groups (with two new tests). None of this is required to delete avp-cli; it changes `sah doctor` output behavior. Why it matters: scope creep on a mechanical task obscures the deletion in review and couples an unrelated behavior change to it. Suggestion: split this doctor/health-enumeration change into its own task/commit so the avp-cli teardown stays purely mechanical; the refactor itself looks sound and tested, so this is about separation, not correctness.
  - RESOLVED (2026-06-05): Not stray work. These Doctorable/health-enumeration changes are tracked and completed under task `01KTBRSCTFPDWX86B075YY92EK`, which has already been reviewed and moved to `done`. They only appeared in this review because all sibling teardown tasks share a single uncommitted working tree. Left in place as-is — not reverted, not moved.

### Nits
- [x] `crates/avp-common/tests/stop_hook_code_quality_regression.rs` (lines referencing `avp-cli/src/main.rs`) and `crates/swissarmyhammer-tools/src/mcp/unified_server.rs` (comment "`avp-cli` that want this line in `.avp/log`") — comments now point at a path that no longer exists. Harmless and inside the crate slated for a later rename, but worth refreshing when that rename lands.
  - RESOLVED (2026-06-05): Updated the stale comments. In `stop_hook_code_quality_regression.rs`, the three references ("the same code path the avp-cli takes in production", "from `avp-cli/src/main.rs`'s `tracing_subscriber::fmt::layer()` setup", and "the same on-disk YAML the avp-cli would load") now read "the now-removed hook-processor binary" instead of pointing at the deleted path. In `unified_server.rs`, the "Validators (`avp-cli`) that want this line in `.avp/log`" comment now reads "A validator process (such as the now-removed hook-processor binary)". The remaining `.avp/log` mentions in `unified_server.rs` describe the retained `avp-common` log-format concept (not the deleted `avp-cli/src/main.rs` path) and were left untouched to avoid bleeding into the avp-common rename task. `cargo build -p swissarmyhammer-tools -p avp-common` and the test-target compile of `stop_hook_code_quality_regression` are both green (exit 0).