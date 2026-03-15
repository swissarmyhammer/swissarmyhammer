---
assignees:
- assistant
position_column: done
position_ordinal: ffffdf80
title: 'Clean up .gitignore: remove per-tool entries, let tools self-manage'
---
## What
The root `.gitignore` has ~15 entries managing internals of `.swissarmyhammer/` (like `tmp/`, `sessions/`, `transcripts/`). Per new philosophy: each tool directory owns its own `.gitignore` via `DirectoryConfig::GITIGNORE_CONTENT`. The root `.gitignore` should NOT micromanage tool internals.

### Changes
1. Remove all `.swissarmyhammer/...` entries (they reference the old name AND micromanage internals)
2. Do NOT add equivalent `.sah/...` entries — `SwissarmyhammerConfig::GITIGNORE_CONTENT` already handles `tmp/`, `todo/`, `*.log`, `workflow-runs/`, `transcripts/`, `questions/`
3. Keep only the per-subcrate ignores like `swissarmyhammer-cli/.sah` etc. if those specific project dirs shouldn't have a `.sah` committed
4. Remove `.ralph/` entries if any — ralph's own `.gitignore` handles itself

## Acceptance Criteria
- [ ] No `.swissarmyhammer` in `.gitignore`
- [ ] No root-level entries micromanaging tool dir internals
- [ ] Tool dirs self-manage via GITIGNORE_CONTENT