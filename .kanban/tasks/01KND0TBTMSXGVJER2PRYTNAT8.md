---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffb380
title: 'Integration test: sidecar diffs written by PostToolUse are read at Stop'
---
## What

No test currently verifies the full sidecar diff lifecycle: PostToolUseFileTracker writes diffs → Stop chain reads them. The two halves are tested independently but never wired together.

### Test to write:
In `avp-common/tests/stop_validators_integration.rs` (or a new `sidecar_diffs_integration.rs`):

1. Create a temp dir with a file (`test.txt`)
2. Run `PreToolUseFileTracker::process()` to stash pre-content
3. Modify the file
4. Run `PostToolUseFileTracker::process()` — this should write a sidecar diff to `.avp/turn_diffs/<session_id>/`
5. Verify the sidecar diff file exists on disk
6. Simulate Stop: load diffs via `TurnStateManager::load_all_diffs(session_id)`
7. Assert the loaded diffs contain the correct path and diff text matching the file change

This proves the write path (PostToolUse) and read path (Stop) use the same directory structure and encoding.

### Approach (TDD):
Use `/tdd` workflow. Write the failing test FIRST, then fix if any wiring is broken.

## Acceptance Criteria
- [ ] Test creates real file, runs both file tracker chain links, and verifies sidecar diff on disk
- [ ] Test loads diffs the same way Stop validators would and asserts content

## Tests
- [ ] `test_post_tool_use_writes_sidecar_diff_readable_at_stop`
- [ ] Run `cargo nextest run -p avp-common`"