---
assignees:
- claude-code
depends_on:
- 01KQ367HE0Z8ZSXY90CTT8QYGG
position_column: todo
position_ordinal: f680
project: acp-upgrade
title: Bump workspace dep agent-client-protocol = "0.11"
---
## What

Apply the version bump in workspace `Cargo.toml`:

- Edit `/Users/wballard/github/swissarmyhammer/swissarmyhammer-mcp/Cargo.toml` line:
  `agent-client-protocol = "0.10"` → `agent-client-protocol = "0.11"`
- Run `cargo update -p agent-client-protocol` to advance `Cargo.lock`.
- Confirm `Cargo.lock` resolves to `0.11.1` (current latest on crates.io).
- Do **not** add the `unstable` feature flag — see the optional follow-up task for that.

This task is intentionally tiny so dependents (per-crate rewrite tasks) have a single, atomic version bump to depend on.

## Spike note

The spike (01KQ367HE0Z8ZSXY90CTT8QYGG) already produced this exact bump on the `spike/acp-0.11` branch (commit `f206917c8`). This task can either cherry-pick that commit or re-do the change directly on the working branch — both produce the same diff (Cargo.toml line + Cargo.lock changes that pull in `agent-client-protocol-derive 0.11.0`, `futures-concurrency 7.7.1`, `futures-lite 2.6.1`, `jsonrpcmsg 0.1.2`, `pin-project 1.1.11`; remove `async-broadcast`).

## Acceptance Criteria
- [ ] `Cargo.toml` lists `agent-client-protocol = "0.11"`.
- [ ] `Cargo.lock` resolves `agent-client-protocol` to `0.11.1`.
- [ ] `cargo check --workspace --all-targets` runs (it WILL fail until per-crate tasks land — the spike captured that the first failure is `agent-client-protocol-extras` with 23 errors; downstream crates can't be checked while extras is broken).

## Tests
- [ ] No tests in this task; per-crate tasks restore green.

## Workflow
- Pure dependency bump. Do not modify any `.rs` files in this task — keep the diff minimal so subsequent per-crate rewrite tasks have a clean baseline.

## Depends on
- 01KQ367HE0Z8ZSXY90CTT8QYGG (spike completed — we now know what we're walking into).
