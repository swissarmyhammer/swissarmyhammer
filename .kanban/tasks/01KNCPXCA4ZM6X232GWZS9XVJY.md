---
assignees:
- claude-code
depends_on:
- 01KNCE5ZJ49DZHB4FM7H1747PE
position_column: done
position_ordinal: ffffffffffffffffffdb80
title: Scope turn_state.yaml by session ID and clean at SessionStart
---
## What

Pre-existing problem: `turn_state.yaml` is a single project-wide file shared across all sessions. A subagent's `StopCleanup` wipes the parent session's tracked changed files. The original design avoided per-session files to prevent \"file explosion with subagents\" but created a race condition instead.

Now that card 3 introduces session-scoped diff directories (`.avp/turn_diffs/<session_id>/`), the turn state should follow the same pattern for consistency and correctness.

### Changes:
- Move from `.avp/turn_state.yaml` to `.avp/turn_state/<session_id>.yaml`
- Clean at SessionStart (for that session) instead of StopCleanup — preserves debug evidence, matches diff lifecycle
- Remove StopCleanup of turn_state (Stop no longer clears state)
- Each session's changed paths are isolated from other sessions

### Why this is safe now:
The original concern about \"file explosion\" was about many session files accumulating. With SessionStart cleanup, each session cleans its own file at the start of a new turn. Stale files from crashed sessions can be cleaned by age (optional, future work).

### Shared cleanup refactor with Card 3:
Card 3 already wires `clear_diffs(session_id)` into `session_start_chain`. This card adds `turn_state` cleanup to the same SessionStart chain and removes it from `StopCleanup`. Both cards modify `session_start_chain()` in `factory.rs` and `StopCleanup` in `file_tracker.rs` — the implementer should build on Card 3's changes rather than conflicting.

### Files to modify:
- `avp-common/src/turn/state.rs` — Change `state_path()` to use `turn_state/<session_id>.yaml`, restore session_id parameter usage in load/save/clear
- `avp-common/src/chain/links/file_tracker.rs` — Move turn_state cleanup from `StopCleanup` to `SessionStartCleanup`, alongside the diff cleanup added by Card 3. StopCleanup can potentially be removed entirely.
- `avp-common/src/chain/factory.rs` — Wire state cleanup into session_start_chain (extend what Card 3 added), remove from stop_chain

### Approach (TDD):
Use `/tdd` workflow. Write failing tests FIRST, then implement.

## Acceptance Criteria
- [ ] Each session has its own turn state file under `.avp/turn_state/`
- [ ] Subagent cleanup never touches parent session's state
- [ ] State cleaned at SessionStart, survives past Stop for debugging
- [ ] Existing tests updated to pass session IDs correctly
- [ ] StopCleanup either removed or reduced to a no-op

## Tests
- [ ] Unit test: two sessions write/read independently
- [ ] Unit test: clear for session A doesn't affect session B
- [ ] Unit test: SessionStart cleans that session's state
- [ ] Run `cargo nextest run -p avp-common`"