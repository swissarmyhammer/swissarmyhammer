---
assignees:
- claude-code
depends_on:
- 01KKSW5P87FEMVWSN21EHVRV8D
position_column: todo
position_ordinal: '8180'
title: Add percent_complete computed field to board entity
---
## What
Define `percent_complete` as a proper computed field on the board entity, with a `board-percent-complete` derivation that uses the new entity query capability to count done vs total tasks.

### Steps

1. **Field definition YAML** — create `swissarmyhammer-kanban/builtin/fields/definitions/percent_complete.yaml`:
   ```yaml
   id: "..."
   name: percent_complete
   description: Board completion percentage (done tasks / total tasks)
   type:
     kind: computed
     derive: board-percent-complete
   editor: none
   display: number
   section: header
   ```

2. **Board entity YAML** — add `percent_complete` to `swissarmyhammer-kanban/builtin/fields/entities/board.yaml` fields list.

3. **Register derivation** — in `swissarmyhammer-kanban/src/defaults.rs`, register `board-percent-complete` on the `ComputeEngine`. The derive fn:
   - Queries `column` entities via the query fn, sorts by order, identifies terminal column (highest order)
   - Queries `task` entities, counts those in terminal column vs total
   - Returns `json!({ "done": done_count, "total": total_count, "percent": pct })`

4. **Remove hardcoded summary fields** — the `done_tasks` / `percent_complete` data now comes from the board entity's computed field, not from a separate summary block. Update `get_board_data` in `kanban-app/src/commands.rs` and `GetBoard` in `get.rs` to include these in the summary from the board entity's derived field.

### Files
- `swissarmyhammer-kanban/builtin/fields/definitions/percent_complete.yaml` (new)
- `swissarmyhammer-kanban/builtin/fields/entities/board.yaml` (add field)
- `swissarmyhammer-kanban/src/defaults.rs` (register derivation)
- `swissarmyhammer-kanban/src/board/get.rs` (use derived field in summary)
- `kanban-app/src/commands.rs` (use derived field in summary)

## Acceptance Criteria
- [ ] `percent_complete` appears on the board entity when read via `EntityContext::read("board", "board")`
- [ ] Value is `{ done, total, percent }` object with correct counts
- [ ] Terminal column is determined by highest `order` value (same logic as existing `terminal_id`)
- [ ] `summary` JSON includes `done_tasks` and `percent_complete` sourced from the computed field
- [ ] 0 tasks → `{ done: 0, total: 0, percent: 0 }`

## Tests
- [ ] Update `test_empty_board` to assert percent_complete
- [ ] Update `test_board_with_tasks_in_different_columns` to assert done=1, total=3, percent=33
- [ ] `cargo nextest run -p swissarmyhammer-kanban` all pass