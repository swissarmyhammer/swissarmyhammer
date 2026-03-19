---
assignees:
- claude-code
depends_on:
- 01KM3397BPR4801YWXTRFVYEY0
position_column: done
position_ordinal: fffffff380
title: Show dependency pills in card header after tags
---
## What
Display `blocked_by` and `blocks` dependencies as small pills in the card header, after the tag pills. These are computed fields (arrays of task IDs) set by backend enrichment in `task_helpers.rs`. The card should resolve each ID to a task title via `useEntityStore().getEntity("task", id)`.

**Rendering approach:**
- After the existing `headerFields.map(...)` loop in EntityCard, add a `DependencyPills` component
- Read `entity.fields.blocked_by` and `entity.fields.blocks` (both `string[]`)
- For each ID, look up the task via `getEntity("task", id)` and show its title (truncated)
- `blocked_by` pills: show with a warning/orange style — e.g. "⊳ Fix login" (blocked by that task)
- `blocks` pills: show with a subtle/muted style — e.g. "⊲ Deploy" (this task blocks that one)
- Clicking a dependency pill should inspect that task (call `inspectEntity`)
- If no dependencies, render nothing (no empty container)

**Files:**
- `kanban-app/ui/src/components/entity-card.tsx` — add `DependencyPills` component rendered after headerFields, uses `useEntityStore` (already imported) to resolve task IDs to titles

**Design constraints:**
- Pills should be compact — similar size to TagPill but simpler (no FocusScope/context menu needed, just click-to-inspect)
- Use existing color patterns: `text-amber-500`/`bg-amber-500/10` for blocked-by, `text-muted-foreground`/`bg-muted` for blocks
- Truncate long task titles to ~20 chars with ellipsis
- Don't render if both arrays are empty

## Acceptance Criteria
- [ ] Cards with `blocked_by` entries show orange-ish dependency pills after tags
- [ ] Cards with `blocks` entries show muted dependency pills after tags
- [ ] Each pill shows the dependency task's title (truncated)
- [ ] Clicking a pill opens the inspector for that task
- [ ] Cards with no dependencies show nothing extra (no empty space)

## Tests
- [ ] Manual: create task A, create task B with depends_on: [A] — B shows "blocked by A" pill, A shows "blocks B" pill
- [ ] Manual: click a dependency pill — inspector opens for the referenced task
- [ ] `npm run build` in ui/ succeeds