---
assignees:
- claude-code
depends_on:
- 01KNKZYSYT8W352AP4KZFYVH1G
position_column: todo
position_ordinal: 947f8180
title: Add `perspective.goto` command for jumping directly to a perspective by index
---
## What

Add a Rust-side `perspective.goto` command that switches to a perspective by its 1-based index within the current view kind. This complements the existing `perspective.next`/`perspective.prev` cycling commands with a direct jump — analogous to vim's `Ngt` (go to tab N).

### Current state

- `perspective.next` and `perspective.prev` are being added in a sibling card (`01KNKZYSYT8W352AP4KZFYVH1G`). They establish the pattern: read `KanbanContext` for the perspective list, filter by `view_kind` arg, read/write `UIState` for the active perspective.
- `ui.perspective.set` (`swissarmyhammer-kanban/src/commands/ui_commands.rs:177-196`) sets a perspective by explicit ID via `UIState::set_active_perspective`.
- The frontend perspective list (`kanban-app/ui/src/lib/perspective-context.tsx`) filters perspectives by view kind and renders them in order — so index N in the tab bar corresponds to the Nth perspective matching the current view kind.

### Approach

**Rust side** — add `GotoPerspectiveCmd` to `swissarmyhammer-kanban/src/commands/perspective_commands.rs`:
- Required args: `index` (1-based integer), `view_kind` (string)
- Logic: list perspectives via `perspective_context()`, filter to `view_kind`, pick element at `index - 1`. If index is out of range, no-op (return `null`). Otherwise call `UIState::set_active_perspective` with the selected ID.
- Follows the same pattern as `NextPerspectiveCmd`/`PrevPerspectiveCmd` for accessing both `KanbanContext` and `UIState`.
- Always available (like `ui.perspective.set`).

### Files to modify

1. **`swissarmyhammer-kanban/src/commands/perspective_commands.rs`** — Add `GotoPerspectiveCmd` struct + `Command` impl. The execute method:
   - `ctx.require_extension::<KanbanContext>()`
   - `ctx.arg("index")` → parse as usize
   - `ctx.arg("view_kind")` → filter perspectives
   - Index into filtered list (1-based → 0-based), call `ui.set_active_perspective`

2. **`swissarmyhammer-kanban/src/commands/mod.rs`** — Register `"perspective.goto"` → `GotoPerspectiveCmd`. Update the command count assertion.

3. **`swissarmyhammer-commands/builtin/commands/perspective.yaml`** — Add YAML entry for `perspective.goto` with `index` and `view_kind` params. No default keybindings (the frontend will provide `1`–`9` via scope-level `CommandDef.keys`).

## Acceptance Criteria
- [ ] `perspective.goto` with `index: 1` switches to the first perspective matching the view kind
- [ ] `perspective.goto` with `index: N` where N > count is a no-op returning `null`
- [ ] `perspective.goto` with `index: 0` or negative is a no-op returning `null`
- [ ] Command updates UIState via `set_active_perspective` and returns the `UIStateChange`
- [ ] Command is registered in the command registry and YAML definitions

## Tests
- [ ] `swissarmyhammer-kanban/src/commands/perspective_commands.rs` — Unit tests: goto index 1 selects first, goto index 3 of 3 selects last, goto index 4 of 3 is no-op, goto index 0 is no-op
- [ ] Run `cargo test -p swissarmyhammer-kanban perspective_commands` — passes
- [ ] `cargo nextest run -p swissarmyhammer-commands` — YAML parses correctly

## Workflow
- Use `/tdd` — write failing tests first, then implement to make them pass.