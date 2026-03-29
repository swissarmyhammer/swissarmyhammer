---
assignees:
- claude-code
depends_on:
- 01KM6CG7TB07A3B38DW6P3WXKE
position_column: done
position_ordinal: ffffffffffcf80
title: Add EntityDef commands to TypeScript types and schema context
---
## What

Mirror the Rust `EntityCommand` type on the frontend and expose it through the schema context so components can read entity commands from the schema instead of hardcoding them.

### Files to modify
- `kanban-app/ui/src/types/kanban.ts` — add `EntityCommand` interface with `id`, `name` (template string), `context_menu`, `keys`; add `commands?: EntityCommand[]` to `EntityDef`
- `kanban-app/ui/src/lib/schema-context.tsx` — add a `getEntityCommands(entityType: string): EntityCommand[]` helper that returns the commands array from the cached schema, or `[]` if not loaded yet

### Template resolution
Add a utility function `resolveCommandName(template: string, entityType: string, entity?: Entity): string` in a new file `kanban-app/ui/src/lib/entity-commands.ts`:
- `{{entity.type}}` → capitalized entity type name (\"task\" → \"Task\")
- `{{entity.<field>}}` → field value lookup from the entity instance, falls back to empty string if missing
- No match → left as-is (unknown vars are not an error)

## Acceptance Criteria
- [ ] `EntityCommand` interface exists in `kanban.ts` matching Rust struct
- [ ] `EntityDef.commands` is optional and typed as `EntityCommand[]`
- [ ] `resolveCommandName(\"Inspect {{entity.type}}\", \"task\")` returns `\"Inspect Task\"`
- [ ] `resolveCommandName(\"Rename {{entity.name}}\", \"column\", someColumnEntity)` returns `\"Rename Backlog\"`
- [ ] `getEntityCommands(\"task\")` returns the commands from the schema

## Tests
- [ ] `kanban-app/ui/src/lib/entity-commands.test.ts` — test `resolveCommandName` with `{{entity.type}}`, `{{entity.title}}`, missing fields, no template vars
- [ ] `pnpm --filter kanban-app test` passes