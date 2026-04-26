---
assignees:
- claude-code
depends_on:
- 01KM6CGNBFV5GXFSC2VJJ93CSZ
position_column: done
position_ordinal: ffffffffffff9f80
title: Build CommandDefs from entity schema instead of hardcoding
---
## What

Create a shared hook `useEntityCommands(entityType: string, entityId: string, entity?: Entity): CommandDef[]` in `kanban-app/ui/src/lib/entity-commands.ts` that converts the schema's `EntityCommand[]` into `CommandDef[]` with:
- Resolved template names via `resolveCommandName` (\"Inspect {{entity.type}}\" → \"Inspect Task\")
- `target` set to the entity moniker
- `contextMenu` from the YAML `context_menu` field
- `execute` wired to `inspectEntity(moniker)` for `entity.inspect`, `dispatch_command` for `entity.archive`, etc.

Then replace the hardcoded command arrays in **all seven components** that currently define entity commands:

### Files to modify
- `kanban-app/ui/src/lib/entity-commands.ts` — add `useEntityCommands` hook
- `kanban-app/ui/src/components/entity-card.tsx` (line 67) — replace hardcoded `entity.inspect`
- `kanban-app/ui/src/components/column-view.tsx` (line 96) — replace hardcoded `entity.inspect`
- `kanban-app/ui/src/components/board-view.tsx` (line 60) — replace hardcoded `entity.inspect`
- `kanban-app/ui/src/components/mention-pill.tsx` (line 93) — replace hardcoded `entity.inspect`
- `kanban-app/ui/src/components/avatar.tsx` (line 58) — replace hardcoded `entity.inspect` for actors
- `kanban-app/ui/src/components/command-palette.tsx` (line 437) — replace hardcoded `entity.inspect` in search results

### Execute handler mapping
- `entity.inspect` → calls `inspectEntity(moniker)` (from InspectContext)
- `entity.archive` → calls `dispatch_command` with the target
- Unknown IDs → fall through to `dispatch_command` (future-proof)

The hook accepts an optional `extraCommands` array so callers can append local commands (e.g. mention-pill's `task.untag` stays local — it's a relationship command, not an entity command).

### Commands that remain hardcoded (acceptable)
- `app-shell.tsx`: `app.*`, `file.*`, `settings.*` — app-global, not entity-specific
- `grid-view.tsx`: `grid.moveUp/Down/Left/Right`, `grid.edit`, `grid.escape`, `grid.deleteRow`, `grid.new*` — view-specific navigation, declared in view YAML
- `App.tsx`: `nav.view.*` — generated from view registry
- `mention-pill.tsx`: `task.untag` — relationship command (task×tag pair), passed as extraCommands

## Acceptance Criteria
- [ ] `useEntityCommands(\"task\", \"abc\")` returns CommandDefs matching YAML-declared commands for task
- [ ] All 7 components use `useEntityCommands` — no hardcoded entity.inspect or entity.archive anywhere
- [ ] Right-clicking a task card on the board shows \"Inspect Task\", \"Archive Task\"
- [ ] Right-clicking a column shows \"Inspect Column\"
- [ ] Right-clicking the board background shows \"Inspect Board\"
- [ ] Avatar right-click shows \"Inspect Actor\"
- [ ] Command palette search results get correct entity commands from schema

## Tests
- [ ] `kanban-app/ui/src/lib/entity-commands.test.ts` — test hook returns correct CommandDefs for a mock schema
- [ ] `kanban-app/ui/src/components/entity-card.test.tsx` — update to verify context menu items come from schema
- [ ] `pnpm --filter kanban-app test` passes