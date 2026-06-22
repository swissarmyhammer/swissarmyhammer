---
assignees:
- claude-code
comments:
- actor: wballard
  id: 01kvgdff1mr7s32zq3nyysq2ba
  text: |-
    Picked up. Research done.

    Key findings:
    - CacheUsage lives at `claude_agent::protocol_translator::CacheUsage` (NOT re-exported at crate root despite task wording `claude_agent::CacheUsage`). Fields: cache_read_input_tokens, cache_creation_input_tokens, input_tokens, output_tokens — all Option<u64>. Will reference via the module path.
    - Wire contract for cache_usage over ACP: production puts it in PromptResponse._meta under key "cache_usage" as `usage.to_meta_json()` (claude-agent/src/agent_prompt_handling.rs build_streaming_response), and reads it back via `meta.get("cache_usage")` + `CacheUsage::from_meta_json` (lib.rs execute_prompt_with_agent). I'll mirror this exactly in pool.rs run_prompt.
    - pool.rs `run_prompt` already holds `prompt_response` (PromptResponse) — extract cache_usage from its .meta there, populate SessionTurn. Also wire turn.cache_usage into Respond::deliver Collected branch (currently `cache_usage: None` placeholder).
    - Fleet test harness: ScriptedAgent in review/test_support.rs builds PromptResponse::new(StopReason::EndTurn) with no _meta. To exercise claude-style cache usage through collect_forked_task I'll add a `cache_usage: Option<CacheUsage>` knob to ScriptedAgentConfig and attach it to the response _meta via to_meta_json, same wire shape as production.

    Plan (TDD): RED tests for classify_reuse table + SessionTurn.cache_usage propagation; then implement classifier in fleet.rs + threading in pool.rs; then log in collect_forked_task, collect_task, verify collect_verify; then fleet-level collect_forked_task WarmCache test.
  timestamp: 2026-06-19T17:04:55.348769+00:00
- actor: wballard
  id: 01kvgdty7jfqjdbyd42t29mx2m
  text: |-
    Implementation landed (TDD RED->GREEN verified).

    RED: the 6 new tests failed to compile (classify_reuse/PrefixReuse undeclared, SessionTurn.cache_usage unknown field). GREEN: all 6 pass.

    Changes:
    - pool.rs: added SessionTurn.cache_usage (Option<claude_agent::protocol_translator::CacheUsage>); populate from prompt_response._meta["cache_usage"] via CacheUsage::from_meta_json in run_prompt; wired turn.cache_usage into Respond::deliver (replaced the cache_usage:None placeholder). Added CacheUsageAgent mock + propagation test.
    - fleet.rs: added PrefixReuse enum (WarmKv/WarmCache/Cold) + pure classify_reuse(fork, usage) with precedence native-KV > claude-cache-read > cold; label()/reused_tokens()/cache_read()/cache_created() helpers. Logged classification in collect_forked_task and the monolithic collect_task. Added classify_reuse table tests + a fleet-level test exercising the real collect_forked_task path with claude-shape cache usage (DegradedAttach fork + cache_usage with reads -> WarmCache).
    - verify.rs: collect_verify Forked branch now classifies + logs reuse.
    - test_support.rs: ScriptedAgentConfig.cache_usage knob attaches cache_usage to PromptResponse._meta (same wire shape as production).
    - Updated 2 existing log-string assertions (fleet warm-fork test, verify warm-fork test) to the new "prefix reuse" / reuse=\"warm KV fork\" log lines; the degraded-cold assertions still match (message retained).

    Next: full `cargo nextest run -p swissarmyhammer-validators` + clippy.
  timestamp: 2026-06-19T17:11:11.346331+00:00
- actor: wballard
  id: 01kvge4djkp56r3seaccfezpq9
  text: |-
    really-done verification complete:
    - cargo nextest run -p swissarmyhammer-validators: 312 passed, 0 skipped.
    - cargo clippy -p swissarmyhammer-validators --all-targets -- -D warnings: clean, exit 0.
    - TDD RED->GREEN evidence captured (the 6 new tests failed to compile before impl; all pass after).
    - Adversarial double-check: PASS. Confirmed all 4 acceptance criteria met and no regression to existing warm/cold log assertions (precedence native-KV > claude-cache-read > cold is documented + intentional; backends mutually exclusive in practice). Fixed the one non-blocking doc nit it raised (cold-write test comment said Cold "carries the created count" — Cold is a unit variant; reworded). Re-ran classify_reuse tests after the fix: 4 passed.

    Moving to review.
  timestamp: 2026-06-19T17:16:21.971035+00:00
