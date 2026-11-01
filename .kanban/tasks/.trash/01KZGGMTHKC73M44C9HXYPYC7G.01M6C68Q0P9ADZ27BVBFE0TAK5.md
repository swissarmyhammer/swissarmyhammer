---
assignees:
- claude-code
position_column: todo
position_ordinal: ffa180
title: 'validator findings: crates/swissarmyhammer-validators/src/validators/pool.rs (pre-existing)'
---
Pre-existing validator findings surfaced by `review file crates/swissarmyhammer-validators/src/validators/pool.rs` on 2026-08-08. None of these lines belong to card ^811xj0q (warm prefix reuse telemetry); that card only changed lines 900-901, 908 and 1219-1222, swapping the literal `"cache_usage"` for `CacheUsage::META_KEY`. Filed separately so ^811xj0q is not blocked by code it never touched.

Findings whose subject is refactoring test code that already existed were dropped per the review skill's standing exception.

## Review Findings (2026-08-08 05:37)

- [ ] `crates/swissarmyhammer-validators/src/validators/pool.rs:576` — SessionPinGuard is a public struct with non-empty representation (agent: Option<ConnectionTo<Agent>>, session_id: SessionId) but does not implement or derive Debug. The documentation rule requires Debug implemented for all public types with non-empty representation. Add `#[derive(Debug)]` to SessionPinGuard, or hand-implement Debug if ConnectionTo<Agent> doesn't implement Debug.
- [ ] `crates/swissarmyhammer-validators/src/validators/pool.rs:876` — Near-identical error wrapping blocks differ only in the message string. Lines 876–878 and 940–942 both follow the pattern `claude_agent::AgentError::Internal(format!("failed to <context>: {}", e))` — this is one function with an argument waiting to be extracted. Extract a helper function: `fn wrap_internal_error(context: &str, e: impl std::fmt::Display) -> claude_agent::AgentError { claude_agent::AgentError::Internal(format!("failed to {}: {}", context, e)) }`. Call it as `.map_err(|e| wrap_internal_error("execute prompt", e))?` and `.map_err(|e| wrap_internal_error("create session", e))?` to eliminate duplication.