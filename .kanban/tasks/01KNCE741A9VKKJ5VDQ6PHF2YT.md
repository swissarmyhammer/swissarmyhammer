---
assignees:
- claude-code
depends_on:
- 01KNCE5ZJ49DZHB4FM7H1747PE
- 01KNCE5AHN1SHZHXKMC38AP5AK
position_column: done
position_ordinal: ffffffffffffffffffde80
title: Update Stop validator context rendering to include filtered diffs
---
## What

Wire the sidecar diffs (from card 3) and per-ruleset file filtering (from card 2) together so ALL validators — both PostToolUse and Stop — receive context with diffs from the unified sidecar source, filtered to only files matching their patterns.

### Files to modify:
- `avp-common/src/chain/links/validator_executor.rs` — In `process()` for all hook types: load diffs from sidecar files, filter by ruleset file patterns, pass filtered diffs to context preparation. For PostToolUse: load just the current file's diff. For Stop: load all diffs, filter per-ruleset.
- `avp-common/src/turn/diff.rs` — May need a helper to build `FileDiff` structs from the stored diff text map. Update or remove `CTX_FILE_DIFFS` usage if fully replaced by sidecar reads.

### Approach (TDD):
Use `/tdd` workflow. Write failing tests FIRST, then implement.

1. Write unit test: given stored sidecar diffs for `[a.rs, b.py, c.rs]` and patterns `[\"*.rs\"]`, rendered context only includes diffs for `a.rs` and `c.rs`
2. Write unit test: no file patterns → all diffs included
3. Write unit test: PostToolUse validator reads current file's diff from sidecar
4. Implement the filtering and wiring in validator_executor
5. Verify rendered output includes fenced diff blocks for matching files only

## Acceptance Criteria
- [ ] Stop validator for `*.rs` files sees only Rust file diffs in context
- [ ] PostToolUse validator sees the current file's diff from sidecar (not ChainContext)
- [ ] Diff text is rendered as fenced diff blocks in the validator prompt
- [ ] Validators without file patterns see all diffs
- [ ] `CTX_FILE_DIFFS` ChainContext usage removed or deprecated in favor of sidecar reads

## Tests
- [ ] Unit test: filtered diff rendering with mixed file types (Stop)
- [ ] Unit test: single-file diff rendering (PostToolUse)
- [ ] Unit test: no patterns → all diffs pass through
- [ ] Run `cargo nextest run -p avp-common`"