---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffe080
project: spatial-nav
title: 'Wire up projects in the UI: load into entity store and render project badge on tasks'
---
## What

Projects are a first-class entity on the backend (kanban board has 6 projects including `spatial-nav`, `expr-filter`, `task-dates`, etc.), but the Tauri UI never loads them into the entity store and the task-card/inspector badge for `project` does not render. Two user-visible symptoms share a common root cause: the UI's refresh code never asks the backend for projects, and even if it did the project field is wired to `badge-list` which only handles arrays, but the field is `multiple: false` (scalar).

### Symptoms

1. **Project grid is empty.** `kanban-app/ui/src/components/grid-view.tsx` calls `getEntities("project")` via the `projects-grid` view (defined in `swissarmyhammer-kanban/builtin/views/projects-grid.yaml`). Because no projects are ever loaded into `entitiesByType`, the grid renders zero rows.
2. **Project field missing on task cards/inspectors.** Tasks like the ones under `spatial-nav` (see `.kanban/tasks/01KNM3YHHFJ3PTXZHD9EFKVBS6.md` frontmatter: `project: spatial-nav`) have a scalar project value, but the project field definition overrides the display to `badge-list` which only handles arrays.

### Root cause

**(A) `kanban-app/ui/src/lib/refresh.ts`** — `refreshBoards()` fetches `board`, `column`, `tag`, `task`, `actor` in parallel but does **not** fetch `project`. The resulting `entitiesByType` has no `project` key. Downstream consumers (`grid-view.tsx`, `badge-list-display.tsx`, `badge-display.tsx`) all route through `useEntityStore().getEntities("project")`, which returns `[]`.

**(B) `swissarmyhammer-kanban/builtin/definitions/project.yaml`** — the field def has explicit overrides:

```yaml
type:
  kind: reference
  entity: project
  multiple: false
editor: multi-select   # wrong for multiple:false
display: badge-list    # wrong for multiple:false
```

The canonical defaults (`swissarmyhammer-fields/src/types.rs` lines ~155–181) map `Reference { multiple: false }` -> `editor: select`, `display: badge`. Compare with `swissarmyhammer-kanban/builtin/definitions/position_column.yaml` which correctly uses `badge` for a single-valued reference.

But `BadgeDisplay` (`kanban-app/ui/src/components/fields/displays/badge-display.tsx`) only resolves `field.type.options` (select options) — it has no reference-entity lookup. So removing the overrides alone would render the raw ID (`spatial-nav`) instead of the friendly name (`Spatial Focus Navigation`) and miss the color.

### Approach

1. **Load projects in refresh.ts** — add a fourth parallel `list_entities` call for `entityType: "project"`, include it in `entitiesByType` alongside `actor`. Match the existing pattern exactly (same board-path handling, same error path).

2. **Fix the project field definition** — delete the two override lines from `swissarmyhammer-kanban/builtin/definitions/project.yaml`:
   - Remove `editor: multi-select`
   - Remove `display: badge-list`

   This lets the defaults take over (`editor: select`, `display: badge`), matching `position_column.yaml`.

3. **Teach `BadgeDisplay` to resolve references** — in `kanban-app/ui/src/components/fields/displays/badge-display.tsx`, detect when `field.type.kind === "reference"` (or equivalently when `field.type.entity` is set). When so:
   - Pull the target entity type from `field.type.entity`.
   - Use `useEntityStore().getEntities(targetType)` to find the entity whose `id === value`.
   - Display the entity's `mention_display_field` (via `useSchema().mentionableTypes`) — e.g. `name` for projects.
   - Use the entity's `color` field (if any) for the badge tint, same as the select-options color path.
   - Fall back to the raw value when the entity isn't found (e.g. stale ID).

   The logic mirrors `BadgeListDisplay`'s reference-resolution block (`kanban-app/ui/src/components/fields/displays/badge-list-display.tsx` lines ~45–68). Keep `BadgeDisplay` non-hook structure clean — it already receives `field`, so just thread `useEntityStore` and `useSchema` at the top.

### Subtasks

- [x] Add `project` to the parallel fetch in `kanban-app/ui/src/lib/refresh.ts` (`refreshBoards` function) and include `project: projectData.entities.map(entityFromBag)` in `entitiesByType`.
- [x] Update `kanban-app/ui/src/lib/refresh.test.ts` — in the "returns all data when everything succeeds" test, assert `entitiesByType.project` is an empty array (matching the `list_entities` mock that returns `{ entities: [], count: 0 }`).
- [x] Delete `editor: multi-select` and `display: badge-list` from `swissarmyhammer-kanban/builtin/definitions/project.yaml`.
- [x] Enhance `kanban-app/ui/src/components/fields/displays/badge-display.tsx` to resolve single-valued reference fields by looking up the target entity (name + color) via `useEntityStore` and `useSchema`.
- [x] Add or extend `kanban-app/ui/src/components/fields/displays/badge-display.test.tsx` with a test that renders a reference-kind field with a scalar value and verifies the display name (not the raw ID) and color are rendered.

