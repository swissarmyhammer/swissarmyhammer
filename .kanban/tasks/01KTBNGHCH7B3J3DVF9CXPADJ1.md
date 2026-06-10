---
assignees:
- claude-code
depends_on:
- 01KTBNFB7NPXNWKDK86T9A0M5C
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffee80
project: local-review
title: 'Teardown: retire AVP hook-execution machinery (keep loader + ACP executor)'
---
## What
Remove the hook-triggered execution path from `avp-common`. KEEP: rules-as-data loader (loader.rs/types.rs/parser.rs, now hook-free) + new hook-free shared bounded `AgentPool`.

## Execution Plan (inline teardown) — DONE
- [x] Phase 1 TDD: new `validator/pool.rs` — `AgentPool` (submit→Future, fixed workers draining one shared queue, backend-aware count local=1/remote=N, per-call token cap). RED watched (7 todo!() fails), GREEN (10 pool tests pass).
- [x] Phase 2: deleted chain/, strategy/, turn/, hooks/, types/, context.rs, install.rs, lockfile.rs, validator/executor.rs, validator/runner.rs. Stripped HookType/trigger/MatchContext.hook_type from types.rs, loader.rs, parser.rs, builtin/mod.rs. Trimmed lib.rs, error.rs, Cargo.toml (dropped unused hook deps; moved extras+futures to dev-deps).
- [x] Phase 3: deleted all hook integration test files + recording fixtures; loader/glob/precedence coverage retained as unit tests in loader.rs/types.rs/builtin.
- [x] Phase 4: fixed `agent-client-protocol-extras` — removed avp_schema_tests.rs, its mod line, avp_test_context helper + the now-unused HookCommandContext import, and the avp-common dev-dependency.
- [x] Phase 5: `cargo test -p avp-common` GREEN (128 unit + 2 doc, 0 failed, 0 warnings, exit 0); `cargo build --workspace` GREEN (exit 0).

## Acceptance Criteria — MET
- [x] No engine-crate code references `HookType`, PreToolUse/PostToolUse/Stop, turn-diff sidecars, or hook stdin/stdout protocol (only `triggerMatcher` regex feature retained; no HookType type anywhere).
- [x] A hook-free `AgentPool` exists: submit at any time; fixed worker count drains one shared queue; worker count backend-aware (PoolConfig::local=1, ::remote=N/AIMD flag, ::with_concurrency override); per-call token cap retained.
- [x] All consumers updated / dead surface removed; `cargo build --workspace` green.
- [x] No real-time validation fires on tool calls (hook system gone).

## Tests — PASS
- [x] Loader tests (precedence, glob match, tool/file/changed-files matching) retained as unit tests — green.
- [x] AgentPool tests (mock harness PlaybackAgent + MockAgent-based PeakProbe/Erroring/MetaRecording): all M results returned, never >N in flight, pipelining mid-drain, local=1 strictly serial, erroring task no-deadlock, per-call max_tokens cap — green.
- [x] `cargo test -p avp-common` and `cargo build --workspace` green.

## Note for reviewer
The only non-test crate that depended on avp-common was `agent-client-protocol-extras` (dev-dep only); its lone consumer was the schema-compatibility test `avp_schema_tests.rs` that asserted extras' HookEvent JSON deserializes through the now-deleted `avp_common::HookInput`. With AVP's hook schema gone that test's reason to exist is gone, so it (and the dev-dep) were removed; extras keeps its own independent hook system. Loader precedence + RuleSet directory loading are now covered by unit tests (loader.rs, builtin/mod.rs) rather than the deleted hook-coupled integration files.

## Review Findings (2026-06-05 11:19)

Verified independently: `cargo test -p avp-common` (128 unit + 2 doc, 0 failed), `cargo build --workspace` (exit 0), `cargo clippy -p avp-common` (0 warnings), `cargo test -p agent-client-protocol-extras` (all green). Confirmed no `HookType` / `HookCommandContext` / `HookInput` enum or struct survives in `avp-common`; the retained `triggerMatcher` is a plain `Option<String>` regex field, not a resurrected hook type. The `AgentPool` is well-engineered — the receiver `Mutex` is held only across `recv` (not across `run_prompt`), shutdown is handled gracefully on a closed queue, and the test suite proves the in-flight cap, pipelining, local serialization, no-deadlock-on-error, and the per-call token cap. Core teardown is clean. No blockers.

### Warnings
- [x] `crates/agent-client-protocol-extras/src/recording.rs` (`all_avp_recording_fixtures_deserialize`) — this test loops over `avp-common/tests/fixtures/recordings/*.json`, but this task **deleted** every one of those fixtures. The test guards with `if !fixtures_dir.exists() { return; }`, so it now early-returns and never reaches its `assert!(checked > 0)` — it reports green while exercising nothing. The teardown hollowed out this sibling test; it now silently passes on empty input, a maintenance trap. Fix: either remove the test along with the fixtures it validated, or repoint it at a surviving fixture source so the `checked > 0` guard is actually reachable.
  - RESOLVED: Removed the hollowed-out `all_avp_recording_fixtures_deserialize` test. The on-disk-format-stability guarantee it provided is already covered by the sibling `legacy_existing_fixture_deserializes`, which inlines a real legacy fixture (`initialize`/`new_session`/`prompt` with a notification) and asserts it parses with the current `RecordedSession`/`RecordedCall` types — no dependency on deleted files, so the coverage is genuinely exercised. Left an explanatory NOTE comment in place of the deleted test. Verified: `cargo test -p agent-client-protocol-extras` 207 lib + others, 0 failed.

### Nits
- [x] `crates/avp-common/src/validator/pool.rs` (`run_prompt` doc comment) — says "Drive a single prompt turn against an ACP 0.11 agent connection", but the workspace now builds against `agent-client-protocol` 0.12.1. Update the version in the doc comment.
  - RESOLVED: Changed the `run_prompt` doc to "ACP 0.12" (matching `agent-client-protocol` 0.12.1 in Cargo.lock).
- [x] `crates/avp-common/src/validator/pool.rs` (`PoolConfig::aimd`) — the `aimd` field is set by `local()`/`remote()`/`with_concurrency()` and asserted in tests, but never read by `AgentPool::new` or `worker_loop`; no AIMD adjustment logic exists despite the module doc claiming the count is "AIMD-adjusted to discover the API ceiling". This is an intentional placeholder flag per the acceptance criteria (scoped for a follow-up task that consumes the pool), so not dead code to remove — but a brief `// reserved for AIMD; not yet consumed` note on the field would prevent a future reader mistaking it for wired-up behavior.
  - RESOLVED: Added a `// reserved for AIMD; not yet consumed` note on the `aimd` field explaining it is set by the constructors and asserted in tests but not read by `AgentPool::new`/`worker_loop`. Also softened the module-level doc so it no longer claims the count is "AIMD-adjusted" today — it now states the flag is reserved and the adaptive logic is not yet wired up, keeping doc and field consistent. Verified: `cargo clippy -p avp-common` exit 0 (no warnings), `cargo test -p avp-common` 128 unit + 2 doc, 0 failed.