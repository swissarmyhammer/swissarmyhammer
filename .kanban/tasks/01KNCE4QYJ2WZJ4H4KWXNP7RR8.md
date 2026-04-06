---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffd980
title: Enable file glob matching for Stop-triggered validators
---
## What

Change `matches_files()` in `avp-common/src/validator/types.rs` (both `Validator` and `RuleSet` impls) to support file matching for Stop hooks against accumulated changed files.

Currently (line 424 and 811), Stop hooks bypass file matching entirely:
```rust
if match_criteria.files.is_empty() || ctx.hook_type == HookType::Stop {
    return true;
}
```

Instead, when `hook_type == HookType::Stop` and `match_criteria.files` is non-empty, match the glob patterns against a new `changed_files` field on `MatchContext`.

### Files to modify:
- `avp-common/src/validator/types.rs` — Add `changed_files: Option<Vec<String>>` to `MatchContext`, update both `matches_files()` impls
- `avp-common/src/chain/links/validator_executor.rs` — Populate `changed_files` on `MatchContext` when building it for Stop hooks

### Approach (TDD):
Use `/tdd` workflow. Write failing tests FIRST, then implement.

1. Write unit tests for the new `matches_files()` behavior with Stop hooks
2. Add `changed_files: Option<Vec<String>>` to `MatchContext` struct
3. In both `matches_files()` impls: when `hook_type == Stop` and files patterns exist, check if ANY changed file matches ANY glob pattern (rather than checking a single `ctx.file_path`)
4. In `build_match_context()` or the `process()` method, populate `changed_files` from turn state for Stop hooks

## Acceptance Criteria
- [ ] Stop validators with `match.files` patterns only run when at least one changed file matches a pattern
- [ ] Stop validators without `match.files` still run unconditionally (backward compat)
- [ ] `MatchContext` has `changed_files` field populated for Stop hooks

## Tests
- [ ] Unit test in `types.rs`: Stop hook with `files: [\"*.rs\"]` + changed_files `[\"foo.rs\"]` → matches
- [ ] Unit test: Stop hook with `files: [\"*.rs\"]` + changed_files `[\"foo.py\"]` → no match
- [ ] Unit test: Stop hook with empty `files` + any changed_files → matches (backward compat)
- [ ] Unit test: Stop hook with `files: [\"*.rs\"]` + no changed_files → no match
- [ ] Run `cargo nextest run -p avp-common`"