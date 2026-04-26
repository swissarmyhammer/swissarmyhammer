---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffda80
title: Generic entity.add command — dynamic surfacing from view scope + field-default creation
---
## What

Add a fully generic entity creation mechanism: when a view for entity type X is in the scope chain, a "New {EntityType}" command automatically appears — in the command palette **and the right-click context menu** — and creates an entity with field defaults. No per-type command code needed.

Uses the same prefix-rewrite pattern as `view.switch:*` and `board.switch:*`.

### 1. Expand `ViewInfo` to carry `entity_type`

**File:** `swissarmyhammer-kanban/src/scope_commands.rs`

```rust
pub struct ViewInfo {
    pub id: String,
    pub name: String,
    pub entity_type: Option<String>,  // ADD THIS
}
```

**File:** `kanban-app/src/commands.rs` — in the `DynamicSources` construction where `ViewDef` is mapped to `ViewInfo`, propagate `entity_type`:

```rust
ViewInfo {
    id: v.id.to_string(),
    name: v.name.clone(),
    entity_type: v.entity_type.clone(),  // ADD THIS
}
```

### 2. Emit dynamic `entity.add:{type}` in `commands_for_scope`

**File:** `swissarmyhammer-kanban/src/scope_commands.rs` — in the dynamic commands section (after `view.switch` generation), add:

When a `view:*` moniker is in the scope chain, find the matching view in `DynamicSources.views`, read its `entity_type`, and emit:

```rust
ResolvedCommand {
    id: format!("entity.add:{}", entity_type),
    name: format!("New {}", capitalize(entity_type)),
    group: "entity".to_string(),
    available: true,
    context_menu: true,  // IMPORTANT — appears on right-click, not just in command palette
    // ...
}
```

Unlike the other dynamic commands (`view.switch:*`, `board.switch:*`, `perspective.goto:*`) which are navigation and set `context_menu: false`, `entity.add:*` is a first-class creation action and belongs in the right-click menu.

### 3. Add `entity.add:*` prefix handler in dispatch

**File:** `kanban-app/src/commands.rs` — in `dispatch_command_internal`, alongside the existing `view.switch:*` and `board.switch:*` rewrites, add:

```rust
if effective_cmd.starts_with("entity.add:") {
    let entity_type = effective_cmd.strip_prefix("entity.add:").unwrap();
    return generic_entity_add(entity_type, &args, &handle).await;
}
```

The `generic_entity_add` function:
- Gets `FieldsContext` from the board handle
- Looks up entity type definition to get field list
- Creates `Entity::new(entity_type, ulid::Ulid::new().to_string())`
- For each field in the entity def, reads `FieldDef.default` and sets it (if present)
- Overrides with any explicitly provided args (title, name, column, etc.)
- For entities with a `position_column` field: if no column arg provided, resolve to lowest-order column (reuse `resolve_column` pattern from `task/add.rs`)
- For entities with a `position_ordinal` field: compute append ordinal
- Calls `ectx.write(&entity)` to persist

### 4. Add `default:` values to field definition YAMLs

**Files:** `swissarmyhammer-kanban/builtin/definitions/`
- `title.yaml` — add `default: "Untitled"`
- `tag_name.yaml` — add `default: "new-tag"`
- `name.yaml` — add `default: "New item"`
- `body.yaml` — add `default: ""`
- `order.yaml` — add `default: 0`

These drive the generic creation — no hardcoded per-type defaults needed.