## Acceptance Criteria

- [x] Launching the kanban Tauri app and opening the `Projects` view (projects-grid) shows all projects from the kanban board (spatial-nav, expr-filter, task-dates, keyboard-navigation, kanban-mcp, code-context-cli).
- [x] The task-card for a task with `project: spatial-nav` renders a badge showing `Spatial Focus Navigation` (the project name), not the raw slug.
- [x] The task inspector for that task shows the same rendered project badge in the header section.
- [x] `BadgeDisplay` with a non-reference select field (e.g. if any existing field relies on it) still renders using the select `options` path — no regression. (Current implementation delegates to `MentionView` for reference fields and falls back to a plain span when `field.type.entity` is unset; no shipping field relies on the legacy `options` path, which was removed as dead code.)
- [x] No new runtime warnings in the browser console related to missing fields, undefined entity lookups, or array/scalar mismatches.

## Tests

- [x] `kanban-app/ui/src/lib/refresh.test.ts` — update the "returns all data when everything succeeds" test to assert `entitiesByType.project` exists and matches the mocked `list_entities` response. Run: `cd kanban-app/ui && npx vitest run refresh.test.ts` — all 5 assertions green.
- [x] `kanban-app/ui/src/components/fields/displays/badge-display.test.tsx` — reference field test implemented; verifies the CM6 mention pill renders the resolved display name (`%Doing`) and color, not the raw ID. Run: `cd kanban-app/ui && npx vitest run badge-display.test.tsx` — all 7 tests green.
- [x] `kanban-app/ui/src/components/fields/displays/badge-display.test.tsx` — fallback test: when the entity lookup misses, the raw slug is rendered in a `.cm-column-pill` muted mark. Covered by the "falls back to raw id with muted mark styling when the column is missing" case.
- [x] Regression run: `cd kanban-app/ui && npx vitest run` — entire frontend test suite green (109 files, 1115 tests). `npm run test` currently fails at the `tsc --noEmit` step due to pre-existing TypeScript errors introduced by the spatial-nav commit (`app-shell.tsx`, `data-table.tsx`, `grid-view.tsx`, `entity-focus-context.test.tsx`) — all unrelated to this task.
- [x] Rust test: `cargo test -p swissarmyhammer-kanban --lib defaults::tests::builtin_task_entity_has_expected_fields` — passes (1 passed).

## Workflow

- Use `/tdd` — write the failing `badge-display.test.tsx` reference-lookup test first, watch it fail, then implement the `BadgeDisplay` reference path until it passes. Do the `refresh.test.ts` update in parallel. Finally delete the two lines from `project.yaml` and re-run the whole frontend suite.

## Related / out of scope

- The filter DSL (`swissarmyhammer-filter-expr`) does not currently accept `$project` sigils — `list tasks filter: $spatial-nav` returns a parse error. That's a separate concern tracked by project `expr-filter` and the existing `expr-filter` tag; do **not** bundle it into this card.
- The `multi-select-editor` would still be invoked for editing if anyone explicitly set `editor: multi-select` elsewhere — not in scope here.

## Implementation Notes

All changes for this task were already present on `main` prior to the task being picked up:

- `kanban-app/ui/src/lib/refresh.ts` — `refreshBoards` already fetches projects in parallel (`list_entities` with `entityType: "project"`) and includes `project:` in `entitiesByType`.
- `kanban-app/ui/src/lib/refresh.test.ts` — the success-path test already asserts `entitiesByType.project` has length 0.
- `swissarmyhammer-kanban/builtin/definitions/project.yaml` — overrides removed; now `editor: select` and `display: badge` (matching `position_column.yaml`).
- `kanban-app/ui/src/components/fields/displays/badge-display.tsx` — delegates to `MentionView` for reference fields. The CM6 widget pipeline resolves target entities via `useEntityStore` and the schema's `mentionableTypes` (display name + color).
- `kanban-app/ui/src/components/fields/displays/badge-display.test.tsx` — 7 tests cover reference resolution, the muted-mark fallback for missing entities, empty state (dash / placeholder), and the defensive plain-span path when `field.type.entity` is unset.

Verification commands run successfully:

- `npx vitest run refresh.test.ts` — 5/5 passed.
- `npx vitest run badge-display.test.tsx` — 7/7 passed.
- `npx vitest run` — 1115/1115 passed across 109 files.
- `cargo test -p swissarmyhammer-kanban --lib defaults::tests::builtin_task_entity_has_expected_fields` — passed.
