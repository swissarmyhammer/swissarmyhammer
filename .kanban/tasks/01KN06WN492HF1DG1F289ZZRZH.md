---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffec80
title: Add menu_name to CommandDef and show board switch commands in Window menu
---
## What

The Window menu should dynamically include board switch commands, and command names should use templates like `{{entity.display_name}}` instead of hardcoded format strings. This extends the existing `{{entity.type}}` template system.

### Template variables

Extend `resolve_name_template` in `swissarmyhammer-kanban/src/scope_commands.rs` to support:

| Template | Resolves to | Missing value |
|---|---|---|
| `{{entity.type}}` | Capitalized entity type (existing) | — |
| `{{entity.display_name}}` | Entity's `name` field | empty string |
| `{{entity.context.display_name}}` | `KanbanContext::name()` (path stem above `.kanban`) | empty string |

**Simple substitution, no magic.** Each variable resolves independently. Missing = empty string. The template controls the format.

Example board switch command templates:
- `name: \"Switch to Board: {{entity.display_name}} ({{entity.context.display_name}})\"` → palette: \"Switch to Board: My Project (swissarmyhammer-kanban)\"
- `menu_name: \"{{entity.context.display_name}}\"` → Window menu: \"swissarmyhammer-kanban\"

### Naming hierarchy

1. **`KanbanContext::name()`** — sync, path-derived: `root().parent().file_name()` → `\"swissarmyhammer-kanban\"`. Stored as field, computed once.
2. **Entity display name** — just the entity's `name` field value. Empty string if not set.
3. **Composition** — the template controls how these are combined. No implicit formatting.

### Changes

**1. Add `name()` to `KanbanContext`** (`swissarmyhammer-kanban/src/context.rs`):
- `pub fn name(&self) -> &str` — returns path stem above `.kanban`, synchronous, computed once in constructor

**2. Extend template resolution** (`swissarmyhammer-kanban/src/scope_commands.rs`):
- Expand `resolve_name_template` to accept optional `entity_name: &str` and `context_name: &str`
- `{{entity.display_name}}` → `entity_name` (empty string if not provided)
- `{{entity.context.display_name}}` → `context_name` (empty string if not provided)
- `{{entity.type}}` → capitalized entity type (unchanged)

**3. Add `menu_name` to `CommandDef`** (`swissarmyhammer-commands/src/types.rs`):
- `pub menu_name: Option<String>` — display text for native menus, falls back to `name`
- Also a template — resolved the same way as `name`

**4. Use `menu_name` in menu builder** (`kanban-app/src/menu.rs:59`):
- `name: cmd.menu_name.clone().unwrap_or_else(|| cmd.name.clone())`

**5. Pass context/entity data into dynamic command generation** (`kanban-app/src/commands.rs`):
- `BoardInfo` gains `context_name: String` (from `handle.ctx.name()`) and `entity_name: String` (from board entity's `name` field, or empty)
- `commands_for_scope` passes these through to template resolution

**6. Add dynamic board switch items to Window menu** (`kanban-app/src/menu.rs`):
- After static Window items, before minimize/maximize
- For each open board, create `MenuItem` with id `board.switch:{path}` and label from resolved `menu_name` template
- Add separator before the group

## Acceptance Criteria
- [ ] `KanbanContext::name()` returns the path stem above `.kanban`
- [ ] `{{entity.display_name}}` resolves to entity name or empty string
- [ ] `{{entity.context.display_name}}` resolves to context path stem or empty string
- [ ] Window menu shows open boards in a group
- [ ] Palette shows \"Switch to Board: My Project (swissarmyhammer-kanban)\"
- [ ] `cargo nextest run` passes

## Tests
- [ ] `context.rs` — test: `name()` returns correct path stem
- [ ] `scope_commands.rs` — test: `{{entity.display_name}}` with name → entity name
- [ ] `scope_commands.rs` — test: `{{entity.display_name}}` without name → empty string
- [ ] `scope_commands.rs` — test: `{{entity.context.display_name}}` → context name
- [ ] `scope_commands.rs` — test: combined template resolves both variables
- [ ] `types.rs` — test: CommandDef with `menu_name` deserializes correctly
- [ ] `cargo nextest run -p swissarmyhammer-kanban` passes
- [ ] `cargo nextest run -p kanban-app` passes