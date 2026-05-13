---
assignees:
- claude-code
depends_on:
- 01KRE1WT72MJWNGQBVAD4V5VKM
- 01KRE1SSN9AX8R67XC58HHQKKB
- 01KRE7VDF7RXHV39VPEVH23NN4
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffe080
title: Migrate Add Perspective and Sort tab buttons to command-driven rendering
---
## What

Final migration in the epic. Two affordances move from hardcoded UI to registry-rendered `<CommandButton>`s with `<CommandPopover>` pickers:

1. **Add Perspective** (`+` button): annotates the existing `perspective.save` command with `tab_button: { icon: "plus" }`. The picker has one text param (`name`); the existing implicit "Untitled" name behavior moves into the command's default arg if the user just clicks Save without typing.
2. **Sort** (tab affordance on grid views): annotates `perspective.sort.set` with `tab_button: { icon: "arrow-up-down" }`. The picker has two enum params (`field` from `perspective.fields`, `direction` from `sort.directions`). Because `perspective.sort.set` already carries `view_kinds: [grid]`, the button is automatically hidden on board views — the original bug, finally fixed by the same mechanism that handles every other view-restricted button.

This task closes out the epic: the tab bar has zero hardcoded button JSX after it lands.

### Post-refactor crate homes

This task depends on `01KRE7VDF7RXHV39VPEVH23NN4` (relocate DynamicSources and friends). After that refactor, file paths in this task that reference `swissarmyhammer-kanban/...` refer to the post-refactor homes:

- `perspective.save` and `perspective.sort.set` `execute` impls live in `swissarmyhammer-perspectives`.
- `commands_for_scope` and the emission infrastructure live in `swissarmyhammer-commands`.
- `PerspectiveFieldsResolver` and `SortDirectionsResolver` are registered from `swissarmyhammer-perspectives` and `swissarmyhammer-commands` respectively.
- The YAML stays in `swissarmyhammer-kanban/builtin/commands/perspective.yaml`.

### Files to modify

- `swissarmyhammer-kanban/builtin/commands/perspective.yaml`:
  - Find the existing `perspective.save` entry (it carries the new-perspective name as a `name` param today). Annotate:
    ```yaml
    tab_button:
      icon: plus
    params:
      - name: name
        from: picker
        shape: text
      - name: view_id
        from: scope
      # ... existing perspective_id, view fields stay as they are
    ```
    Keep all existing params; the change adds `tab_button` + sets the `name` param's `shape: text` and source `from: picker`.
  - Find `perspective.sort.set` and annotate:
    ```yaml
    tab_button:
      icon: arrow-up-down
    params:
      - name: field
        from: picker
        shape: enum
        options_from: "perspective.fields"
      - name: direction
        from: picker
        shape: enum
        options_from: "sort.directions"
      - name: perspective_id
        from: scope
    ```
    `view_kinds: [grid]` already present from the prior epic — leaves Sort hidden on board.

- `kanban-app/ui/src/components/perspective-tab-bar.tsx`:
  - Delete `<AddPerspectiveButton>` and its `import { Plus } from "lucide-react"` if unused after deletion.
  - The registry path renders both `perspective.save` (Add) and `perspective.sort.set` (Sort, grid only) automatically.
  - No state migration needed — `<CommandPopover>` owns the form state.

- `kanban-app/ui/src/components/command-icon-registry.ts` — add `"arrow-up-down": <ArrowUpDown>` from lucide-react. `plus`, `filter`, `group` should already be there from earlier tasks.

- `kanban-app/ui/src/components/perspective-context.tsx` (or wherever `useAutoCreateDefaultPerspective` lives — flagged in the prior epic as a deferred item): leave it. The auto-create path doesn't touch `<AddPerspectiveButton>` and is independent.

### Behavior

- `+` button on the tab bar visually identical. Clicking opens a small popover with a `name` text input; submit dispatches `perspective.save` with the typed name AND `view_id` from scope (preserving the per-view-id scoping the prior epic introduced). Empty input falls back to the dispatcher's "Untitled" default.
- A new Sort button appears on grid views, hidden on board views (verified by the existing `view_kinds: [grid]` filter). Clicking opens a popover with two dropdowns — field + direction — populated from the backend resolvers. Submit dispatches `perspective.sort.set` with the picked args. This is the affordance the original perspective-sort bug was asking for, now properly registry-driven.

### Out of scope

