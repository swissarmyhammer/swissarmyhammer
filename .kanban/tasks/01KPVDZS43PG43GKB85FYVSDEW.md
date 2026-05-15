---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffff8b80
title: '"Clear Sort" should clear ALL sort entries on the active perspective — drop required `field` arg'
---
## What

Invoking "Clear Sort" (`perspective.sort.clear`) from the command palette or context menu fails with `MissingArg("field")`. The user-facing contract — both the command's name and its place in the palette/context-menu (no field picker UI) — means "clear all sort entries on this perspective," not "clear the sort for a specific field." The current backend demands a `field` arg that nothing supplies.

**Root cause** (confirmed by tracing):

- `swissarmyhammer-commands/builtin/commands/perspective.yaml:100-110` declares `perspective.sort.clear` with two `from: args` params, `field` and `perspective_id`.
- `swissarmyhammer-kanban/src/commands/perspective_commands.rs:467-496` (`ClearSortCmd::execute`):
  - `perspective_id` is now resolved via `resolve_and_persist_perspective_id` (fallback chain: explicit arg → scope chain → UIState → first perspective for view kind — fixed in `01KPTJR0Y9F1ZEGS439HYY07BF`, already merged).
  - `field` is still loaded with `ctx.arg("field")…ok_or_else(|| MissingArg("field"))` — a hard requirement. Palette and context-menu dispatches never supply it, so the command always fails there.
- The implementation uses `existing_sort.into_iter().filter(|e| e.field != field).collect()` — removes one entry, which is semantically duplicated by `perspective.sort.toggle`'s state cycle (asc → desc → none at `perspective_commands.rs:499+`). A dedicated "remove single field" entry point is not needed.

**Verified frontend callers of `perspective.sort.*`**:
- `kanban-app/ui/src/components/data-table.tsx:332` — `useDispatchCommand("perspective.sort.toggle")` for column-header clicks. This is the only frontend caller, and it targets `toggle`, not `clear`.
- `perspective.sort.clear` has NO explicit frontend caller today. Every reachable invocation is from the palette/context-menu with no args. Dropping the `field` arg breaks zero existing callers.

## Approach

Make `perspective.sort.clear` unconditionally clear the full sort list on the resolved perspective.

1. **`swissarmyhammer-commands/builtin/commands/perspective.yaml`** (lines 100-110): remove the `field` param from the declaration:
   ```yaml
   - id: perspective.sort.clear
     name: Clear Sort
     scope: "entity:perspective"
     undoable: true
     context_menu: true
     params:
       - name: perspective_id
         from: args
   ```
   The `perspective_id` param stays declared `from: args` to match the pattern of the other perspective mutation commands; the backend's resolver fallback handles the common "no args supplied" case.

2. **`swissarmyhammer-kanban/src/commands/perspective_commands.rs:459-497`** (`ClearSortCmd::execute`): delete the `field` read and the `existing_sort.filter` call. Dispatch `UpdatePerspective::new(&perspective_id).with_sort(vec![])` directly.
   ```rust
   async fn execute(&self, ctx: &CommandContext) -> swissarmyhammer_commands::Result<Value> {
       let kanban = ctx.require_extension::<KanbanContext>()?;
       let perspective_id = resolve_and_persist_perspective_id(ctx, &kanban).await?;
       let op = UpdatePerspective::new(&perspective_id).with_sort(Vec::new());
       run_op(&op, &kanban).await
   }
   ```
   Also update the rustdoc at lines 452-458 from "Clear a sort entry for a specific field" to "Clear every sort entry on the active perspective. Multi-field perspectives are reset to unsorted."

3. **Existing test `test_clear_sort_cmd_removes_field`** at `perspective_commands.rs:929-960`: rename to `test_clear_sort_cmd_removes_all_entries` and rewrite to set up a two-field sort (`title` asc + `priority` desc), dispatch `ClearSortCmd` with no `field` arg, and assert both entries are gone. This is both the coverage for the new semantics and the regression guard for the bug the user reported.

**Per-field removal remains available** via `perspective.sort.toggle` (which cycles asc → desc → none) — no capability is lost. The column-header UI already uses toggle exclusively; no frontend change is needed.

## Acceptance Criteria

- [x] Invoking "Clear Sort" from the command palette on a grid with any perspective active clears all sort entries on that perspective (no `MissingArg` error, no toast).
- [x] Invoking "Clear Sort" from a perspective tab's right-click context menu produces the same effect.
- [x] A perspective with multiple sort entries (e.g. `title asc, priority desc`) becomes unsorted after one dispatch.
- [x] A perspective with no sort entries is a no-op — no error, and an empty sort list remains empty.
- [x] `perspective.sort.toggle` behavior is unchanged — column-header clicks still cycle through none → asc → desc → none for the clicked field.
- [x] Undo restores the previous sort list (existing `UpdatePerspective` is `undoable: true`; `undoable: true` flag preserved in YAML).
- [x] The frontend `DataTable` header interactions continue to work (no regression in `data-table.tsx:332`).

## Tests

- [x] Rewrite `test_clear_sort_cmd_removes_field` → `test_clear_sort_cmd_removes_all_entries` in `swissarmyhammer-kanban/src/commands/perspective_commands.rs`:
  1. Create a perspective, add two sort entries via `SetSortCmd` (`title` asc + `priority` desc).
  2. Dispatch `ClearSortCmd` with only `perspective_id` in args (no `field`).
  3. Assert the returned perspective's `sort` is empty or absent.
- [x] New test `test_clear_sort_cmd_works_from_palette_with_no_perspective_id` in the same file:
  1. Set up a perspective, set it as UIState active, add a sort entry.
  2. Dispatch `ClearSortCmd` with empty args and no scope perspective moniker.
  3. Assert sort is cleared — this confirms the resolver fallback still kicks in for the no-args case.
- [x] Additional test `test_clear_sort_cmd_noop_on_empty_sort` covers the empty-sort no-op acceptance criterion.
- [x] Existing tests still pass:
  - `test_toggle_sort_cmd_cycles_none_asc_desc_none` (`perspective_commands.rs:963-998`) — confirms per-field removal path is unchanged.
  - `test_set_sort_cmd_*` and `test_clear_sort_cmd_available*` at `perspective_commands.rs:1103-1122`.
  - Any palette / context-menu resolution tests in `swissarmyhammer-kanban/src/scope_commands.rs`.
- [x] Run: `cargo nextest run -p swissarmyhammer-kanban perspective_commands` — all 41 passing.

## Workflow

- Use `/tdd`. Start with `test_clear_sort_cmd_removes_all_entries` — it should fail with `MissingArg("field")` against today's code. Remove the `field` requirement in YAML + Rust, then the test passes.
- Do NOT touch `perspective.sort.set` or `perspective.sort.toggle` — both genuinely need `field` (they operate on a specific column by design). The cleanup is scoped to `clear` only.
- Do NOT introduce a new `perspective.sort.clearField` command to restore the per-field-clear semantic. That path is already covered by `toggle` and we don't need a near-duplicate.
- Keep the `undoable: true` and `context_menu: true` flags on `perspective.sort.clear` intact. #bug #perspectives #commands