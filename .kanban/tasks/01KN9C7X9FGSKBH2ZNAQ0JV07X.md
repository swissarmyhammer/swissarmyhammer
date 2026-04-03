---
assignees:
- claude-code
depends_on:
- 01KN9C5394341SWFR5E65YZV4W
position_column: todo
position_ordinal: '8180'
title: Push active perspective into UIState scope chain for command palette visibility
---
## What

8 of 11 perspective commands have `scope: \"entity:perspective\"` and are invisible in the command palette because the UIState `scope_chain` never contains a `perspective:{id}` moniker. The moniker only exists in the local React `CommandScopeProvider` on perspective tabs (for right-click context menus). The command palette reads from `UIState.scope_chain`, which is set by `ui.setFocus` — and no code ever pushes a perspective moniker there.

### Fix approach

When `ui.perspective.set` fires and sets `active_perspective_id`, it should also inject `perspective:{active_id}` into the UIState scope chain. This way the command palette's `list_commands_for_scope` call will include scoped perspective commands whenever a perspective is active.

### Files to modify
- `swissarmyhammer-commands/src/ui_state.rs` — when `active_perspective_id` is set, ensure the scope chain includes `perspective:{id}`. Either:
  - (A) `set_active_perspective()` automatically appends/replaces the perspective moniker in the window's scope chain, OR
  - (B) The scope chain builder in `scope_chain_for_window()` (or equivalent) always includes the active perspective moniker
- `swissarmyhammer-kanban/src/commands/ui_commands.rs` — `SetActivePerspectiveCmd` may need to update the scope chain after setting the ID

### What success looks like
With an active perspective, pressing Cmd+K shows perspective commands (Filter, Clear Filter, Group, Clear Group, Sort, etc.) in the command palette.

## Acceptance Criteria
- [ ] Active perspective moniker (`perspective:{id}`) present in UIState scope chain
- [ ] Scoped perspective commands appear in command palette when a perspective is active
- [ ] Switching perspectives updates the scope chain moniker
- [ ] Clearing the active perspective removes the moniker from scope chain

## Tests
- [ ] Rust: `ui.perspective.set` → scope chain contains `perspective:{id}`
- [ ] Rust: `scope_commands::commands_for_scope` with perspective in chain → returns filter/group/sort commands
- [ ] `cargo nextest run -p swissarmyhammer-commands` — all pass
- [ ] `cargo nextest run -p swissarmyhammer-kanban` — all pass