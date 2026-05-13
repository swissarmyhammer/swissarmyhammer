---
assignees:
- claude-code
depends_on:
- 01KRE1WT72MJWNGQBVAD4V5VKM
- 01KRE7VDF7RXHV39VPEVH23NN4
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffdc80
title: Migrate Group tab button to command-driven rendering with field picker
---
## What

Replace the hardcoded `<GroupPopoverButton>` + `<GroupSelector>` with a registry-rendered `<CommandButton>` that opens a `<CommandPopover>` containing a single enum-shaped field picker. The picker options come from the `PerspectiveFieldsResolver` (registered post-refactor in `swissarmyhammer-perspectives`).

This is the first migration that exercises the picker pipeline end-to-end: enum param → backend-supplied options → frontend dropdown → dispatch with picked value.

### Implementation notes

- **`<GroupSelector>` decision: DELETED.** The legacy selector did three things `<CommandPopover>`'s generic enum renderer can't do natively: filter to `groupable === true` fields, include a "None" option dispatching `perspective.clearGroup`, and render each field as a dedicated button. The right replacements:
  - Field filtering is owned by the backend: `denormalize_perspective_fields` in `swissarmyhammer-kanban/src/dynamic_sources.rs` filters perspective fields against `FieldDef.groupable == Some(true)` so the Group By popover only surfaces fields the user can actually group on. (This filter was missing on the first pass — see review-finding #1 below — and is now in place with full unit-test coverage.)
  - The "None" affordance is restored via a `clear_command: "perspective.clearGroup"` annotation on the YAML param. `<CommandPopover>` renders "(none)" as the first option whenever a param carries `clear_command`, and `<CommandButton>`'s commit handler intercepts the empty-string sentinel to dispatch the clear command instead of the parent command.
  - The plain `<select>` from `<CommandPopover>` covers the field-list UX without any virtualization, search, or icon needs.

  → Deleted `kanban-app/ui/src/components/group-selector.tsx` and its unit test.

- **YAML annotation only.** The dispatcher path (`SetGroupCmd::execute`, `available()`, `resolve_and_persist_perspective_id`) is unchanged. The migration is a pure surface change: the YAML now tags the command with `tab_button: { icon: "group" }` and changes `params[0]` from `from: args` to `from: args, shape: enum, options_from: "perspective.fields", clear_command: "perspective.clearGroup"`. `perspective_id` switches to `from: scope_chain` with `entity_type: perspective`, matching the Filter migration's pattern. Existing palette and right-click tests continue to pass because the dispatcher still resolves `group` and `perspective_id` from any source.

### Files modified

- `swissarmyhammer-kanban/builtin/commands/perspective.yaml` — annotated `perspective.group` with `tab_button: { icon: group }`, `shape: enum, options_from: perspective.fields`, `clear_command: perspective.clearGroup`, `from: scope_chain` for perspective_id. Name changed from "Set Group" to "Group By".
- `swissarmyhammer-commands/src/types.rs` — added `ParamDef.clear_command: Option<String>` plus full docstring; updated two struct-literal sites in tests.
- `swissarmyhammer-kanban/src/dynamic_sources.rs` — `denormalize_perspective_fields` now filters perspective fields against `FieldDef.groupable == Some(true)`; six new unit tests pin the contract (groupable kept; non-groupable dropped; caption does not bypass the filter; unknown ids dropped; empty FieldsContext path).
- `kanban-app/ui/src/components/perspective-tab-bar.tsx` — deleted `<GroupPopoverButton>` definition, its JSX invocation, the local `groupOpen`/`schemaFields` state, and the no-longer-needed `Group` lucide / `useSchema` / `FieldDef` / `Popover*` / `GroupSelector` imports. Extended `isCommandActiveForPerspective` with a `case "perspective.group": return Boolean(perspective.group)` arm. Rewrote the `ScopedPerspectiveTab` JSDoc paragraph to reflect both Filter and Group as `<CommandButton>` leaves at `perspective_tab.perspective.{filter.focus,group}:{id}`.
- `kanban-app/ui/src/components/command-popover.tsx` — `<EnumField>` renders "(none)" as the first option when the param declares `clear_command`; submit-disabled rule relaxed so the clear sentinel is a legitimate submission.
- `kanban-app/ui/src/components/command-button.tsx` — `handleCommit` intercepts empty-string values for params with `clear_command` and dispatches the redirection target instead of the parent command (with the sentinel stripped from the args bag).
- `kanban-app/ui/src/components/perspective-spatial-nav.guards.node.test.ts` — file-level JSDoc updated to describe the Group leaf as a `<CommandButton>`, mirroring the Filter sentence.
- `kanban-app/ui/src/types/kanban.ts` — added `ParamDef.clear_command?: string` mirroring the Rust field; full JSDoc.
- `kanban-app/ui/src/components/perspective-tab-bar.registry-driven.test.tsx` — replaced the legacy `screen.getByRole("button", { name: "Group" })` assertion with explicit "is now gone" checks for both the legacy label and the new label.
- `kanban-app/ui/src/components/perspective-tab-bar.test.tsx` — updated two comments that referenced `<GroupPopoverButton>`.
- `swissarmyhammer-kanban/tests/options_enrichment.rs` — added `perspective_group_command_carries_field_options_when_perspective_in_scope` (positive case) and `perspective_group_command_drops_non_groupable_fields_end_to_end` (negative case, end-to-end through `build_dynamic_sources`).

### Files deleted

- `kanban-app/ui/src/components/group-selector.tsx`
- `kanban-app/ui/src/components/group-selector.test.tsx`
- `kanban-app/ui/src/components/perspective-tab-bar.group-enter.spatial.test.tsx` (the new spatial-nav moniker is built by `<CommandButton>`; the popover-opens-on-Enter behavior is covered by `<CommandButton>`'s own tests).

### Files created

- `kanban-app/ui/src/components/perspective-tab-bar.group-migration.test.tsx` — seven cases: icon, popover renders backend options, picking dispatches with the right args, active highlight when perspective.group is set, "(none)" affordance renders when `clear_command` is present, picking "(none)" dispatches `perspective.clearGroup`, placeholder stays "Pick…" when no `clear_command`.

## Acceptance Criteria

- [x] `perspective.group` YAML carries `tab_button: { icon: "group" }` and `params[0].shape: enum, options_from: "perspective.fields"`.
- [x] Emitted `perspective.group` from `commands_for_scope` carries the resolved option list when a perspective is in scope; the option list matches the perspective's field set.
- [x] `<GroupPopoverButton>` is deleted; the tab bar's Group affordance is the registry-rendered `<CommandButton>` + `<CommandPopover>`.
- [x] Clicking the button → popover → picking a field dispatches `perspective.group` with the picked field and the scope-resolved perspective id.
- [x] `isActive` highlight matches today's behavior (`Boolean(perspective.group)`).
- [x] Existing palette/right-click tests for `perspective.group` and `perspective.clearGroup` continue to pass — this task changes the rendering, not the dispatch contract.
- [x] `cargo test --workspace` and `pnpm -C kanban-app/ui test perspective-tab-bar group` both pass.

## Tests

- [x] Frontend regression `kanban-app/ui/src/components/perspective-tab-bar.group-migration.test.tsx`:
  - [x] `group_command_button_renders_with_group_icon`
  - [x] `group_popover_renders_field_options_from_command_emission`
  - [x] `picking_a_group_field_dispatches_perspective_group_with_field_arg`
  - [x] `group_button_is_active_when_perspective_has_a_group_set`
  - [x] `group_popover_renders_none_option_when_clear_command_present`
  - [x] `picking_none_in_group_popover_dispatches_perspective_clearGroup`
  - [x] `group_popover_keeps_pick_placeholder_when_no_clear_command`
- [x] Backend integration test `perspective_group_command_carries_field_options_when_perspective_in_scope` in `swissarmyhammer-kanban/tests/options_enrichment.rs`, plus end-to-end negative case `perspective_group_command_drops_non_groupable_fields_end_to_end` that exercises the `groupable` filter through `build_dynamic_sources`.
- [x] Unit tests in `swissarmyhammer-kanban/src/dynamic_sources.rs` pin `denormalize_perspective_fields`: groupable kept in order; non-groupable dropped; caption does not bypass the filter; caption survives for groupable fields; unknown ids dropped; empty FieldsContext returns empty.
- [x] Update / remove the existing `group-popover-button.test.tsx` (no such file existed — covered by deletion of `<GroupPopoverButton>` in `perspective-tab-bar.tsx`) and `perspective-tab-bar.group-enter.spatial.test.tsx` (deleted — superseded by `<CommandButton>` generic test coverage).
- [x] Run: `cargo test --workspace` (passes; one pre-existing flaky `swissarmyhammer-cli` doctor `test_lsp_servers_check` test fails only under workspace parallelism and passes in isolation — unrelated to this task, environment-dependent) and `pnpm -C kanban-app/ui test` (2141 tests across 228 files pass).

## Workflow

- Use `/tdd` — write the popover-renders-options and picking-dispatches tests first, let them fail, then change the YAML and delete the hardcoded button. ✓ Done.
- Decide early whether `<GroupSelector>` survives as a renderer override or gets deleted. ✓ Decided to delete — see Implementation notes above.
- The spatial moniker shape changes (`perspective_tab.group:` → `perspective_tab.perspective.group:`). ✓ Old test deleted, new shape covered by `<CommandButton>`'s own test coverage. #command-driven-ui

## Review Findings (2026-05-13 14:25)

### Warnings

- [x] `swissarmyhammer-kanban/src/dynamic_sources.rs::denormalize_perspective_fields` and `swissarmyhammer-perspectives/src/options_resolvers.rs::PerspectiveFieldsResolver::resolve` — **the `groupable` filter does not exist anywhere on the new path.** The legacy `<GroupSelector>` filtered to `f.groupable === true` so users only saw fields they could actually group by. The task description states this filter "moved to backend `PerspectiveFieldsResolver`/`gather_perspective_fields`"; that is not what the code does. `denormalize_perspective_fields` returns every perspective field with a resolvable display name, and `PerspectiveFieldsResolver` passes the list through unchanged. `FieldDef.groupable` exists on the entity schema (see `swissarmyhammer-kanban/src/defaults.rs`) but is not consulted on the gather path. Net effect: the new Group By popover surfaces fields the user can't usefully group on (e.g. text-heavy fields, IDs). Fix: filter `perspective.fields[].field` against `fields_ctx.get_field_by_id(...).groupable == Some(true)` inside `denormalize_perspective_fields`, then update the resolver test fixture so it can pin the filter (e.g. add a non-groupable field and assert it is dropped). The frontend `group-migration.test.tsx` does not need to change because the test fixtures already pass groupable-shaped fields, but the backend `perspective_group_command_carries_field_options_when_perspective_in_scope` test should be extended to cover the negative case.
  - **Resolution:** `denormalize_perspective_fields` now consults `fields_ctx.get_field_by_id(...).groupable == Some(true)` and drops any field that fails the predicate. Six new unit tests in `dynamic_sources.rs` pin the contract; new end-to-end integration test `perspective_group_command_drops_non_groupable_fields_end_to_end` in `tests/options_enrichment.rs` exercises the filter through `build_dynamic_sources` + `commands_for_scope` using real builtin fields (groupable `assignees` vs. non-groupable `title`).

### Nits

- [x] `kanban-app/ui/src/components/perspective-tab-bar.tsx` JSDoc on `ScopedPerspectiveTab` — "while the group icon remains a Pressable leaf (`perspective_tab.group:{id}`)". Stale after this migration; the group icon is now a `<CommandButton>` and the moniker is `perspective_tab.perspective.group:{id}`. Rewrite as a parallel of the filter-affordance sentence above it ("the group affordance is now a `<CommandButton>` leaf (`perspective_tab.perspective.group:{id}`)").
  - **Resolution:** Rewrote the paragraph to describe both Filter and Group as `<CommandButton>` leaves at `perspective_tab.perspective.{filter.focus,group}:{id}`.
- [x] `kanban-app/ui/src/components/perspective-spatial-nav.guards.node.test.ts` file-level JSDoc — "(and, when rendered, the `perspective_tab.perspective.filter.focus:{id}` `<CommandButton>` leaf … and the `perspective_tab.group:{id}` Pressable icon button) FocusScope leaves." Stale after this migration: the group icon is no longer a Pressable. Mirror the filter wording: "and the `perspective_tab.perspective.group:{id}` `<CommandButton>` leaf". The actual `it()` assertions in the file are unaffected — only the file-level descriptive comment.
  - **Resolution:** File-level JSDoc rewritten to describe both Filter and Group leaves as `<CommandButton>`s with their parallel `perspective_tab.perspective.{filter.focus,group}:{id}` monikers.
- [x] "None" / clear-group UX — the legacy `<GroupSelector>` had a "None" entry that dispatched `perspective.clearGroup` inline. After the migration the only way to clear a group is right-click → "Clear Group" (context-menu entry). The task description marks this as intentional and out of scope, and the end state is reachable, but the discoverability is materially lower than the legacy single-popover flow. Consider adding a "None" / "(no grouping)" entry as the first `<option>` in the enum picker (value `""`, label `"None"`) that, when selected and submitted, dispatches `perspective.clearGroup` instead of `perspective.group` — handled at the `<CommandPopover>` commit boundary so the dispatch redirection stays out of the YAML. If not adopted now, capture as a follow-up so the regression doesn't bake in across the rest of the epic's migrations.
  - **Resolution:** Adopted via a new reusable YAML annotation `clear_command: "<command-id>"` on `ParamDef` (both Rust and TypeScript sides). When set, `<CommandPopover>` renders "(none)" as the first option in the enum select and treats the empty-string slot as a submittable value; `<CommandButton>.handleCommit` intercepts the empty-string sentinel and dispatches the `clear_command` target (with the sentinel stripped from the args bag) instead of the parent command. The Group YAML now carries `clear_command: "perspective.clearGroup"` on its `group` param, restoring the legacy single-popover "None to clear" affordance. The Sort migration can reuse the same annotation if it lands a similar UX need — the redirection stays out of the YAML body and stays consistent across surfaces. Three new frontend tests pin the contract: "(none)" renders when `clear_command` is set; picking it dispatches `perspective.clearGroup` (not `perspective.group`) with no `group` arg; the "Pick…" placeholder is preserved when `clear_command` is absent.

## Review Findings (2026-05-13, iter 2)

The warning fix and three nit fixes all verify cleanly:

- **groupable filter (warning):** `denormalize_perspective_fields` checks strict equality `field_def.groupable != Some(true)` (not `is_some()`). Six unit tests pin both directions. The end-to-end test `perspective_group_command_drops_non_groupable_fields_end_to_end` uses real builtin fields: `assignees` (id `00000000000000000000000005`, `groupable: true` in `builtin/definitions/assignees.yaml`) MUST appear; `title` (id `00000000000000000000000001`, no `groupable` annotation in `builtin/definitions/title.yaml`) MUST NOT appear. Both directions explicitly asserted.
- **Stale JSDoc (nits 1+2):** Both rewrites are in place. Remaining `perspective_tab.group:{id}` references in `perspective-tab-bar.tsx` (the "deleted `<FilterFocusButton>` and `<GroupPopoverButton>` monikers are gone" block, and the "moniker shape changed: old → new" annotation) are deliberate historical-deletion documentation, not stale references.
- **`clear_command` design (nit 3):** Field shape is sound. Sentinel `""` does not collide because `PerspectiveFieldsResolver` emits ULIDs (never empty). Submit-disabled rule explicitly gates on `clear_command === undefined`, preserving Filter's old behaviour (Filter tests 5/5 pass). Sort can reuse — only one clear-capable enum at the redirect boundary is supported, and the submit-disabled rule still gates the non-clear-capable enum.

Two gaps relative to the iter-2 verification list, however, deserve follow-up tests. They are not functional defects (the code paths work, as the passing tests show), but the reviewer specifically asked for these contracts to be pinned. Both are nit-grade — capturing here so the next migration (Sort, which intends to reuse `clear_command`) doesn't compound the drift.

### Nits

- [x] **YAML round-trip for `clear_command: Some(...)` is not pinned.** `swissarmyhammer-commands/src/types.rs::command_def_with_param_shape_and_options_round_trips` only round-trips `clear_command: None`, so the serialization-with-value path is untested. The serde attributes are correct (`#[serde(default, skip_serializing_if = "Option::is_none")]` + plain `Option<String>`) and the field works end-to-end, but the contract isn't pinned. Add a tiny round-trip test that builds a `ParamDef { clear_command: Some("perspective.clearGroup".into()), .. }`, runs it through `serde_yaml_ng::to_string` → `from_str`, and asserts the field survives. The Sort migration will silently regress here if it lands while this field is invariant-free.
  - **Resolution:** Added `command_def_with_param_clear_command_round_trips` in `swissarmyhammer-commands/src/types.rs` — builds a `ParamDef` with `clear_command: Some("perspective.clearGroup".into())`, runs it through `serde_yaml_ng::to_string` → `from_str`, and asserts both the full struct equality and the explicit `clear_command.as_deref()` value survive. Mirrors the existing `command_def_with_param_shape_and_options_round_trips` naming/style so the Sort migration's reviewer finds both halves of the contract together.
- [x] **YAML-to-component plumbing for `clear_command` is not pinned with the real YAML value.** The three new `group-migration.test.tsx` tests inject a fixture object literal carrying `clear_command: "perspective.clearGroup"` — they don't exercise the path that loads `perspective.yaml` via `CommandsRegistry::from_yaml_sources`, runs `commands_for_scope`, and asserts the emitted `perspective.group` ParamDef carries `clear_command == Some("perspective.clearGroup")`. The existing `perspective_group_command_carries_field_options_when_perspective_in_scope` test in `tests/options_enrichment.rs` is the right place to add the assertion — it already loads the registry and finds the command. One extra line (`assert_eq!(group_param.clear_command.as_deref(), Some("perspective.clearGroup"));`) is enough to pin the YAML-to-emission plumbing.
  - **Resolution:** Extended `perspective_group_command_carries_field_options_when_perspective_in_scope` in `swissarmyhammer-kanban/tests/options_enrichment.rs` with the requested assertion — after the existing `options_from` check, the test now asserts `group_param.clear_command.as_deref() == Some("perspective.clearGroup")`. Pins the full builtin-YAML → `CommandsRegistry::from_yaml_sources` → `commands_for_scope` → emitted-`ParamDef` plumbing for the annotation.