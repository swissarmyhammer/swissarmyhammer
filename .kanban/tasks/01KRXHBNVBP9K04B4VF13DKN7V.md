---
assignees:
- claude-code
position_column: todo
position_ordinal: '8180'
title: Remove committed .hence transcript files and gitignore the directory
---
Two raw transcript dumps are committed to the repo and should not be tracked.

## Files to remove
- `crates/acp-conformance/.hence/transcript_raw.jsonl`
- `crates/claude-agent/.hence/transcript_raw.jsonl` (~5.3 MB)

## Steps
- `git rm` both files (and the `.hence/` directories if they become empty).
- Add `.hence/` to `.gitignore`. Note `.acp/` is already ignored (`.gitignore:80`, `.gitignore:114`); `.hence/` was missed, which is how these slipped in.
- Confirm no code references the `.hence/` path — current code writes to `.acp/`, so `.hence/` is stale leftover from a prior naming.

Independent of the manager-consolidation cards; can land first.