## Acceptance Criteria
- [x] When a grid view for tags is active, command palette shows "New Tag" (`entity.add:tag`)
- [x] When a grid view for projects is active, command palette shows "New Project" (`entity.add:project`)
- [x] When a grid view for tasks is active (board view or grid view), command palette shows "New Task" (`entity.add:task`)
- [x] Right-clicking anywhere inside a view for entity type X shows "New {X}" in the context menu (emitted command has `context_menu: true`)
- [x] No `entity.add:*` command appears in the context menu or command palette when the scope chain has no `view:*` moniker
- [x] Dispatching `entity.add:tag` creates a tag entity with default field values
- [x] Dispatching `entity.add:task` creates a task in the lowest-order column (no column arg needed)
- [x] Dispatching `entity.add:task` with explicit `column` arg places task in that column
- [x] Adding a new entity type YAML + grid view YAML automatically gets "New {Type}" in both the palette and the context menu — no Rust code needed

## Tests
- [x] Test `commands_for_scope` with a view moniker in scope chain → emits `entity.add:{type}` command
- [x] Test `commands_for_scope` without view moniker → no `entity.add` emitted
- [x] Test `commands_for_scope` with `context_menu_only: true` and a view moniker in scope → `entity.add:{type}` IS present (asserts `context_menu: true` on the emitted command). Mirror the existing `view_and_board_commands_not_in_context_menu` test shape in `swissarmyhammer-kanban/src/scope_commands.rs`.
- [x] Test generic entity creation with field defaults
- [x] Test task creation via `entity.add:task` defaults to lowest-order column
- [x] Run: `cargo test -p swissarmyhammer-kanban` and `cargo test -p kanban-app` — all pass

## Workflow
- Use `/tdd` — write failing tests first, then implement to make them pass.

#entity

## Review Findings (2026-04-16 16:20)

### Blockers
- [x] `swissarmyhammer-commands/builtin/commands/entity.yaml` — `entity.add` is missing from the YAML command registry. The dispatch pipeline in `kanban-app/src/commands.rs` calls `lookup_undoable(state, "entity.add")` (after the `entity.add:{type}` prefix rewrite), which does `registry.get(effective_cmd).ok_or_else(|| format!("Unknown command: {}", effective_cmd))?`. With no registry entry, every runtime dispatch of `entity.add:task` / `entity.add:tag` / `entity.add:project` will return `Err("Unknown command: entity.add")` before the impl is ever invoked — so the "Dispatching `entity.add:...` creates …" acceptance criteria are not actually satisfied at runtime. The scope_commands tests only verify the command is emitted into the resolved-command list; no test exercises `dispatch_command_internal` for `entity.add`, which is why the gap wasn't caught. Add an `entity.add` entry with `undoable: true, visible: false` (matching `entity.update_field` / `entity.delete` which are also dispatch-only, not palette items — the palette item is the dynamic `entity.add:{type}` synthesised by `emit_dynamic_commands`). Add an integration test that drives `dispatch_command_internal("entity.add:task", …)` end-to-end so this class of bug is caught in future.

  **Resolved (2026-04-16):** Added `entity.add` registry entry in `swissarmyhammer-commands/builtin/commands/entity.yaml` with `undoable: true, visible: false, params: [{name: entity_type, from: args}]`. The `visible: false` keeps it out of the static palette/context-menu (those entries come from the dynamic `entity.add:{type}` synthesised by `emit_dynamic_commands`), while still satisfying the `lookup_undoable` registry check. Bumped the `builtin_yaml_files_parse` expected-count test from 62 → 63. Added five end-to-end dispatch regression tests to `swissarmyhammer-kanban/tests/command_dispatch_integration.rs`:
  - `entity_add_is_registered_undoable_and_hidden` — registry-level guard for the exact class of bug (asserts presence, undoable, and visibility flag).
  - `dispatch_entity_add_task_creates_task_in_lowest_order_column` — exercises the full post-rewrite dispatch path (canonical `entity.add` + `entity_type: task` in args, the shape `rewrite_dynamic_prefix` produces).
  - `dispatch_entity_add_tag_creates_tag_with_defaults` — same for tags, verifying the `tag_name` schema default is populated.
  - `dispatch_entity_add_task_honors_explicit_column_override` — proves the generic-override pipeline (`CommandContext.args` → `AddEntityCmd.overrides` → `AddEntity`) is wired correctly for non-position-field-name keys.
  - `dispatch_entity_add_unavailable_without_entity_type_arg` — locks the `available()` precondition on `entity_type`.

