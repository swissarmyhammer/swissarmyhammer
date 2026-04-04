---
assignees:
- claude-code
depends_on: []
position_column: done
position_ordinal: ffffffffffffffffffda80
title: Persist diffs across turn for Stop validators
---
## What

Currently, file diffs (`FileDiff` structs) live only in `ChainContext` per tool call via `CTX_FILE_DIFFS`. They're available for PostToolUse validators but lost by the time Stop fires. Changed file *paths* are accumulated in `TurnState.changed`, but the actual diff text is not.

We want sidecar diff files to become the **single source of truth** for diffs across ALL validator types — both tool-triggered (PostToolUse) and Stop-triggered. This unifies the diff pipeline: the file tracker writes, all validators read from the same place.

**Key constraint:** AVP runs as a separate process for each hook invocation. There is no shared in-memory state between PostToolUse and Stop calls. Multiple sessions (parent + subagents) share the same `.avp/` directory.

### Design: Session-scoped sidecar diff files

Write each diff as a standalone file in a **session-scoped** directory:
```
.avp/turn_diffs/
  <session_id_1>/          # parent session
    src__main.rs.diff
    src__lib__foo.rs.diff
  <session_id_2>/          # subagent session
    tests__integration.rs.diff
```

- **PostToolUseFileTracker** writes `<encoded_path>.diff` into the session's subdirectory. No locking needed — atomic write or last-writer-wins is fine.
- **PostToolUse validators** read the current file's diff from the session's sidecar directory.
- **Stop validators** glob `.avp/turn_diffs/<session_id>/*.diff` to load that session's accumulated diffs.
- **SessionStart** cleans `.avp/turn_diffs/<session_id>/` for a fresh turn — NOT at Stop. This preserves diffs after Stop for post-mortem debugging.
- The `CTX_FILE_DIFFS` ChainContext mechanism can be removed or deprecated — sidecar files are the canonical source.

### Why session-scoped, clean at Start

**Session scoping** prevents subagents from clobbering parent session diffs. Each session writes and reads its own directory. A subagent's lifecycle is fully isolated from the parent.

**Clean at SessionStart** (not Stop) because:
1. Diffs are valuable debug evidence — they should survive after Stop for inspection
2. A fresh turn starts with SessionStart, which is the natural reset point
3. StopCleanup currently wipes turn_state.yaml and has the pre-existing subagent race — we don't inherit that problem

### Files to modify:
- `avp-common/src/turn/state.rs` — Add helpers on `TurnStateManager`: `write_diff(session_id, path, diff_text)`, `load_diff(session_id, path) -> Option<String>`, `load_all_diffs(session_id) -> HashMap<String, String>`, `clear_diffs(session_id)`. Use `.avp/turn_diffs/<session_id>/` directory structure.
- `avp-common/src/chain/links/file_tracker.rs` — In `PostToolUseFileTracker`, write sidecar diff files with session_id. In `SessionStartCleanup` (or a new starter), call `clear_diffs(session_id)`.
- `avp-common/src/chain/links/validator_executor.rs` — For both PostToolUse and Stop hooks, load diffs from session-scoped sidecar files instead of ChainContext.
- `avp-common/src/chain/factory.rs` — Wire diff cleanup into session_start_chain.
- `.avp/.gitignore` — Add `turn_diffs/` and `turn_state.yaml` entries.

### Path encoding:
Encode file paths for filenames by replacing `/` with `__` (double underscore). Decode on read by reversing. Simple and avoids nested directories.

### Approach (TDD):
Use `/tdd` workflow. Write failing tests FIRST, then implement.

1. Write unit test: write_diff + load_diff (single file, single session) roundtrip in a temp dir
2. Write unit test: write_diff + load_all_diffs roundtrip
3. Implement write_diff, load_diff, load_all_diffs with session subdirectories
4. Write unit test: writing same path twice keeps latest content
5. Write unit test: clear_diffs empties only that session's directory, leaves other sessions intact
6. Write unit test: two sessions write diffs independently, each sees only its own
7. Wire into PostToolUseFileTracker (write) and ValidatorExecutorLink (read for both hook types)
8. Wire clear_diffs into SessionStart chain
9. Update `.avp/.gitignore`

## Acceptance Criteria
- [ ] Diffs from multiple tool calls (separate AVP processes) accumulate as sidecar files under session-scoped directories
- [ ] PostToolUse validators read diffs from sidecar files (same source as Stop)
- [ ] Stop validators load all accumulated diffs for their session at Stop time
- [ ] Multiple edits to the same file keep only the latest diff (last writer wins)
- [ ] Subagent diffs are isolated — parent session's `.avp/turn_diffs/<parent_id>/` is never touched by subagent cleanup
- [ ] `SessionStart` cleans the session's diff directory (not Stop — diffs survive for debugging)
- [ ] No changes to turn_state.yaml format — diffs stay out of the YAML
- [ ] `.avp/.gitignore` includes `turn_diffs/` and `turn_state.yaml`

## Tests
- [ ] Unit test: write_diff + load_diff roundtrip (single file, single session)
- [ ] Unit test: write_diff + load_all_diffs roundtrip (multiple files)
- [ ] Unit test: overwrite same path keeps latest
- [ ] Unit test: clear_diffs removes only that session's files
- [ ] Unit test: two sessions are isolated from each other
- [ ] Unit test: load_all_diffs on empty/missing dir returns empty map
- [ ] Run `cargo nextest run -p avp-common`"