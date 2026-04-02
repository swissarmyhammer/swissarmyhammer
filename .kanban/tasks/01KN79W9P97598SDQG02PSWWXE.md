---
assignees:
- claude-code
depends_on:
- 01KN79NWWHY3FZ6GMJXR6Q6C2S
position_column: done
position_ordinal: ffffffffffffffffffa780
title: 4. Add perspective commands to scope chain
---
## What

Add perspective commands to the scope chain so they appear in the command palette and context menus when a perspective is active.

## CRITICAL: Perspective is NOT an entity

**Do NOT create a perspective entity YAML.** Do NOT add perspective to `builtin/fields/entities/`. Do NOT touch entity count assertions in `defaults.rs` or `context.rs`. Perspectives are managed by `PerspectiveContext` / `swissarmyhammer_perspectives` — a completely separate system from the entity/fields system.

## How the scope chain already works

Read `swissarmyhammer-kanban/src/scope_commands.rs` before writing any code. The relevant path:

- `commands_for_scope()` Pass 2 calls `registry.available_commands(scope_chain)`
- That calls `scope_matches(def.scope, scope_chain)` in `registry.rs`
- `scope_matches` with `Some("entity:perspective")` will match if **any moniker in the chain** has type `"perspective"` — it uses `parse_moniker()` which works on any `type:id` string
- No entity YAML, no entity registration, no Rust changes needed for scope matching to work

## What actually needs to happen

### 1. Backend — add `scope` to perspective commands

In `swissarmyhammer-commands/builtin/commands/perspective.yaml`, add `scope: "entity:perspective"` and `context_menu: true` to the perspective commands (`perspective.filter`, `perspective.clearFilter`, `perspective.group`, `perspective.clearGroup`). This makes them appear when `perspective:{id}` is in the scope chain. No Rust changes needed — `scope_matches` already handles this.

### 2. Frontend — alias `useEntityCommands` as `useCommands`

In `kanban-app/ui/src/lib/entity-commands.ts`, export `useCommands` as an alias for `useEntityCommands`. The function already works for any type string. This is a naming clarity change only — do NOT change any logic, do NOT remove `useEntityCommands`.

### 3. Frontend — test that `useCommands` works for perspective type

## Files to modify
- `swissarmyhammer-commands/builtin/commands/perspective.yaml` — add `scope` + `context_menu` fields
- `kanban-app/ui/src/lib/entity-commands.ts` — add `useCommands` alias

## Acceptance Criteria
- [ ] `useCommands()` hook exported (alias of `useEntityCommands`)
- [ ] Perspective commands have `scope: "entity:perspective"` in YAML
- [ ] Rust test: perspective commands appear when `perspective:01ABC` is in scope chain
- [ ] Rust test: perspective commands absent when no perspective in scope
- [ ] Frontend test: `useCommands("perspective", id)` works
- [ ] Entity count assertions in `defaults.rs` and `context.rs` stay at 7 (unchanged)
- [ ] All existing tests pass

## Tests
- [ ] `kanban-app/ui/src/lib/entity-commands.test.ts` — verify `useCommands` works for perspective type
- [ ] `swissarmyhammer-kanban/src/scope_commands.rs` — test: perspective commands available when `perspective:01ABC` is in scope
- [ ] `cargo nextest run -E 'rdeps(swissarmyhammer-kanban)'` — all pass
- [ ] `pnpm test` from `kanban-app/ui/` — all pass