---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffffd480
title: Replace plain <input> with TextEditor (CM6) for perspective tab rename
---
## What

`perspective.rename` is defined in YAML (`swissarmyhammer-commands/builtin/commands/perspective.yaml:26-34`) but has NO Rust `Command` impl and is NOT registered in `swissarmyhammer-kanban/src/commands/mod.rs`. The `RenamePerspective` operation exists at `swissarmyhammer-kanban/src/perspective/rename.rs` but nothing wires it to the command dispatch.

### Fix

Follow the `DeletePerspectiveCmd` pattern (perspective_commands.rs:100-138):

1. **Add `RenamePerspectiveCmd`** to `swissarmyhammer-kanban/src/commands/perspective_commands.rs`:
   - Extract `id` and `new_name` from `ctx.require_arg_str()`
   - Construct `RenamePerspective::new(id, new_name)`
   - Call `run_op(&op, &kanban).await`

2. **Register** in `swissarmyhammer-kanban/src/commands/mod.rs` after the `perspective.delete` entry (~line 183):
   ```rust
   map.insert(\"perspective.rename\".into(), Arc::new(perspective_commands::RenamePerspectiveCmd));
   ```

3. **Update count** in `register_commands_returns_expected_count` test (mod.rs:295) from 61 to 62.

4. **Remove diagnostic logging** from `rename.rs` (the `tracing::warn!` lines added during debugging).

## Acceptance Criteria
- [ ] `RenamePerspectiveCmd` struct exists with `Command` trait impl
- [ ] `perspective.rename` registered in command map
- [ ] Dispatching `perspective.rename` with `{id, new_name}` renames the perspective
- [ ] Command count test updated to 62

## Tests
- [ ] `test_rename_perspective_cmd`: create perspective via SavePerspectiveCmd, rename via RenamePerspectiveCmd, verify new name in result
- [ ] `test_rename_perspective_cmd_not_found`: rename nonexistent ID returns error
- [ ] `cargo nextest run -p swissarmyhammer-kanban perspective` — all pass
- [ ] `cargo nextest run -p swissarmyhammer-kanban register_commands` — passes with count 62

## Workflow
- Use `/tdd` — write failing test first, then implement.

#bug