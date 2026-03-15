---
position_column: done
position_ordinal: c9
title: Extract ProjectStructure as Initializable component
---
## What

Extract `create_project_structure()` from monolithic `init.rs` into a standalone `ProjectStructure` struct implementing `Initializable`.

- `init`: Creates `.swissarmyhammer/`, `.prompts/`, `.swissarmyhammer/workflows/` directories
- `deinit`: Optionally removes `.swissarmyhammer/` and `.prompts/` directories (controlled by `--remove-directory` flag, which may need to be passed through InitScope or a config)
- Priority: 20 (after MCP registration, before tool-specific dirs)
- `is_applicable`: Only for `Project` and `Local` scopes

**Files:**
- NEW: `swissarmyhammer-cli/src/commands/install/components/project_structure.rs`
- EDIT: `swissarmyhammer-cli/src/commands/install/components/mod.rs`
- EDIT: `swissarmyhammer-cli/src/commands/install/init.rs` — remove `create_project_structure()`

## Acceptance Criteria
- [ ] `ProjectStructure` implements `Initializable`
- [ ] `is_applicable()` returns false for `User` scope
- [ ] `init()` creates same directory structure as current `create_project_structure()`
- [ ] `deinit()` handles directory removal
- [ ] Old function removed from `init.rs`

## Tests
- [ ] `cargo test -p swissarmyhammer-cli` passes
- [ ] Manual: `sah init` creates `.swissarmyhammer/`, `.prompts/`, `workflows/`