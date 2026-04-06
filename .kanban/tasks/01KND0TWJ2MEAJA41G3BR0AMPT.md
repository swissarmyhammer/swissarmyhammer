---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffe080
title: 'Integration test: session-scoped diff isolation across two sessions'
---
## What

Unit tests in `state.rs` verify `write_diff`/`load_all_diffs` isolation, but no chain-level test runs two sessions' PostToolUse → Stop sequences and verifies they don't interfere.

### Test to write:
In `avp-common/tests/stop_validators_integration.rs` (or new file):

1. Create a temp dir with two files: `a.rs` and `b.rs`
2. Run PreToolUse + modify + PostToolUse for `a.rs` with `session_id = \"parent-session\"`
3. Run PreToolUse + modify + PostToolUse for `b.rs` with `session_id = \"subagent-session\"`
4. Verify sidecar dirs: `.avp/turn_diffs/parent-session/` has only `a.rs` diff, `.avp/turn_diffs/subagent-session/` has only `b.rs` diff
5. Load diffs for `parent-session` — assert it sees only `a.rs`
6. Load diffs for `subagent-session` — assert it sees only `b.rs`
7. Run SessionStartCleanup for `subagent-session`
8. Verify `parent-session` diffs are still intact

This proves that subagent lifecycle (write + cleanup) doesn't affect parent session state.

### Approach (TDD):
Use `/tdd` workflow. Write the failing test FIRST, then fix if any wiring is broken.

## Acceptance Criteria
- [ ] Two sessions write diffs via the chain link (not direct API), each sees only its own
- [ ] Cleanup of one session doesn't affect the other

## Tests
- [ ] `test_session_scoped_diffs_isolated_through_chain_links`
- [ ] Run `cargo nextest run -p avp-common`"