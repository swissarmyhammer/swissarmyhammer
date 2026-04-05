---
assignees:
- claude-code
depends_on:
- 01KNF8SW8CJHRFE6B3ZEQF1FV9
position_column: todo
position_ordinal: '8480'
title: 'Frontend: Remove swimlanes, wire project field grouping'
---
## What

Remove swimlane references from the frontend and ensure the `project` reference field works correctly with the existing metadata-driven grouping system. Add group header display name resolution for reference fields.

### Files to modify:
- **`kanban-app/ui/src/types/kanban.ts`** — remove `swimlanes` from `BoardData` interface and `parseBoardData`
- **`kanban-app/ui/src/components/board-view.tsx`** — remove `position_swimlane` from move task args (line ~384); grouping already works via `groupField` from perspective
- **`kanban-app/ui/src/components/board-view.test.tsx`** — remove swimlane from test data
- **`kanban-app/ui/src/components/window-container.tsx`** — remove swimlane references
- **`kanban-app/ui/src/components/window-container.test.tsx`** — remove swimlane from test fixtures
- **`kanban-app/ui/src/components/quick-capture.tsx`** — remove swimlane references if any
- **`kanban-app/ui/src/components/rust-engine-container.tsx`** — remove swimlane references
- **`kanban-app/ui/src/components/nav-bar.test.tsx`** — remove swimlane test data
- **`kanban-app/ui/src/lib/refresh.ts`** — remove swimlane references
- **`kanban-app/src/state.rs`** — remove swimlane from Tauri state
- **`kanban-app/src/commands.rs`** — remove swimlane from commands
- **`kanban-app/src/watcher.rs`** — remove swimlane from file watcher

### Group display name resolution:
When grouping by a reference field like `project`, the group header currently shows the raw field value (entity ID). We need to resolve it to the entity's `search_display_field` value (the project name).

Check how the board-view and grid-view render group headers. If they use the raw field value, add resolution logic that:
1. Looks up the referenced entity type from the field's `type.entity`
2. Looks up the `search_display_field` from the entity schema
3. Resolves the ID to the display name from the entity store

This may already work if the field stores display names, but verify.

## Acceptance Criteria
- [ ] `grep -r swimlane kanban-app/` returns zero hits (except node_modules)
- [ ] `BoardData` interface has no `swimlanes` property
- [ ] Board view move operations don't send swimlane parameter
- [ ] Grouping by `project` field shows project names in group headers (not IDs)
- [ ] All frontend tests pass

## Tests
- [ ] `npm test` (or equivalent) passes in kanban-app
- [ ] Board view renders correctly without swimlanes
- [ ] Group-by project shows readable names

## Workflow
- Use `/tdd` — write failing tests first, then implement to make them pass. #swimlane-to-project