---
assignees:
- claude-code
position_column: todo
position_ordinal: b480
title: 'Coverage: daemon.rs health_check + restart_with_backoff edge cases'
---
## What

`swissarmyhammer-lsp/src/daemon.rs` — `health_check` and `restart_with_backoff` have uncovered branches: stale process detection logic and backoff sleep branch. These are edge-case integration scenarios.

## Acceptance Criteria
- [ ] Test exercises stale process detection path in `health_check`
- [ ] Test exercises backoff sleep branch in `restart_with_backoff`

## Tests
- [ ] Add integration test in `swissarmyhammer-lsp/tests/` that sets up a daemon with a process that becomes stale, verifies health_check detects it
- [ ] Add test that triggers restart_with_backoff and verifies backoff behavior
- [ ] `cargo test -p swissarmyhammer-lsp health_check` passes
- [ ] `cargo test -p swissarmyhammer-lsp restart` passes

## Workflow
- Use `/tdd` — write failing tests first, then implement to make them pass. #coverage-gap