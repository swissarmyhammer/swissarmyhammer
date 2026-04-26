---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffd980
title: Clean up turn diffs and state on allowed Stop, not SessionStart
---
## What

Turn diffs and state are currently cleaned at SessionStart, but SessionStart fires at the beginning of the Claude Code session — not between turns. Diffs from turn 1 leak into turn 2's Stop validators.

The fix: clean up at Stop, but ONLY when the stop is allowed (no validator blocked). When stop is blocked, the agent continues and may make more changes — diffs must survive for the next Stop.

### Current flow (broken):
- SessionStart: cleans diffs/state (but only fires once per session, not per turn)
- Stop: validators run, but diffs are never cleaned → accumulate across turns

### Desired flow:
- Stop: validators run → if allowed (no blocking failures), clean diffs + turn state for this session
- Stop blocked: diffs survive for the continued turn
- SessionStart: can still clean stale state from previous crashed sessions (optional, secondary)

### Implementation:

The cleanup must happen AFTER `ValidatorExecutorLink::process()` returns and the result is known. Two options:

**Option A: Post-validator chain link** — add a new chain link after ValidatorExecutorLink that reads the chain result and cleans up if no block occurred. This is tricky because chain links process sequentially and the block decision is in the ChainResult.

**Option B: Cleanup inside `handle_ruleset_results`** — after determining that no validator blocked, call cleanup. This is simpler and keeps the logic co-located.

**Option C: Cleanup in the strategy's `process()` method** — after `route_to_chain` returns for a Stop hook and the result is allowed, call cleanup. This is the highest-level option and cleanest separation.

Recommend **Option C** — the strategy already knows the hook type and the result.

### Files to modify:
- `avp-common/src/strategy/claude/strategy.rs` — After `route_to_chain` returns for Stop, if result is success (no block), call `turn_state.clear(session_id)` and `turn_state.clear_diffs(session_id)` and `turn_state.clear_pre_content(session_id)`
- `avp-common/src/chain/links/file_tracker.rs` — Remove diff/pre-content cleanup from `SessionStartCleanup` (or keep as stale-state fallback)
- `avp-common/src/chain/factory.rs` — May need to pass turn_state to the strategy or expose it

### Approach (TDD):
Use `/tdd` workflow. Write failing tests FIRST.

1. Write test: after allowed Stop, diffs are cleaned
2. Write test: after blocked Stop, diffs survive
3. Write test: second turn doesn't see first turn's diffs (after allowed Stop between them)

## Acceptance Criteria
- [ ] Diffs cleaned after allowed Stop (no validator blocks)
- [ ] Diffs survive after blocked Stop (validator forced continuation)
- [ ] Turn state (changed files) also cleaned on allowed Stop
- [ ] Pre-content cleaned on allowed Stop
- [ ] Second turn's Stop validators don't see first turn's diffs
- [ ] SessionStart still cleans stale state from crashed sessions (fallback)

## Tests
- [ ] Unit test: allowed Stop triggers cleanup
- [ ] Unit test: blocked Stop preserves diffs
- [ ] Integration test: two-turn sequence with cleanup between
- [ ] Run `cargo nextest run -p avp-common`"