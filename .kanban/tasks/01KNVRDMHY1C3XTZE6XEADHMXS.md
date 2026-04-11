---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffab80
title: 'Fix perspective.goto palette commands: dispatch rewrite missing + duplicate entries'
---
## What

Two bugs with the `perspective.goto:<id>` dynamic commands in the command palette:

### Bug 1: Commands do nothing when selected

`dispatch_command_internal` in `kanban-app/src/commands.rs` has a rewrite loop (lines ~973-1020) that handles `window.focus:`, `view.switch:`, and `board.switch:` dynamic prefixes — stripping the suffix and rewriting to the actual command with args. **`perspective.goto:` is missing from this loop.** When the palette dispatches `perspective.goto:p1`, the dispatcher tries to find a command impl for the literal string `\"perspective.goto:p1\"`, which doesn't exist. Silent failure.

**Fix:** Add a rewrite case in `dispatch_command_internal` that strips the `perspective.goto:` prefix and rewrites to `ui.perspective.set` with `perspective_id` arg. The handler `SetActivePerspectiveCmd` (`swissarmyhammer-kanban/src/commands/ui_commands.rs`) already reads `perspective_id` from `ctx.require_arg_str(\"perspective_id\")`.

Pattern to follow (from `view.switch:` rewrite at ~line 982):
```rust
if let Some(perspective_id) = effective_cmd.strip_prefix(\"perspective.goto:\") {
    let mut merged = match effective_args {
        Some(Value::Object(map)) => map,
        _ => serde_json::Map::new(),
    };
    merged.insert(\"perspective_id\".into(), Value::String(perspective_id.to_string()));
    effective_cmd = \"ui.perspective.set\".to_owned();
    effective_args = Some(Value::Object(merged));
    continue;
}
```

### Bug 2: Duplicate \"Default\" perspective entries

`gather_perspectives` in `kanban-app/src/commands.rs` collects ALL perspectives from `pctx.all()` without filtering by the current view kind. If a \"Default\" perspective exists for both \"board\" and \"grid\" views, both show in the palette regardless of which view is active.

**Fix:** Filter perspectives by the active view kind. The active view ID is available from `UIState` (the `active_view` field or similar). Pass the current view kind to `gather_perspectives` and filter `p.view == current_view_kind`.

### Files to modify

- `kanban-app/src/commands.rs` — add `perspective.goto:` rewrite in `dispatch_command_internal`; filter perspectives by view kind in `gather_perspectives`

## Acceptance Criteria
- [ ] Selecting \"Go to Perspective: Alpha\" in the command palette switches to that perspective
- [ ] Only perspectives matching the current view kind appear (no duplicate Default)
- [ ] Existing `view.switch:` and `board.switch:` rewrites still work

## Tests
- [ ] `cargo nextest run -p kanban-app` — add test in commands.rs or integration test: dispatching `perspective.goto:p1` rewrites to `ui.perspective.set` with `perspective_id: \"p1\"`
- [ ] `cargo nextest run -p swissarmyhammer-kanban scope_commands` — existing tests still pass
- [ ] Manual: open palette, select a perspective → perspective switches

## Workflow
- Use `/tdd` — write failing tests first, then implement to make them pass.