- The `useAutoCreateDefaultPerspective` auto-create path stays as-is. It's a startup-time helper, not a tab-button affordance.
- Any column-header sort-toggle UI in the grid view (`data-table.tsx`'s `SortingState`) — that's tanstack-table's own column-click sort affordance and is separate from the tab-button surface.
- Polishing the popover styling beyond what `<CommandPopover>` already does.

## Acceptance Criteria

- [x] `perspective.save` YAML carries `tab_button: { icon: "plus" }` and `params.name.shape: text`.
- [x] `perspective.sort.set` YAML carries `tab_button: { icon: "arrow-up-down" }` and both enum params with their `options_from` keys.
- [x] `<AddPerspectiveButton>` is deleted; the `+` affordance is the registry-rendered `<CommandButton>`.
- [x] Submitting the Add popover dispatches `perspective.save` with `name` from the input and `view_id` from scope.
- [x] On a grid view, a Sort `<CommandButton>` appears in the tab bar. On a board view, it does NOT appear — verified by a regression test.
- [x] Submitting the Sort popover dispatches `perspective.sort.set` with the picked `field` + `direction` + scope-resolved `perspective_id`.
- [x] `perspective-tab-bar.tsx` no longer contains any hardcoded affordance JSX (`<FilterFocusButton>`, `<GroupPopoverButton>`, `<AddPerspectiveButton>` are all deleted; the tab bar's button surface is 100% registry-rendered).
- [x] `cargo test --workspace` and `pnpm -C kanban-app/ui test perspective-tab-bar` both pass.

## Tests

- [x] Frontend regression `kanban-app/ui/src/components/perspective-tab-bar.add-and-sort-migration.test.tsx`:
  - `add_perspective_button_renders_with_plus_icon_from_registry`.
  - `submitting_add_popover_dispatches_perspective_save_with_name_and_view_id`.
  - `sort_button_appears_on_grid_view_and_disappears_on_board_view` — fixture with active view kind toggled grid/board; assert the Sort `<CommandButton>` mounts/unmounts. This is the regression test the user's original bug report needed and didn't get.
  - `submitting_sort_popover_dispatches_perspective_sort_set_with_field_and_direction`.
  - `tab_bar_has_no_hardcoded_button_jsx` — search the rendered DOM for any of `<FilterFocusButton>`, `<GroupPopoverButton>`, `<AddPerspectiveButton>` (by data-test attribute or by their unique classnames) — assert absent.
- [x] Backend integration test (in whichever crate owns `commands_for_scope` post-refactor): `perspective_sort_set_command_carries_field_and_direction_options` — emit through `commands_for_scope` with a perspective + a grid view in scope, assert `field.options.len() > 0` (matches perspective fields) AND `direction.options == [{asc, Ascending}, {desc, Descending}]`.
- [x] Update / delete `add-perspective-button.test.tsx` and any `perspective-tab-bar.test.tsx` cases that asserted on the deleted hardcoded components — they're replaced by the new test file.
- [x] Run: `cargo test --workspace` and `pnpm -C kanban-app/ui test perspective-tab-bar` — both green.

## Workflow

- Use `/tdd` — start with `sort_button_appears_on_grid_view_and_disappears_on_board_view` (the original user-visible bug) and `tab_bar_has_no_hardcoded_button_jsx`. Let them fail. Then annotate YAML and delete the hardcoded JSX.
- This task lands the user-visible payoff for the entire epic — the sort affordance that should have appeared from the prior `view_kinds` task. Verify it manually before marking done: launch the app, switch between grid and board views, confirm Sort appears/disappears.
- After this task lands, the perspective tab bar is fully registry-driven. Document that in the implementation notes so future contributors know not to add hardcoded buttons here. #command-driven-ui

## Review Findings (2026-05-13 14:41)

### Blockers

- [x] `swissarmyhammer-kanban/src/commands/perspective_commands.rs:262-265` — `SavePerspectiveCmd::execute` reads `view_id` from `ctx.arg("view_id")` only, with NO fallback to `ctx.resolve_entity_id("view")`. The YAML declares `view_id` as `from: scope_chain, entity_type: view`, and the `<BarRegistryTabButtons>` builds a scope chain `["view:<id>", "board:<id>"]` — but the dispatcher never auto-populates the args bag from scope_chain entries (verified by reading `build_dispatch_context` in `kanban-app/src/commands.rs:1498` — it copies `args` and `scope` separately and the param's `from: scope_chain` declaration is metadata only, not an automatic injection step). Result: when the `+` button popover submits `{ name: "X" }`, `view_id` arrives as `None` and `AddPerspective::new(name, view)` saves a perspective with NO `view_id`. This silently regresses the per-view-id scoping the prior epic introduced — `<AddPerspectiveButton>` (deleted by this task) was previously dispatching `{ name, view: viewKind, view_id: viewId }`, and the legacy `view_id` arg is now dropped. Fix: in `SavePerspectiveCmd::execute`, fall back to `ctx.resolve_entity_id("view").map(String::from)` when `ctx.arg("view_id")` is `None`, mirroring the `resolve_perspective_id` pattern used by `SetGroupCmd`. Add a Rust integration test that calls `SavePerspectiveCmd.execute` with `scope_chain: vec!["view:V1".into()]` and asserts the resulting perspective has `view_id: Some("V1")`. The frontend test `submitting_add_popover_dispatches_perspective_save_with_name` explicitly does not assert view_id (line 376 comment claims "view_id is resolved by the backend from the scope chain" — that claim is wrong; the backend has no scope-chain-to-args injection pass).

  Resolution: `SavePerspectiveCmd::execute` now calls `resolve_active_view(ctx, &kanban)` to recover `view_id` from the scope chain when the args bag does not supply one. The shared helper already implements the explicit-arg → scope-chain precedence used elsewhere in the file. Added `test_save_perspective_cmd_resolves_view_id_from_scope_chain` in `perspective_commands.rs` — it builds a context with `scope_chain: ["view:01JMVIEW0000000000BOARD0"]` and no `view_id` arg, then asserts the persisted perspective carries `view_id: "01JMVIEW0000000000BOARD0"`. Also added `test_save_perspective_cmd_explicit_view_args_override_scope_chain` to pin the explicit-arg-wins-over-scope precedence.

- [x] `swissarmyhammer-kanban/src/commands/perspective_commands.rs:261` — `SavePerspectiveCmd::execute` reads `view` from args and falls back to `"board"`. The YAML for `perspective.save` post-migration does not declare a `view` param at all, and the popover does not collect or send `view`. Pre-migration the legacy `<AddPerspectiveButton>` always sent `{ name, view: viewKind, view_id: viewId }` — so a user clicking `+` on a grid view created a perspective with `view: "grid"`. Post-migration, the `+` button always creates a perspective with `view: "board"` regardless of the active view kind. On grid / list views the new perspective will not appear in `filteredPerspectives` (the `<usePerspectiveTabBar>` filter checks `p.view === viewKind`) and the user-visible behavior is "click +, nothing happens". Fix options: (a) declare `view` in the YAML with `from: scope_chain, entity_type: view` so the same auto-resolve fix from the prior blocker covers it (requires resolving the view kind from `KanbanContext` via the view id, not just the moniker), OR (b) declare `view` as a hidden frontend-injected arg like the legacy button did and have `<CommandButton>` or a thin wrapper inject it at click time. The cleanest fix is to follow the same scope_chain resolution pattern as `view_id` and look up the view's kind via the `DynamicSources` / `ViewInfo` registry. Add a Rust integration test asserting that when `scope_chain: ["view:V1"]` and `DynamicSources.views[0]` has `kind: "grid"`, `SavePerspectiveCmd::execute` saves the perspective with `view: "grid"`, not `"board"`.

  Resolution: the same `resolve_active_view` call that recovers `view_id` also returns the view kind (looked up against `KanbanContext::views()`'s registry via `resolve_kind_from_view_id`). `SavePerspectiveCmd::execute` now uses the resolved kind when `arg("view")` is missing, falling through to `"board"` only when the registry cannot supply one. Added `test_save_perspective_cmd_resolves_view_kind_from_scope_chain` which dispatches against `scope_chain: ["view:01JMVIEW0000000000TGRID0"]` (the builtin grid view) and asserts the persisted perspective carries `view: "grid"` and the matching `view_id`. The shared helper means a single code path covers both view_id and view-kind resolution.

### Warnings

- [x] `kanban-app/ui/src/components/perspective-tab-bar.add-and-sort-migration.test.tsx:376-378` — Test comment claims "view_id is resolved by the backend from the scope chain — not asserted here because the frontend mock doesn't run the scope-chain pass". This is misleading: there is no backend scope-chain-to-args pass in `dispatch_command_internal` or anywhere else. The comment encodes a false invariant that future readers will rely on. Fix: rewrite the comment to acknowledge the real boundary — the backend test `perspective_sort_set_command_carries_field_and_direction_options` (in `options_enrichment.rs`) covers emission-time options enrichment, not dispatch-time arg resolution. The dispatch-time view_id resolution needs its own Rust integration test (see blocker above).

  Resolution: the comment in `add-and-sort-migration.test.tsx` now reads in full: the dispatcher (`SavePerspectiveCmd::execute`) reads `view_id` from the scope chain when the args bag does not supply one; the wire payload from the popover therefore carries only `{ name }`; the dispatch-time `view_id` resolution is pinned by the Rust integration tests cited above. The misleading "backend scope-chain pass" wording is gone.

- [x] `kanban-app/ui/src/components/perspective-tab-bar.view-id-scoping.test.tsx:259-288` — The two removed "clicking '+' on view-X dispatches perspective.save with view_id: view-X" tests were retired with a comment claiming the contract is now enforced by the YAML's `from: scope_chain` + the backend. The comment also says the contract "is now covered end-to-end by the backend integration test that ships with the picker-pipeline commands_for_scope pass". There is no such backend test — `commands_for_scope` emits commands for display in palettes / context menus, it does not exercise the dispatch-time `view_id` injection. The removed tests asserted real production behavior (the legacy button sent view_id) that no new test now covers. Either restore frontend tests that drive the popover end-to-end and stub the IPC to assert `args.view_id` is present, OR add the backend integration test referenced in the blocker above so the contract is pinned somewhere.

  Resolution: the comment in `perspective-tab-bar.view-id-scoping.test.tsx` now cites the three new Rust integration tests (`test_save_perspective_cmd_resolves_view_id_from_scope_chain`, `test_save_perspective_cmd_resolves_view_kind_from_scope_chain`, `test_save_perspective_cmd_explicit_view_args_override_scope_chain`) and explains the architectural boundary: the dispatcher (not `commands_for_scope`) is where scope-chain → args fallback happens, so the contract is pinned in the dispatcher's own test suite. The "no such backend test" pointer is corrected with the actual file path.

- [x] `swissarmyhammer-kanban/src/commands/perspective_commands.rs:1336` — The `test_perspective_mutation_cmds_always_available` test was expanded to include `SavePerspectiveCmd.available(&ctx)` after the migration made it unconditionally available. The test passes a `CommandContext` with no args and no scope, which is fine — but `SavePerspectiveCmd` was previously gated on `name`-arg presence (presumably; the diff is not visible from here). The migration's rationale ("popover collects name → emit-time gating would hide the button") is sound. However, the same rationale could justify hiding the command in palettes when there is no view scope (because without a view scope, the resulting perspective is unscoped and may not appear in any view). Consider whether `SavePerspectiveCmd::available` should remain `true` for the tab-button surface but the palette / context-menu surface should still gate on some scope precondition. Today this is out of scope for this migration's blocker fix, but leaving a note: the always-available change works for the button but means the palette will list "Save Perspective" even in contexts where dispatch will silently produce a `view: "board"`, `view_id: None` perspective that doesn't belong anywhere. Surfacing the command in those contexts is mildly confusing.

  Resolution: the reviewer flagged this as "out of scope for this migration's blocker fix". Considered: differential availability across surfaces (always-true at the tab button, scope-gated at palette/context menu) would require either threading the surface identity into `available(&ctx)` or splitting the command into two registry entries. Both options are larger structural changes that warrant their own task — they touch the palette emission pipeline, not the save dispatcher. The two blocker fixes above already eliminate the worst of the "doesn't belong anywhere" scenario: a palette invocation now resolves `view_id` and `view` from the scope chain (when scope is present) and only falls through to `view: "board", view_id: None` when there's genuinely no view in scope. A follow-up task should be filed if a real user reports surprise from the palette path — the architectural change is too speculative to land in this migration.

### Nits

- [x] `kanban-app/ui/src/components/perspective-tab-bar.tsx:1132-1136` — Comment says "two grid views in the same window would each get their own `perspective_bar.perspective.save:<view_id>` leaf", which is correct, but the surface namespace `perspective_bar` (underscore) doesn't match the surrounding spatial-nav zone `ui:perspective-bar` (hyphen). The naming inconsistency is benign because the zone segment and the leaf surface live in different namespaces, but a future reader might assume they should match. Either flip the surface to `perspective-bar` to align with the zone, or add a one-line comment noting the deliberate separation.

  Resolution: added an explanatory paragraph immediately after the existing comment block: the surface key uses underscore (`perspective_bar`) because it's a `<CommandButton>` registry key; the zone segment uses hyphen (`ui:perspective-bar`) because it's a `FocusLayer` segment. The two strings live in different namespaces and the existing call sites for both shapes follow this convention, so they are deliberately not unified.

- [x] `swissarmyhammer-kanban/builtin/commands/perspective.yaml:36-42` — The `perspective.save` `name` param uses `from: args, shape: text`. This is the correct shape for "args bag at dispatch, text input in the picker", but the task description's pseudo-YAML used `from: picker` (which is not a real `ParamSource` value). The implementer correctly translated `from: picker` into `from: args, shape: text` — worth a one-line comment in the YAML explaining the translation so a contributor copying the task's example doesn't try `from: picker` literally.

  Resolution: added a multi-line comment above the `name` param in `perspective.yaml` explaining the wire format: the popover writes the typed string into the args bag at submit time, so `from: args, shape: text` is the real shape; the task's pseudo-YAML used `from: picker` which is not a real `ParamSource` value; the translation `from: picker` → `from: args, shape: text` happens here. Same convention applies to every other popover-collected param.