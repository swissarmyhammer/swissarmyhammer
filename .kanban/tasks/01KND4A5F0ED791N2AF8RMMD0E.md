---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffd280
title: 'Fix pre-content cache: persist to disk for cross-process diff computation'
---
## What

`TurnStateManager.content_cache` is in-memory only. AVP runs as separate processes per hook, so PreToolUse stashes content that PostToolUse can never read. Diffs degrade to full-file-as-new.

### Fix:
Persist pre-content to sidecar files, same pattern as diffs:
```
.avp/turn_pre/<session_id>/<tool_use_id>/<encoded_path>.pre
```

- PreToolUse writes pre-content to disk instead of (or in addition to) in-memory cache
- PostToolUse reads pre-content from disk instead of in-memory cache
- SessionStartCleanup clears `turn_pre/<session_id>/`
- The in-memory cache can be removed or kept as an optimization for same-process scenarios

### Files to modify:
- `avp-common/src/turn/state.rs` — Add `write_pre_content(session_id, tool_use_id, path, content)` and `take_pre_content(session_id, tool_use_id) -> HashMap<PathBuf, Option<Vec<u8>>>` using sidecar files. Add `clear_pre_content(session_id)`.
- `avp-common/src/chain/links/file_tracker.rs` — PreToolUse calls `write_pre_content` instead of `stash_content`. PostToolUse calls `take_pre_content` instead of `take_content`. SessionStartCleanup calls `clear_pre_content`.
- `.avp/.gitignore` — Add `turn_pre/`

### Approach (TDD):
Use `/tdd` workflow. Write failing tests FIRST.

1. Write test: write_pre_content + take_pre_content roundtrip in temp dir
2. Write test: take_pre_content returns None for file content (new file scenario)
3. Write test: take_pre_content cleans up after itself (files removed after take)
4. Write integration test: PreToolUse writes pre-content, PostToolUse reads it (simulating separate processes by using separate TurnStateManager instances)
5. Write integration test: Edit produces proper unified diff (not new-file diff)

## Acceptance Criteria
- [ ] Edit tool produces proper unified diff showing only changed lines
- [ ] Write tool for new files still produces `--- /dev/null` diff
- [ ] Pre-content survives across separate TurnStateManager instances (simulating process boundary)
- [ ] Pre-content cleaned up at SessionStart
- [ ] `.avp/.gitignore` includes `turn_pre/`

## Tests
- [ ] Unit test: write + take pre-content roundtrip
- [ ] Unit test: take returns None content for new files
- [ ] Unit test: take cleans up sidecar files
- [ ] Unit test: clear_pre_content removes session dir
- [ ] Integration test: separate TurnStateManager instances share pre-content via disk
- [ ] Integration test: Edit sequence produces proper edit diff
- [ ] Run `cargo nextest run -p avp-common`"