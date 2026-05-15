---
assignees:
- claude-code
depends_on:
- 01KMWS2PV9EVCK6J7NXVMRJAZG
position_column: done
position_ordinal: ffffffffffffffffca80
title: Implement entity.copy and entity.cut commands
---
## What

Add YAML definitions and Rust implementations for `entity.copy` and `entity.cut` commands.

### Files to modify
- `swissarmyhammer-commands/builtin/commands/entity.yaml` — add 2 YAML command entries
- `swissarmyhammer-kanban/src/commands/clipboard_commands.rs` — new file with CopyCmd + CutCmd
- `swissarmyhammer-kanban/src/commands/mod.rs` — add module, register 2 commands, update count assertion

### YAML definitions

```yaml
- id: entity.copy
  name: Copy
  scope: \"entity:task\"
  undoable: false
  context_menu: true
  keys:
    cua: Mod+C
    vim: y
  params:
    - name: task
      from: scope_chain
      entity_type: task

- id: entity.cut
  name: Cut
  scope: \"entity:task\"
  undoable: true
  context_menu: true
  keys:
    cua: Mod+X
    vim: x
  params:
    - name: task
      from: scope_chain
      entity_type: task
```

Note: vim `x` already bound to `task.untag` (scope: `entity:tag,entity:task`). No conflict — `task.untag` requires tag+task in scope (inner), `entity.cut` requires only task (outer). Scope-based keybinding resolution handles this via `extractScopeBindings` shadowing.

### Rust implementations

**CopyCmd**: `available()` = `has_in_scope(\"task\")`. `execute()` = load task entity, snapshot all fields as JSON, store as `ClipboardState { mode: Copy, ... }` in UIState. Return `{ copied: entity_id }`.

**CutCmd**: `available()` = `has_in_scope(\"task\")`. `execute()` = load task entity, snapshot fields, store as `ClipboardState { mode: Cut, ... }` in UIState, then delete the task (via existing delete operation). Return `{ cut: entity_id }`. Marked `undoable: true` so the delete is wrapped in a transaction.

## Acceptance Criteria
- [ ] YAML entries for entity.copy and entity.cut in entity.yaml
- [ ] CopyCmd snapshots task fields into UIState clipboard with mode=Copy
- [ ] CutCmd snapshots task fields into UIState clipboard with mode=Cut, then deletes the task
- [ ] Both registered in mod.rs, command count updated (29 → 31)
- [ ] Availability tests: available with task in scope, not available without

## Tests
- [ ] `swissarmyhammer-kanban/src/commands/mod.rs` — availability tests for entity.copy and entity.cut
- [ ] `cargo nextest run -E 'rdeps(swissarmyhammer-kanban)'` passes"
<parameter name="assignees">[]