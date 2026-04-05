---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffe380
title: 'WARNING: .avp/.gitignore is stale -- references turn_state.yaml but code now uses turn_state/ directory'
---
.avp/.gitignore:8-9

The gitignore still lists `turn_state.yaml` and `turn_state.yaml.lock` (the old single-file scheme). The new code stores session-scoped state under `.avp/turn_state/<session_id>.yaml`. The old entries are now dead and the new `turn_state/` directory is not gitignored.

If a user commits their `.avp/` directory, session state files could leak into version control.

**This should be fixed in `avp init` (the swissarmyhammer-directory crate's `.avp/` initialization)**, not as a one-off edit. The `.avp/.gitignore` is generated/managed by the init process, so the fix belongs there to ensure all new and existing projects get the correct entries.

Suggestion: Update the init code that generates `.avp/.gitignore` to include:
- `turn_state/` (replaces `turn_state.yaml` and `turn_state.yaml.lock`)
- `turn_diffs/`
- Keep existing `*.log` entry

Also update the current `.avp/.gitignore` in this repo to match. #review-finding"