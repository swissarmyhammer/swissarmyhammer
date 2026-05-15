---
assignees:
- claude-code
depends_on:
- 01KN2Q6HQN1PYDEQ6XYEMCQSSP
- 01KN2QJZXVSVSBJTNVM87QNWR6
position_column: done
position_ordinal: ffffffffffffffffff9c80
title: 'PERSP-8: perspective.list command for left nav'
---
## What

Expose perspective listing through the command system — NOT a Tauri command. Everything goes through command dispatch.

**Add to `swissarmyhammer-commands/builtin/commands/perspective.yaml`:**
```yaml
- id: perspective.list
  name: List Perspectives
  visible: false
```

**Add to `swissarmyhammer-kanban/src/commands/perspective_commands.rs`:**
- `ListPerspectivesCmd` — implements `Command` trait, delegates to `ListPerspectives` operation via dispatch
- Returns JSON array of `{ id, name, view }` — enough for nav rendering

**Register in `swissarmyhammer-kanban/src/commands/mod.rs`:**
- Add `"perspective.list"` → `ListPerspectivesCmd` to `register_commands()`

**Event emission:**
- Perspective mutation commands (save, delete, filter, group, clearFilter, clearGroup) should emit a `"perspectives-changed"` event via the app handle after executing, so the frontend can re-invoke `perspective.list` reactively

The frontend left nav will call `backendDispatch({ cmd: "perspective.list" })` and listen for `"perspectives-changed"` events to refresh — same pattern as views.

## Acceptance Criteria
- [ ] `perspective.list` command registered and callable through dispatch
- [ ] Returns JSON array with `id`, `name`, `view` per perspective
- [ ] Returns empty array when no perspectives exist
- [ ] Mutation commands emit `"perspectives-changed"` event
- [ ] No Tauri `#[command]` — all through command system

## Tests
- [ ] `test_list_perspectives_cmd_empty` — returns empty array
- [ ] `test_list_perspectives_cmd_after_save` — returns saved perspective
- [ ] `test_perspectives_changed_event` — verify event emission on mutations
- [ ] Run: `cargo test -p swissarmyhammer-kanban commands::perspective`