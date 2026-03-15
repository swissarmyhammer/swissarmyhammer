---
assignees:
- assistant
depends_on:
- 01KKM7SZXBK3PAGQAKJMZZ1KX8
- 01KKM7XB6QMFC920836DCB8DNV
- 01KKM7XTJKT3FPEXC9SNMNVF9Y
position_column: done
position_ordinal: ffffd080
title: 'Final sweep: grep-clean all .swissarmyhammer references'
---
## What
After all other cards are done, do a final sweep to ensure no `.swissarmyhammer` string literals remain anywhere in the codebase — source, tests, comments, YAML, TOML, JSON, markdown, CI configs.

### Scope
- `grep -r '.swissarmyhammer' --include='*.rs' --include='*.yaml' --include='*.yml' --include='*.toml' --include='*.json' --include='*.md'`
- Fix any remaining references
- Update `.gitignore` entries if any reference `.swissarmyhammer`
- Update CI workflows if they reference the old dir name
- Update CLAUDE.md / memory files if they reference old paths

### Also verify
- `grep -r 'home_dir()' --include='*.rs'` — should only appear in `swissarmyhammer-directory` crate (the XDG resolution layer) and `model-loader` (HuggingFace cache, separate concern)
- No crate outside `swissarmyhammer-directory` calls `dirs::home_dir()` to build a config path

## Acceptance Criteria
- [ ] Zero hits for `.swissarmyhammer` in any source file
- [ ] `dirs::home_dir()` only in directory crate and model-loader
- [ ] Full `cargo nextest run` passes
- [ ] `cargo build` clean (no warnings about deprecated `from_user_home`)

## Tests
- [ ] `cargo nextest run` (full workspace)
- [ ] `cargo build --workspace` clean