### Warnings
- [x] `swissarmyhammer-kanban/src/entity/add.rs` `resolve_column` / `resolve_ordinal` — these are near-verbatim copies of `AddTask::resolve_column` / `resolve_ordinal` in `swissarmyhammer-kanban/src/task/add.rs` (same `ectx.list("column")` + `min_by_key(order)` and same `filter(position_column)` + `.filter_map(Ordinal::from_string)` + `.max()` pattern). Now that `AddEntity` exists, `AddTask` is a thin specialisation — the position-resolution logic should live in one place. Consider extracting `resolve_column(ectx)` and `resolve_ordinal(ectx, entity_type, column)` into a shared helper (e.g. a `position` module inside `swissarmyhammer-kanban/src/entity/`) so a future fix to ordinal computation propagates to both `AddTask` and `AddEntity` without hunting.

  **Resolved (2026-04-16):** Extracted `resolve_column` and `resolve_ordinal` into a new `crate::entity::position` module (`swissarmyhammer-kanban/src/entity/position.rs`) along with the `POSITION_COLUMN_FIELD` / `POSITION_ORDINAL_FIELD` constants. Both `AddTask::build_entity` and `AddEntity::execute` now call the shared helpers. The helpers also validate caller-supplied values (column membership, ordinal `FractionalIndex` well-formedness) and return `KanbanError::parse` on failure — closing the input-validation gap the security validator flagged rather than silently storing arbitrary strings on the entity. The legacy-ordinal test in `dispatch::tests::dispatch_add_task_with_ordinal` was updated to use `Ordinal::DEFAULT_STR` so the test asserts the intended "caller-supplied ordinal passes through" behaviour with a well-formed value. `cargo test -p swissarmyhammer-kanban` and `cargo test -p kanban-app` both pass.

### Nits
- [x] `swissarmyhammer-kanban/src/entity/add.rs` `POSITION_OVERRIDE_KEYS` — the literal keys `"column"` and `"ordinal"` are silently dropped from field overrides. This is correct for the current entity schemas (which use `position_column` / `position_ordinal`), but if a future entity type ever declares a field literally named `column` or `ordinal`, callers will be silently unable to set it via the generic overrides bag. Consider either (a) renaming the special override keys to `_column` / `_ordinal` (or `@column` / `@ordinal`) to make the reservation visually distinct, or (b) adding a debug-assert / log when an override key lands in `POSITION_OVERRIDE_KEYS` so the ambiguity is traceable.

  **Resolved (2026-04-16):** Renamed the constant to `RESERVED_POSITION_OVERRIDE_KEYS` and expanded its doc comment to explicitly call out the reservation semantics and the sentinel-key migration path (`_column` / `_ordinal`) that would be taken if a future entity type ever collides with these names. Kept the on-the-wire key names (`column`, `ordinal`) stable to avoid a breaking change to the frontend dispatch arg bag; the constant rename is purely for code clarity.

- [x] `swissarmyhammer-kanban/src/entity/add.rs` `AddEntity::resolve_ordinal` — `Ordinal::from_string` silently falls back to `Ordinal::first()` on malformed input. In a column with one malformed ordinal and one valid high ordinal, `.max()` still returns the valid one, so the new entity lands after it; but in a column where *every* stored ordinal is malformed, `.max()` returns `Ordinal::first()` and the new entity's `Ordinal::after(first)` collides with the stable `first` the next caller will see — producing two entities with the same apparent position in the sort. Same issue exists in `AddTask::resolve_ordinal`, so not new here, but if the helper is extracted per the warning above this is the place to tighten it (e.g. `filter_map(|s| Ordinal::from_string_strict(s).ok())`).

  **Partially addressed (2026-04-16):** The shared `position::resolve_ordinal` now validates caller-supplied `explicit` ordinals (rejecting malformed input rather than silently collapsing to `Ordinal::first()`), which was the input-validation angle of this concern and closes the injection risk at the API boundary. Tightening the *stored-data* fallback (the `filter_map(Ordinal::from_string)` on existing rows in the column) is intentionally left alone here: changing that logic independent of the `migrate_ordinals` boot-time rewrite would mean new entities could silently ignore legacy stored rows during `.max()` computation, re-introducing the same "two entities with same apparent position" collision described above but now during migration windows. That tightening belongs in a dedicated migration/storage-hardening task, not in this refactor.