depends_on:
- 01KVG6Y2EVGDAH85YJG3MRHXNT
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffffc780
project: local-review
title: 'review fleet: log warm/cold prompt-cache reuse per validator task'
---
## What

The review fleet primes a shared file-context prefix once and forks one session per validator. On the **llama/qwen** backend a fork reports `prefix_tokens` (reused token count) and `collect_forked_task` logs warm vs cold (`crates/swissarmyhammer-validators/src/review/fleet.rs:486-501`). On the **claude** backend, fork attaches no token counts (`ForkAttachment.prefix_tokens == None`, `pin` is a no-op) — so that same log is **blind**: we cannot tell whether the parallel validator forks are getting Anthropic prompt-cache **reads** (warm) or **cold writes**.

With the dependency task exposing `CollectedResponse.cache_usage` (Anthropic `cache_read_input_tokens` / `cache_creation_input_tokens`), thread that onto `SessionTurn` and log it per validator task so warm/cold reuse is observable on Claude.

Plumbing:
1. Add `cache_usage: Option<claude_agent::CacheUsage>` to `SessionTurn` (`crates/swissarmyhammer-validators/src/validators/pool.rs:222`) and populate it from `CollectedResponse.cache_usage` where the turn is assembled (the `Ok(SessionTurn { .. })` site, ~`pool.rs:851`).
2. Add a small pure classifier so the warm/cold decision is unit-testable without asserting on log strings, e.g.:
   ```rust
   enum PrefixReuse { WarmKv { reused_tokens: u64 }, WarmCache { read: u64, created: u64 }, Cold }
   fn classify_reuse(fork: Option<ForkAttachment>, usage: Option<CacheUsage>) -> PrefixReuse
   ```
   - native KV fork with `prefix_tokens: Some(n)` → `WarmKv`
   - claude with `cache_read_input_tokens > 0` → `WarmCache`
   - `cache_creation_input_tokens > 0` and no reads → `Cold` (cold write)
   - otherwise `Cold`/unknown.
3. In `collect_forked_task` (`fleet.rs:486-501`) call `classify_reuse(turn.fork, turn.cache_usage)` and log it (`tracing::info!`) with `validator`, `session`, and the cache read/created counts. Mirror the same log in the monolithic `collect_task` path and in the verify pass (`crates/swissarmyhammer-validators/src/review/verify.rs`) so primed and verify turns also report cache usage.

Out of scope: changing what `claude-agent` emits (that is the dependency task); altering pool concurrency or pin behavior; any caching policy change. This is observability only.

## Acceptance Criteria
- [ ] `SessionTurn` carries `cache_usage`, populated from `CollectedResponse`.
- [ ] A forked validator turn on the claude backend logs `cache_read_input_tokens` and `cache_creation_input_tokens` (warm vs cold), even though `fork.prefix_tokens` is `None`.
- [ ] The llama/qwen path still logs the native `prefix_tokens` reuse (no regression to the existing warm/cold log).
- [ ] `classify_reuse` is a pure function with the documented mapping and is unit-tested.

## Tests
- [ ] `test_classify_reuse_*` (in `fleet.rs` tests): KV fork → `WarmKv`; claude cache read → `WarmCache`; cold write → `Cold`; empty → `Cold`. Pure-function table test, no agent needed.
- [ ] `pool.rs` test: extend the existing mock turn (around `pool.rs:2060`, where `state_attached`/`prefix_tokens` are set) to assert `SessionTurn.cache_usage` propagates from a `CollectedResponse` carrying cache usage.
- [ ] A fleet-level test (reuse the existing fake `AgentPool`/agent harness the fleet tests already use) asserting a forked task with claude-style `cache_usage` resolves through `collect_forked_task` without error and classifies `WarmCache` — exercising the real `collect_forked_task` path, not just the classifier.
- [ ] Run: `cargo nextest run -p swissarmyhammer-validators` — all green (cargo nextest mandated, never bare cargo test). `cargo clippy -p swissarmyhammer-validators --all-targets -- -D warnings` clean.

## Workflow
- Use `/tdd` — write the `classify_reuse` table test and the `SessionTurn.cache_usage` propagation test first (RED), then implement the threading + classifier to green. Depends on the `claude-agent` cache-usage task being done first.