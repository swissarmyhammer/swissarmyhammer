---
assignees:
- claude-code
position_column: todo
position_ordinal: cd80
title: 'Bug: Context menu shows raw template placeholders (e.g. "Delete {{entity.type}}")'
---
## What
Reported by user: right-click context menus render raw, un-interpolated template names like **"Delete {{entity.type}}"** instead of "Delete Task". The template name is not being rendered against the command context.

Path: right-click → `useContextMenu` (`apps/kanban-app/ui/src/lib/context-menu.ts:73`) invokes `list_commands_for_scope` and pushes each `cmd.name` straight into the native menu (context-menu.ts:93). So whatever `name` the backend returns is shown verbatim — the frontend does no interpolation (correctly; it should already be resolved).

The resolver exists in `crates/swissarmyhammer-kanban/src/scope_commands.rs`:
- `resolve_name_template(name, params)` (scope_commands.rs:168) replaces `{{entity.type}}` → capitalized entity type, plus `{{entity.display_name}}` / `{{entity.context.display_name}}`.
- It is called for the main command branches at scope_commands.rs:1127, 1171, 1220.
- There's even a guard test asserting `!cmd.name.contains("{{")` (scope_commands.rs:1650).

Yet "Delete {{entity.type}}" reaches the UI, so one of:
1. **A command-resolution branch bypasses `resolve_name_template`** and emits `cmd_def.name` raw for the scope/entity that produces the Delete command. Find the branch feeding context-menu commands for that entity and route its `name` through `resolve_name_template`.
2. **Placeholder mismatch**: `resolve_name_template` matches the EXACT substring `{{entity.type}}` (scope_commands.rs:173). If the YAML uses `{{ entity.type }}` (spaces) or a different casing, no replacement happens and the raw token leaks. Check the Delete command's YAML `name`.
3. **`entity_type` not resolved for this scope**: if the branch builds `TemplateParams` with the wrong/empty entity type. (Note: empty would yield "Delete ", not "Delete {{entity.type}}", so this alone doesn't explain it — points back to #1/#2.)

Reproduce: right-click a task (and other entities) and read the Delete / Copy entries — they show `{{entity.type}}`.

## Acceptance Criteria
- [ ] Context-menu entries show fully-resolved names (e.g. "Delete Task", "Copy Tag") — no `{{…}}` ever reaches the UI.
- [ ] Root cause identified (bypassed resolver branch vs. YAML placeholder mismatch).

## Tests
- [ ] Extend `scope_commands.rs` tests so the guard `!cmd.name.contains("{{")` covers the SPECIFIC scope/entity that produced "Delete {{entity.type}}" (the currently-uncovered path).
- [ ] If placeholder-format mismatch: a test asserting `resolve_name_template` handles the actual YAML token, or normalize the YAML and assert resolution.
- [ ] Regression test failing before the fix, passing after.

## Workflow
- Use `/tdd` — failing test first, then fix. #bug