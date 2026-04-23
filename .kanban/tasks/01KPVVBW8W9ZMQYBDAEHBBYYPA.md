---
assignees:
- claude-code
position_column: todo
position_ordinal: ff8580
title: Scope sort commands to grid-kind views via generic view_kinds filter
---
## What

`perspective.sort.set` ("Sort Field"), `perspective.sort.clear` ("Clear Sort"), and `perspective.sort.toggle` ("Toggle Sort") currently surface in the palette, context menu, and native menus regardless of the active view. On the board view they are meaningless — the board organises cards by column grouping, not by a sort order, so offering "Sort Field" on a task column does not correspond to any visible behavior. On grid views sort is the primary ordering mechanism and must stay.

The fix is **not** to branch on the command id. Introduce a generic, declarative `view_kinds` filter so any command can declare "only visible in these view kinds," then annotate the three sort commands with `view_kinds: [grid]`. The same mechanism is reusable for future grid-only, board-only, calendar-only, or timeline-only commands without further Rust changes.

### Files to modify

1. `swissarmyhammer-commands/src/types.rs` — extend `CommandDef` with:
   ```rust
   #[serde(default, skip_serializing_if = "Option::is_none")]
   pub view_kinds: Option<Vec<String>>,
   ```
   Document semantics: `None` means available in every view kind (default); `Some(list)` restricts emission to those kinds. Use `String` rather than importing `ViewKind` so `swissarmyhammer-commands` stays independent of `swissarmyhammer-views`.

2. `swissarmyhammer-kanban/src/scope_commands.rs`:
   - Extend `ViewInfo` with `pub kind: String` (serialized lowercase kebab: "board", "grid", "list", "calendar", "timeline", "unknown"). Keep `entity_type: Option<String>` as-is.
   - In `commands_for_scope` (and any command-emission helper that consumes `CommandDef`), add a resolution stage: find the innermost `view:{id}` moniker in the scope chain, look it up in `DynamicSources.views`, and filter out any command whose `view_kinds` list is non-empty and does not contain the resolved view's kind. Commands with `view_kinds = None` are unaffected.
   - When no `view:{id}` is in scope (pure palette context without a view), treat the `view_kinds` constraint as a hard no-match — a sort command scoped to grid should not appear from a board-tab right-click either. Decision rationale: the palette surface always has a `view:{id}` in the scope chain in real usage (ViewContainer wraps every rendered view), so the only way scope lacks a view moniker is from tests or shell-only invocations; in those cases the safe default is "don't offer view-restricted commands."

3. `swissarmyhammer-commands/builtin/commands/perspective.yaml` — add `view_kinds: [grid]` to the three sort entries:
   - `perspective.sort.set`
   - `perspective.sort.clear`
   - `perspective.sort.toggle`

   Do **not** annotate `perspective.filter`, `perspective.clearFilter`, `perspective.group`, `perspective.clearGroup` — those remain valid on every view kind.

