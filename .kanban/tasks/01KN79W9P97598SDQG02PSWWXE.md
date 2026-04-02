---
assignees:
- claude-code
depends_on:
- 01KN79NWWHY3FZ6GMJXR6Q6C2S
position_column: todo
position_ordinal: 9c80
title: 4. Add perspective commands to scope chain — perspectives are Commandable like entities
---
## What

Perspectives and views need to participate in the command scope chain just like entities do. The good news: the command system is already generic — monikers, scope resolution, dispatch, context menus all work with arbitrary `type:id` strings. No core command system changes needed.

What's missing: perspective commands aren't in the scope chain because perspectives don't generate monikers in the UI yet, and the backend schema doesn't define perspective-level commands.

### What's already generic (no changes needed)
- `moniker.ts` — `moniker(type, id)` works for any type
- `command-scope.tsx` — generic scope chain + shadowing
- `entity-commands.ts` — `useEntityCommands(entityType, entityId)` works for any type
- `focus-scope.tsx` — generic moniker handling
- `context-menu.ts` — generic command dedup
- `scope_commands.rs` — `commands_for_scope()` works with any entity type from schema
- `commands.rs` dispatch — fully generic

### What needs to happen
1. **Rename `useEntityCommands` to `useCommands`** — or alias it. The function already works for any type, but the name implies entity-only. This is a naming/clarity change, not a logic change.
   - `kanban-app/ui/src/lib/entity-commands.ts` → rename exports
   - Update callers (board-view.tsx, grid-view.tsx, entity-inspector.tsx)

2. **PerspectiveProvider generates a perspective moniker in scope chain** — when rendering the active perspective tab bar (future card), wrap content in `<CommandScopeProvider moniker={moniker("perspective", perspectiveId)}>`. This puts `perspective:{id}` in the scope chain.

3. **Backend: perspective commands resolve from scope chain** — the existing `perspective.filter`, `perspective.sort.set`, etc. commands already exist in the YAML. They need `scope: "entity:perspective"` (or equivalent) so `commands_for_scope()` includes them when a perspective moniker is in the chain.

### Files to modify
- `kanban-app/ui/src/lib/entity-commands.ts` — rename `useEntityCommands` → `useCommands` (keep old name as alias for backwards compat)
- `swissarmyhammer-commands/builtin/commands/perspective.yaml` — add `scope` field to perspective commands so they appear in context menus when perspective is in scope

## Acceptance Criteria
- [ ] `useCommands()` hook available (alias or rename of `useEntityCommands`)
- [ ] Perspective commands appear in command palette when perspective moniker is in scope chain
- [ ] Perspective commands appear in context menu on right-click within a perspective scope
- [ ] Existing entity commands unaffected

## Tests
- [ ] `kanban-app/ui/src/lib/entity-commands.test.ts` — verify `useCommands` works for perspective type
- [ ] `swissarmyhammer-kanban/src/scope_commands.rs` — test: perspective commands available when `"perspective:01ABC"` is in scope
- [ ] `cargo nextest run -E 'rdeps(swissarmyhammer-kanban)'` — all pass
- [ ] `pnpm test` from `kanban-app/ui/` — all pass