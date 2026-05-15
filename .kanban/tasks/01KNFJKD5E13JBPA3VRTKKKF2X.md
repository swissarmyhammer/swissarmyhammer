---
assignees:
- claude-code
depends_on:
- 01KNFJJAVBBGNMBW83TDYEEF9E
position_column: done
position_ordinal: ffffffffffffffffffffffffffd780
title: Extend Rust task.move to handle group field updates atomically
---
## What

Extend the Rust `MoveTaskCmd` to accept optional group context so that moving a card across groups is a single atomic backend operation — no client-side field update logic.

### Files to modify

1. **`swissarmyhammer-kanban/src/commands/task_commands.rs`** — `MoveTaskCmd`:
   - Add three optional args: `group_field: Option<String>`, `group_value: Option<String>`, `source_group_value: Option<String>`
   - After the existing column + ordinal move, if `group_field` is present:
     - Load the task's current field value for `group_field`
     - **Single-value field**: set to `group_value` (or clear if `group_value` is empty string)
     - **Array field**: remove `source_group_value` from array (if present), add `group_value` (if non-empty and not already present)
     - Persist the field change as part of the same operation
   - If `group_field` is absent, behavior is identical to today (backward compat)
   - Both the move and the field update should be in the same undo entry

2. **`kanban-app/ui/src/components/board-view.tsx`** — `persistMove`:
   - Read `groupField` from `useActivePerspective()`
   - Read `descriptor.groupValue` from the drop zone
   - Read `sourceGroupValue` from drag state
   - Pass all three as args to `task.move` — NO client-side field computation
   - Frontend is a dumb data passer: perspective says the field name, descriptor says target value, drag state says source value

3. **`kanban-app/ui/src/lib/group-utils.ts`** — remove `computeGroupFieldUpdate` (previously planned). All logic is in Rust.

### Frontend sends, Rust decides

```typescript
// Frontend — board-view.tsx persistMove
args: {
  id: taskId,
  column: descriptor.columnId,
  before_id: descriptor.beforeId,
  after_id: descriptor.afterId,
  // Group context — only when grouping is active:
  group_field: groupField,           // e.g. \"tags\"
  group_value: descriptor.groupValue, // e.g. \"feature\" (target)
  source_group_value: dragState.sourceGroupValue, // e.g. \"bug\" (source)
}
```

```rust
// Rust — MoveTaskCmd
if let Some(field) = args.get(\"group_field\") {
    let target = args.get(\"group_value\");
    let source = args.get(\"source_group_value\");
    // Read current value, compute update, persist
}
```

### Why single command, not two

- Atomic undo: one Ctrl+Z undoes both the move and the group change
- No race condition between move and field update
- Follows the \"commands in Rust\" principle — frontend passes data, backend owns logic

## Acceptance Criteria

- [ ] `task.move` without group args works identically to before (backward compat)
- [ ] `task.move` with group args for single-value field sets the field
- [ ] `task.move` with group args for array field removes source value, adds target value
- [ ] Moving to \"(ungrouped)\" (empty group_value) removes source from array / clears single value
- [ ] Moving from \"(ungrouped)\" (empty source) adds target to array / sets single value
- [ ] Single undo entry for move + group field change
- [ ] Frontend persistMove passes group args through without computing anything
- [ ] `cargo test -p swissarmyhammer-kanban` passes

## Tests

- [ ] Rust unit test: `task.move` with `group_field=project, group_value=alpha` sets project field
- [ ] Rust unit test: `task.move` with `group_field=tags, source=bug, target=feature` swaps in array
- [ ] Rust unit test: `task.move` with `group_field=tags, source=bug, target=\"\"` removes bug from array
- [ ] Rust unit test: `task.move` with `group_field=tags, source=\"\", target=feature` adds feature to array
- [ ] Rust unit test: `task.move` without group args — no field change (backward compat)
- [ ] Rust unit test: undo reverses both move and field change
- [ ] `cargo test -p swissarmyhammer-kanban` passes

## Workflow

- Use `/tdd` — write failing tests first, then implement to make them pass.">
</invoke>