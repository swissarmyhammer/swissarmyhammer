---
assignees:
- claude-code
depends_on:
- 01KNCE4QYJ2WZJ4H4KWXNP7RR8
position_column: done
position_ordinal: ffffffffffffffffffdc80
title: Filter changed-file context passed to Stop validators by their file patterns
---
## What

When a Stop validator has `match.files` patterns, filter the `changed_files` list passed to the validator's render context so it only sees files matching its patterns. This gives validators focused context instead of every file changed in the turn.

### Files to modify:
- `avp-common/src/chain/links/validator_executor.rs` — After matching rulesets, filter the `changed_files` per-ruleset based on its `match.files` globs before passing to `execute_rulesets()`
- `avp-common/src/context.rs` — Update `execute_rulesets()` signature to accept per-ruleset changed files (or pass filtering info)
- `avp-common/src/validator/runner.rs` — Ensure the runner passes filtered files through to rule rendering

### Approach (TDD):
Use `/tdd` workflow. Write failing tests FIRST, then implement.

1. Write unit tests for a `filter_changed_files(patterns, files)` helper
2. Implement the helper using the existing glob matching logic
3. Write integration test for per-ruleset filtering in the executor
4. Wire the filtering into the execution pipeline

## Acceptance Criteria
- [ ] Stop validator for `*.rs` files only sees Rust files in its changed_files context
- [ ] Stop validator with no file patterns sees all changed files
- [ ] The hook_context YAML rendered for the validator only lists matching files

## Tests
- [ ] Unit test: filter_changed_files helper with patterns `[\"*.rs\"]` and files `[\"a.rs\", \"b.py\"]` returns `[\"a.rs\"]`
- [ ] Unit test: filter_changed_files with empty patterns returns all files
- [ ] Integration test: render context for a Stop ruleset shows only matching files
- [ ] Run `cargo nextest run -p avp-common`"