4. `kanban-app/src/commands.rs` — in `gather_views`, populate `ViewInfo.kind` from `ViewDef.kind` by serializing the enum to its kebab-case string (the same representation used in `ViewKind`'s `#[serde(rename_all = "kebab-case")]`).

### Non-goals / don't-do

- Do **not** implement this as per-command `available()` checks in `SetSortCmd` / `ClearSortCmd` / `ToggleSortCmd`. `available()` is for runtime argument/state gating; view-kind is a declarative UI-surface filter and belongs in the metadata layer.
- Do **not** encode view kinds as synthetic `view_kind:{kind}` monikers in the scope chain. The scope chain is for entity monikers; view kind is a property of an entity (the view) already in the chain, so resolve it via `DynamicSources.views` rather than duplicating it as a moniker.
- Do **not** change the runtime behavior of sort commands themselves. If a sort command is somehow dispatched against a board-kind perspective (e.g. via MCP or shell), the Rust impl still performs the mutation — the `view_kinds` filter suppresses the command from palettes/menus only, not from the dispatcher. The backend is not the enforcement point for UI-ergonomics rules.
- Do not add `view_kinds` to every existing command preemptively; only the three sort commands.

### Why declarative, not programmatic

The command registry is metadata-driven (see `feedback_metadata_driven_ui` in project memory). Hard-coding "if board, hide sort" anywhere — in Rust `available()`, in React components, or in a dispatcher branch — would be a one-off special case. The `view_kinds` field makes view-kind scoping a first-class declaration that any command YAML can opt into, consistent with the existing `scope:` and `context_menu:` fields.

## Acceptance Criteria

- [ ] `CommandDef` has a `view_kinds: Option<Vec<String>>` field that round-trips through YAML via `serde_yaml_ng`.
- [ ] `ViewInfo` in `swissarmyhammer-kanban/src/scope_commands.rs` carries `kind: String`.
- [ ] With `view:{grid-id}` in the scope chain and `DynamicSources.views` containing that id with `kind = "grid"`, `commands_for_scope` emits `perspective.sort.set`, `perspective.sort.clear`, and `perspective.sort.toggle`.
- [ ] With `view:{board-id}` in the scope chain and `DynamicSources.views` containing that id with `kind = "board"`, `commands_for_scope` does **not** emit the three sort commands, but still emits `perspective.filter`, `perspective.clearFilter`, `perspective.group`, `perspective.clearGroup`.
- [ ] The existing `perspective_mutation_commands_available_from_palette_scope` test continues to pass for the non-sort mutation commands; the sort commands are removed from its assertion set or moved to a grid-scoped variant.
- [ ] Snapshot fixtures under `swissarmyhammer-kanban/tests/snapshots/board_full.json` and `board_context_menu_only.json` are regenerated and no longer contain `perspective.sort.*`; grid-scoped snapshots (new or existing) do contain them.
- [ ] Running the app: on the board view the palette (Cmd+K) and the right-click menu show Filter / Group entries but not Sort Field / Clear Sort / Toggle Sort. Switching to a grid view restores them.

## Tests

- [ ] `swissarmyhammer-commands/src/types.rs` — round-trip test: add a `CommandDef` with `view_kinds: Some(vec!["grid".into()])`, serialize to YAML, parse back, assert equality. Mirror the shape of `command_def_yaml_round_trip`.
- [ ] `swissarmyhammer-commands/src/types.rs` — defaults test: parse minimal YAML without `view_kinds`, assert `def.view_kinds.is_none()`. Mirror `command_def_minimal_yaml`.
- [ ] `swissarmyhammer-kanban/src/scope_commands.rs` — new test `sort_commands_absent_from_board_view_scope`: build `DynamicSources` with a board-kind view, scope chain `["view:board-view", "board:my-board"]`, call `commands_for_scope`, assert the three sort command ids are not in the output.
- [ ] `swissarmyhammer-kanban/src/scope_commands.rs` — new test `sort_commands_present_in_grid_view_scope`: build `DynamicSources` with a grid-kind view, scope chain `["view:tasks-grid"]`, call `commands_for_scope`, assert all three sort command ids are present.
- [ ] `swissarmyhammer-kanban/src/scope_commands.rs` — new test `view_kind_filter_leaves_filter_and_group_commands_alone`: build a board-kind view scope, assert `perspective.filter`, `perspective.clearFilter`, `perspective.group`, `perspective.clearGroup` are all present (regression guard against over-filtering).
- [ ] `swissarmyhammer-kanban/src/scope_commands.rs` — update `perspective_mutation_commands_available_from_palette_scope` so it only asserts non-sort mutation commands on board scope, and add a parallel `perspective_sort_commands_available_from_grid_palette_scope`.
- [ ] Regenerate the affected snapshot JSON files under `swissarmyhammer-kanban/tests/snapshots/` by running `cargo test -p swissarmyhammer-kanban command_surface_matrix -- --nocapture` and inspecting the diff before committing. Snapshots whose scope is board-backed must lose the three `perspective.sort.*` entries.
- [ ] `cargo test -p swissarmyhammer-commands` and `cargo test -p swissarmyhammer-kanban` both pass green.
- [ ] Manual verification: `cargo run --bin kanban-app`; on a board perspective tab, Cmd+K does not list Sort Field / Clear Sort / Toggle Sort; right-click on a task also omits them. Switch the active view to a grid view; all three reappear.

## Workflow

- Use `/tdd` — start with the four scope_commands unit tests (two positive, two negative) as failing, then wire `CommandDef.view_kinds`, `ViewInfo.kind`, and the filter stage until green.
- Regenerate snapshots only after the unit tests pass, and commit the snapshot diff as a separate logical change if it balloons the PR.
- Finish by running the app manually on a board perspective and a grid perspective to confirm both menu surfaces (palette + right-click) behave as expected.
