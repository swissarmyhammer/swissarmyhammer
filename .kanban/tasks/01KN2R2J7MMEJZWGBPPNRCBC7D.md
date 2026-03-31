---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffc080
title: Fix ShellExecuteTool to use scope-aware settings path (#37)
---
## What

`shelltool init local` writes to `.claude/settings.json` instead of `.claude/settings.local.json` because `ShellExecuteTool::init()` and `deinit()` ignore the scope parameter.

**File:** `swissarmyhammer-tools/src/mcp/tools/shell/mod.rs`

### Subtasks

- [ ] Add `fn claude_settings_path(scope: &InitScope) -> PathBuf` helper near the `Initializable` impl (Project → `.claude/settings.json`, Local → `.claude/settings.local.json`, User → `~/.claude/settings.json`)
- [ ] In `init()` (line 434): rename `_scope` → `scope`, replace hardcoded path at line 467 with helper call
- [ ] In `deinit()` (line 645): rename `_scope` → `scope`, replace hardcoded path at line 666 with helper call
- [ ] Update all log/error messages in both methods to use `claude_settings_path.display()` instead of hardcoded `.claude/settings.json` strings

## Acceptance Criteria

- [ ] `shelltool init local` creates/modifies `.claude/settings.local.json`, not `.claude/settings.json`
- [ ] `shelltool init project` still creates/modifies `.claude/settings.json` (no regression)
- [ ] `shelltool deinit local` modifies `.claude/settings.local.json`
- [ ] Log messages reflect the actual file being written

## Tests

- [ ] Unit test: `claude_settings_path` returns correct path for each `InitScope` variant
- [ ] Integration test: `init()` with `Local` scope writes Bash deny to `.claude/settings.local.json` in a temp dir
- [ ] Integration test: `deinit()` with `Local` scope removes Bash deny from `.claude/settings.local.json`
- [ ] `cargo nextest run -E 'rdeps(swissarmyhammer-tools)'` passes