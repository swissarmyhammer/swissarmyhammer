---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffc580
title: 'Group computation utility: compute group buckets from tasks + groupField'
---
## What

Create a pure utility function `computeGroups(tasks, groupField, fieldDefs)` that takes an array of tasks, a group field name, and field definitions, and returns an ordered list of group buckets.

### Files to create/modify

1. **Create `kanban-app/ui/src/lib/group-utils.ts`** — pure utility, no React dependencies:
   ```typescript
   interface GroupBucket {
     /** The raw group value (string). Empty string for ungrouped. */
     value: string;
     /** Human-readable label for the group header. */
     label: string;
     /** Task entities belonging to this group, preserving their input order. */
     tasks: Entity[];
   }
   
   function computeGroups(
     tasks: Entity[],
     groupField: string,
     fieldDefs: FieldDef[],
   ): GroupBucket[]
   ```

2. **Create `kanban-app/ui/src/lib/group-utils.test.ts`** — comprehensive tests

### Semantics

- **Single-value fields** (e.g. `project`, `color`): each task appears in exactly one group based on `task.fields[groupField]`.
- **Multi-value fields** (e.g. `tags`, `assignees`): if `task.fields[groupField]` is an array `[\"bug\", \"feature\"]`, the task appears in BOTH the \"bug\" and \"feature\" groups. Detect array fields by checking the field's `kind` from `fieldDefs` (or checking if value is an array at runtime).
- **Ungrouped**: tasks where the field is null, undefined, or empty string/array go into a special group with `value: \"\"` and `label: \"(ungrouped)\"`.
- **Sort order**: groups sorted alphabetically by `value`, with ungrouped last.
- **Task order within group**: preserved from input (caller is responsible for pre-sorting by column layout).

### Why this is its own card

This is pure logic with no UI — it needs thorough unit testing before the UI cards build on it. The grouped board view (next card) will call this function.

## Acceptance Criteria

- [ ] `computeGroups` returns correct buckets for single-value fields
- [ ] `computeGroups` returns correct buckets for multi-value (array) fields — tasks appear in multiple groups
- [ ] Ungrouped tasks collected into a single bucket at the end
- [ ] Groups are sorted alphabetically by value
- [ ] Task order within each group preserves input order
- [ ] Empty input (no tasks) returns empty array
- [ ] Exported from the module for use by GroupedBoardView

## Tests

- [ ] `kanban-app/ui/src/lib/group-utils.test.ts` — test single-value grouping (e.g. by project)
- [ ] `kanban-app/ui/src/lib/group-utils.test.ts` — test multi-value grouping (e.g. by tags) — same task in multiple buckets
- [ ] `kanban-app/ui/src/lib/group-utils.test.ts` — test ungrouped tasks go to \"(ungrouped)\" bucket at end
- [ ] `kanban-app/ui/src/lib/group-utils.test.ts` — test empty tasks array returns []
- [ ] `kanban-app/ui/src/lib/group-utils.test.ts` — test alphabetical sort of group values
- [ ] `npm test -- group-utils` passes

## Workflow

- Use `/tdd` — write failing tests first, then implement to make them pass.">
</invoke>