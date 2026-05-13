---
assignees:
- claude-code
position_column: todo
position_ordinal: aa80
title: Group By picks a field but everything lands in "ungrouped" — field-id vs field-name mismatch
---
## What

After the iter-4 fix to the Group By popover (`01KRGW1DYD0T05PSTEDPT5D076`), the field options appear. But picking a field doesn't actually group: **every task ends up in an "ungrouped" bucket**.

## Hypothesis (high confidence — verify before fixing)

The new picker dispatches `perspective.group` with `group: <field_id>` (a ULID, because `PerspectiveFieldsResolver` populates each `ParamOption.value` with the field's id). The frontend regroup logic in `<GroupedBoardView>` / `computeGroups` (`kanban-app/ui/src/lib/group-utils.ts` or similar) reads tasks by **field name** (e.g. `task.status`, `task.assignees`), not field id. So `task[<ULID>]` is undefined for every task → every task lands in the "ungrouped" fallback bucket.

The legacy `<GroupSelector>` (deleted in the Group migration) dispatched with `group: <fieldName>`. The migration swapped that for `group: <fieldId>` because the resolver produces `ParamOption { value: field_id, label: field_display_name }`. That's the right shape for a registry-driven picker, but `computeGroups` was never updated to look up the field by id and find the corresponding task value.

## Files to investigate

- `kanban-app/ui/src/lib/group-utils.ts` (or wherever `computeGroups` lives) — how does it currently read the task's grouping value?
- `kanban-app/ui/src/components/grouped-board-view.tsx` — passes `groupField` down; what's its shape (id or name)?
- `kanban-app/ui/src/components/perspective-context.tsx` — exposes `groupField` to the view; is it the picked dispatch value (id) or transformed back to a name somewhere?
- `swissarmyhammer-kanban/src/commands/perspective_commands.rs::SetGroupCmd::execute` — persists `perspective.group` as whatever string the dispatch sent. The persisted value flows to the frontend; what does the frontend do with it?

## Fix direction (two options — pick whichever fits the architecture better)

**Option A: Make `ParamOption.value` carry the field name.** Change `PerspectiveFieldsResolver` to set `value: field_def.name, label: field_def.display_name`. The user-facing field NAME is what gets dispatched and persisted to `perspective.group`. The frontend's `computeGroups` keeps using field-name lookups, no UI changes needed. Loses the ULID stability if a field is renamed.

**Option B: Update `computeGroups` to resolve field-id → field-name (or read the task's value via the field's storage path).** The dispatched / persisted value is the ULID. The grouping code does a one-step lookup via the schema's FieldDef to know how to read the task. More stable (renames don't break grouping) but more code.

The user's bias is toward "without bullshit" — recommend **Option A** unless there's a concrete reason field IDs matter on the persisted perspective. Persisted field references are typically by id when the field's display name might change, but if the perspective YAMLs already store `group: <field-name>` historically (check `.kanban/perspectives/*.yaml`), don't break that.

## Acceptance Criteria

- [ ] On the user's real board, picking "Assignees" from Group By actually groups tasks by their assignee values (one column per distinct assignee or assignee set, NOT a single "ungrouped" column).
- [ ] Same for "Tags", "Project", and any other field with `groupable: true`.
- [ ] The dispatched / persisted `perspective.group` value is consistent end-to-end (whatever we pick, every consumer reads it the same way).
- [ ] Existing `<GroupedBoardView>` virtualization test still passes (no regression on the perf fix).
- [ ] Existing `perspective_group_options_include_assignees_and_tags_for_board_task_perspective` still passes.

## Tests

- [ ] **New regression test** in `kanban-app/ui/src/components/grouped-board-view.test.tsx` (or a sibling): fixture with 6 tasks distributed across 3 distinct values of a groupable field (e.g. 3 statuses); dispatch `perspective.group` with the field; assert the rendered board has 3 columns (one per distinct value), NOT one "ungrouped" column with all 6 tasks. The test should FAIL on current HEAD and PASS after the fix.
- [ ] **Unit test** on `computeGroups` (`kanban-app/ui/src/lib/group-utils.ts`): given the dispatched value and a task list, assert it returns a non-trivial group structure (not all in fallback bucket).
- [ ] Run: `pnpm -C kanban-app/ui test grouped-board-view group-utils` — green.

## Workflow

- Use `/tdd` — write the regression test first against the current code, watch it land in "ungrouped" (reproducing the user's symptom). Then fix the actual lookup mismatch. Then watch the test pass.
- **Verify with a real `perspective.group` value end-to-end.** The test fixture must dispatch the value the registry actually emits today (the field id), not a hand-crafted field name. Otherwise the test doesn't pin the real path.
- Don't speculatively change `PerspectiveFieldsResolver` AND `computeGroups` AND `perspective.yaml` — pick one and minimize the change surface. #command-driven-ui #bug