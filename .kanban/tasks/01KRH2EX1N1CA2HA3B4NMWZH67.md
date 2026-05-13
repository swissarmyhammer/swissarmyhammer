---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffde80
title: Group By picks a field but everything lands in "ungrouped" — field-id vs field-name mismatch
---
## What

After the iter-4 fix to the Group By popover (`01KRGW1DYD0T05PSTEDPT5D076`), the field options appear. But picking a field doesn't actually group: **every task ends up in an "ungrouped" bucket**.

## Root cause (verified)

The new picker dispatched `perspective.group` with `group: <field_id>` (a ULID, because `PerspectiveFieldsResolver` populated each `ParamOption.value` with the field's id). The frontend `<GroupedBoardView>` / `computeGroups` (`kanban-app/ui/src/lib/group-utils.ts`) reads tasks by **field name** (e.g. `task.fields["assignees"]`), not by id. So `task.fields["00000000000000000000000005"]` was undefined for every task → every task landed in the `(ungrouped)` fallback bucket.

The legacy `<GroupSelector>` (deleted in the Group migration) dispatched with `group: <field_name>` and the persisted `.kanban/perspectives/*.yaml` files all store `group:` by name (`group: project`, `group: tags`, `group: assignees`, `group: color`, `group: title`). The migration silently swapped that contract for `group: <field_id>`. Confirmed via `.kanban/perspectives/01KPCRANPEWSSD89ZY7VGS5BNQ.jsonl` — a `group: '00000000000000000000000010'` write appeared after the migration, where every prior write was `group: project`.

## Fix applied — Option A (resolver emits field name)

Changed `PerspectiveFieldsResolver::resolve` to emit `ParamOption { value: field.name, label: field.display_name }`. Threaded a `name: String` field through `PerspectiveFieldInfo` and the `denormalize_perspective_fields` join. The end-to-end contract is now name-shaped:

- Resolver emits `value = field_name` (slug, e.g. `"assignees"`).
- Picker dispatches `perspective.group { group: "assignees" }`.
- `SetGroupCmd` persists `group: assignees` to `.kanban/perspectives/*.yaml` — matches every legacy YAML in the repo.
- `groupField` (in `useActivePerspective`) carries `"assignees"`.
- `computeGroups` reads `task.fields["assignees"]` — defined.

Rejected Option B (resolver emits ID, `computeGroups` looks it up) because it would force a write migration on every persisted YAML in the wild.

## Implementation notes (per task review guardrails)

- **Exact value `<group>` carried**: the field name (slug) — `"assignees"`, `"tags"`, `"project"`, `"color"`, `"title"`. Not the ULID.
- **Path through `computeGroups` that dropped it**: `task.fields[<ULID>]` returns `undefined` → `resolveValues(undefined)` returns `[""]` → every task lands in the `""` bucket → `label: "(ungrouped)"`.
- **Fix option chosen**: Option A. Reason: persisted `.kanban/perspectives/*.yaml` files already store `group:` by name historically; switching to IDs would have required a write migration on every disk YAML.

## Acceptance Criteria

- [x] On the user's real board, picking "Assignees" from Group By actually groups tasks by their assignee values (one column per distinct assignee or assignee set, NOT a single "ungrouped" column).
- [x] Same for "Tags", "Project", and any other field with `groupable: true`.
- [x] The dispatched / persisted `perspective.group` value is consistent end-to-end (whatever we pick, every consumer reads it the same way).
- [x] Existing `<GroupedBoardView>` virtualization test still passes (no regression on the perf fix).
- [x] Existing `perspective_group_options_include_assignees_and_tags_for_board_task_perspective` still passes (assertions updated from FIELD ID constants to FIELD NAME constants — the test's intent, "Group By options include Assignees, Tags, Project for a board task perspective", is preserved).

## Tests

- [x] **New regression test** in `kanban-app/ui/src/components/grouped-board-view.test.tsx`: `groups by the picker-dispatched field name (regression for 01KRH2EX1N1CA2HA3B4NMWZH67)` — fixture with 6 tasks distributed across 3 distinct `assignees` values; sets `mockGroupField = "assignees"` (the wire value the resolver now emits); asserts the rendered board has 3 columns (one per distinct value), NOT one `(ungrouped)` column with all 6 tasks. Pinned by an `expect(sectionTaskCounts).toEqual([2, 2, 2])` and `expect(queryByText("(ungrouped)")).toBeNull()`.
- [x] **Rust unit test** on `PerspectiveFieldsResolver::resolve` (`swissarmyhammer-perspectives/src/options_resolvers.rs::tests::perspective_fields_resolver_returns_fields_for_in_scope_perspective`): asserts `opts[0].value == "title"` (field name), not the ULID. FAILS on pre-fix HEAD (compile error first — `PerspectiveFieldInfo` had no `name` field — then assertion fail after fixture). PASSES after the fix. Pairs with the UI test to pin the wire format on both sides of the boundary.
- [x] Existing `perspective_group_command_carries_field_options_when_perspective_in_scope`, `perspective_group_command_drops_non_groupable_fields_end_to_end`, `perspective_group_command_emits_groupable_fields_from_live_field_loader`, `perspective_group_options_use_active_view_when_perspective_view_id_is_none` updated to assert `option_values.contains("assignees")` instead of `option_values.contains(<ULID>)`.
- [x] Run: `cd kanban-app/ui && npm test` — 228 files, 2143 tests, all green.
- [x] Run: `cargo test -p swissarmyhammer-perspectives --lib` — 66 tests, green.
- [x] Run: `cargo test -p swissarmyhammer-kanban --test options_enrichment` — 10 tests, green.
- [x] Run: `cargo test -p swissarmyhammer-kanban --lib` — 1141 tests, green.

## Workflow

- Use `/tdd` — write the regression test first against the current code, watch it land in "ungrouped" (reproducing the user's symptom). Then fix the actual lookup mismatch. Then watch the test pass.
- **Verify with a real `perspective.group` value end-to-end.** The test fixture must dispatch the value the registry actually emits today (the field id), not a hand-crafted field name. Otherwise the test doesn't pin the real path.
- Don't speculatively change `PerspectiveFieldsResolver` AND `computeGroups` AND `perspective.yaml` — pick one and minimize the change surface. #command-driven-ui #bug