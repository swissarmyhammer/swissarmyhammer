---
assignees:
- claude-code
position_column: todo
position_ordinal: '8380'
title: 'Decide: keep or delete intentionally #[ignore]d llama-agent test'
---
**File**: `crates/llama-agent/tests/integration/tool_use_multi_turn.rs:356`

**Status**: `#[ignore = "Qwen3-0.6B too small for conditional-verdict tool dispatch — opt-in via --run-ignored=all; see docstring for context"]`

**Context**: Test is gated on a model that the project's default test runner cannot load. Counts as 1 skipped test in the nextest summary.

**Choice**:
1. Delete the test (dead code per strict zero-skip policy), OR
2. Convert from `#[ignore]` to a runtime check that skips the test cleanly with `eprintln!` + early return when the model is unavailable (this no longer counts as ignored, but provides equivalent zero-coverage), OR
3. Move it behind a cargo feature (e.g. `cfg_attr(not(feature = "large-model-tests"), ignore)`) so it doesn't show up as skipped by default.

**Acceptance criteria**: Either the test is deleted, OR `cargo nextest run --workspace` reports `0 skipped` for this test.

**Pre-existing**: predates this branch (last touched in `Squash merge mcp into main`).

#test-failure