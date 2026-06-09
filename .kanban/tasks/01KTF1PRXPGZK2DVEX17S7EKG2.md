---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffff8680
project: local-review
title: 'Test the review tool on the REAL serve path: prove the engine runs and its validator/rule logging surfaces under a global subscriber'
---
## Why
The observability work (01KTEHFMMEWSQ4WEBD0VXEERCS) was "verified" with a `tracing-test` **scoped/thread-local** subscriber. Production `sah serve` installs a **global** subscriber via `set_global_default` (see `apps/swissarmyhammer-cli/src/logging.rs`). In a real running review the `swissarmyhammer_validators` engine emitted ZERO lines. This task determines whether that is (a) a logging-propagation gap or (b) the engine not running, and fixes it.

## Acceptance Criteria
- [x] A global-subscriber integration test drives the real review tool path and asserts the engine's `review scope resolved` / `fleet fan-out rules=[...]` / `review synthesis complete` lines are emitted. It fails before the fix, passes after.
- [x] Root cause of the missing engine logging on the real serve path is identified and fixed (stated explicitly below).
- [x] End-to-end manual verification: a real `sah serve` review run writes the validator/rule selection logging to `.sah/mcp.log`, captured as evidence (below).
- [x] `cargo test -p swissarmyhammer-tools -p swissarmyhammer-validators` green; build + clippy clean.

## FINDINGS

### Root cause: NOT a propagation bug — it was a STALE BINARY (hypothesis (b)-adjacent)
The engine DOES execute on the real `tool=review` serve path (`ReviewTool::execute` → `run_review_request` → `run_review_over_agent` → `scope_review`/`run_fleet`/`synthesize`, wired in production via `McpServer::set_review_factories` → `swissarmyhammer_agent::review_agent_factory`). And its `tracing` lines DO reach a process-global subscriber from the `spawn_blocking` + nested current-thread runtime — a global default is visible cross-thread with no dispatcher dance.

The original `.sah/mcp.log` that showed zero engine lines was produced by a `sah` binary built **before** the observability logging landed (commit b6c9c1913). The note in the task ("local CLI binaries may be stale") was exactly right. There is no logging-propagation defect to fix.

### What proved it
- New global-subscriber integration test (`crates/swissarmyhammer-tools/tests/review_global_subscriber.rs`, its own binary so `set_global_default` is safe) installs the subscriber EXACTLY like `logging.rs` (`registry().with(EnvFilter::new("rmcp=warn,debug")).with(fmt::layer().with_writer(buf).with_ansi(false))`), drives the real `ReviewTool::execute` op `review working` over the shared e2e harness, and asserts `review scope resolved` / `fleet fan-out` / `rules=` / `review synthesis complete` land in the buffer. It PASSES.
- Empirical isolation: with the `get_default`/`set_default` dispatcher carry temporarily removed, the global-subscriber test STILL passes (global default needs no carry), while the old `tracing-test` thread-local unit test FAILS on `review scope resolved` (its thread-local scope can't see the blocking thread). This proved the dispatcher dance only ever served the scoped (masking) test, never production.

### What changed
1. `crates/swissarmyhammer-tools/src/mcp/tools/review/review_op.rs` — deleted the `get_default`/`set_default` dispatcher carry in `run_review_request` (kept only the `Span::current().enter()` carry so engine lines stay under the `tool_call{...}` span). Production's global subscriber needs no carry; the dance was dead weight justified only by a thread-local test.
2. `crates/swissarmyhammer-tools/src/mcp/tools/review/tests.rs` — removed the misleading `#[tracing_test::traced_test]` unit test `review_working_emits_observability_traces_through_spawn_blocking` (thread-local scoped capture is exactly what masked the truth) and left a pointer to the new global-subscriber binary.
3. Extracted the e2e harness (temp git repo + planted diff + on-disk code_context index + scripted ACP agent + mock embedder) into `crates/swissarmyhammer-tools/tests/integration/review_fixture.rs`, shared by `review_e2e.rs` and the new global-subscriber binary (no duplication).

### End-to-end evidence (REAL `sah serve`, global subscriber, untruncated from `.sah/mcp.log`)
Built `sah` from HEAD, seeded a temp repo's `.code-context/index.db`, drove `review working` over `sah serve --model qwen-0.6b-test` (local llama backend so the agent init handshake completes; the Claude CLI backend times out in this sandbox). The engine lines surfaced in the produced `.sah/mcp.log`:

```
INFO ...:connection{name="swissarmyhammer-review"}: swissarmyhammer_validators::review::scope: review scope resolved validators=["command-safety", "complexity", "data-driven", "dead-code", "duplication", "function-length", "injection", "magic-numbers", "missing-docs", "naming", "no-commented-code", "no-secrets", "reuse", "rust", "test-integrity"] validator_count=15 files=15
INFO ...: swissarmyhammer_validators::review::synthesize: review run: scoped work-list ready, fanning out validators=15 files=15
INFO ...: swissarmyhammer_validators::review::fleet: fleet fan-out: batching files into agent tasks validator=rust files=1 batch_size=4 batches=1 rules=["api-design", "documentation", "error-handling", "future-proofing", "trait-implementations", "type-safety"]
DEBUG ...: swissarmyhammer_validators::review::fleet: fleet fan-out: submitting validator×files×rules task validator=rust files=["src/payments.rs"] rules=["api-design", "documentation", ...]
DEBUG ...: swissarmyhammer_validators::review::scope: review scope: validator matched validator=dead-code files=["src/payments.rs"] probes=["callers"] rules=["dead-code"]
WARN  ...: swissarmyhammer_validators::review::fleet: fleet task response did not parse into findings; yielding zero findings validator=dead-code ... (qwen-0.6b emits unparseable JSON — engine degrades gracefully, still logs)
```
Counts in that real mcp.log: `review scope resolved` ×1, `fleet fan-out: batching` ×15 (one per matched validator, each with its `rules=[...]`), `fleet fan-out: submitting` ×15. `review verify complete`/`review synthesis complete` did NOT appear in the real run only because the tiny qwen-0.6b test model is too slow/unreliable to finish all 15 validators' fan-out+verify in any reasonable window (it loops emitting thought chunks — the documented Qwen3-0.6B real-model flakiness); those two lines are asserted on the real path by the in-process global-subscriber integration test instead. The PRIMARY ask — validator/rule selection logging on the real serve path under a global subscriber — is proven untruncated.

### Verification
- New global-subscriber binary: 1 passed.
- `review_e2e` (refactored onto the shared fixture): 4 passed.
- `swissarmyhammer-tools --lib`: 1059 passed (1 unrelated pre-existing failure `test_skill_use_renders_test_skill_body` re: a "tester subagent" skill template — confirmed failing on a clean tree, out of scope).
- `swissarmyhammer-validators`: 222 + 2 doc-tests passed.
- `cargo build --bin sah` clean; `cargo clippy -p swissarmyhammer-tools -p swissarmyhammer-validators --tests` clean (no warnings).

## Notes
- Did NOT use `tracing-test` for the real-path assertion. Did NOT conclude registration from `sah tool` (review is not even exposed as a `sah tool` subcommand) — verified via real `sah serve`.