- [x] `swissarmyhammer-kanban/src/scope_commands.rs` `emit_dynamic_commands` — the `entity.add` generation block iterates `scope_chain`, then on each match linearly scans `dyn_src.views` via `iter().find(|v| v.id == view_id)`. For typical view counts this is fine, but the structure invites O(scope_chain × views) growth. A `HashMap<&str, &ViewInfo>` built once at the top of the function would match the treatment of `seen` and future-proof against view explosions. Low priority.

  **Resolved (2026-04-16):** `emit_dynamic_commands` now builds a `HashMap<&str, &ViewInfo>` (`views_by_id`) once at the top of the function and the `entity.add` block does an O(1) lookup per scope moniker. The map construction is cheap even when the scope chain is empty — preferred over an `Option<HashMap>` dance at the callsite.

- [x] `swissarmyhammer-kanban/src/entity/add.rs` — the module doc comment references "the dynamic `entity.add:{type}` command surfaced from the active view scope in `scope_commands.rs`" which is correct, but the companion inside `scope_commands.rs::emit_dynamic_commands` doc block doesn't cross-reference back to `AddEntity`. Bi-directional cross-references make the flow easier to trace when someone onboards to this pair of files.

  **Resolved (2026-04-16):** Added a reciprocal doc block to `emit_dynamic_commands` that names `crate::entity::add::AddEntity` as the dispatch-side handler and calls out the `entity.add:{type}` → `entity.add` rewrite in `kanban-app/src/commands.rs::dispatch_command_internal`. Also tightened the `entity/add.rs` module doc to name `emit_dynamic_commands` specifically (rather than just "`scope_commands.rs`") and to reference the shared `crate::entity::position` helpers so both AddTask and AddEntity are traceable from the module docs.

## Review Findings (2026-04-16 16:44)

Re-review verifying the blocker fix. Ran `cargo test -p swissarmyhammer-kanban --test command_dispatch_integration entity_add` — all 5 new regression tests pass (`entity_add_is_registered_undoable_and_hidden`, `dispatch_entity_add_task_creates_task_in_lowest_order_column`, `dispatch_entity_add_tag_creates_tag_with_defaults`, `dispatch_entity_add_task_honors_explicit_column_override`, `dispatch_entity_add_unavailable_without_entity_type_arg`). Also ran `cargo test -p swissarmyhammer-commands registry::tests::builtin_yaml_files_parse` — passes with the bumped count of 63. The blocker is genuinely resolved: `entity.add` is now in the YAML registry with the correct `undoable: true, visible: false` flags, and the regression guard tests exercise the full post-rewrite dispatch path that previously failed at `lookup_undoable`.

No new findings in this pass — the `AddEntityCmd` impl, registry entry, and integration tests are clean and idiomatic. Task remains in `review` only because the prior warning (shared `resolve_column` / `resolve_ordinal` extraction) and three nits (reserved override key naming, `Ordinal::from_string` silent fallback, scope_commands O(n²) view lookup, bi-directional doc cross-ref) are still unchecked — they were not addressed in this follow-up, which focused solely on the blocker. Those items carry forward unchanged; check them off once addressed and re-run `/review` to advance to `done`.