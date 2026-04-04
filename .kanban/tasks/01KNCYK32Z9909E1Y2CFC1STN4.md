---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffee80
title: Fix command palette inspect on grid — cell moniker field suffix causes entity not found
---
## What

When a grid cell is focused and the user opens the command palette and selects "Inspect", the inspector shows "entity not found". The previous fix (card 01KNCSHWHFTVCSZS3H0WRTMDM7) only addressed the double-click path in FocusScopeInner. The command palette path is still broken.

**Root cause:** The palette reads `scope_chain` from `useUIState()` (line 67 of `command-palette.tsx`) and passes it to `list_commands_for_scope`. When a grid cell is focused, the UIState scope chain is `["tag:tag-1.color", "tag:tag-1", "board:board", "window:main"]`. In `commands_for_scope` (`scope_commands.rs:193-244`), each moniker is walked and entity commands are emitted with `target: Some(moniker.clone())`. The cell moniker `tag:tag-1.color` is processed first (innermost), so `ui.inspect` gets `target: "tag:tag-1.color"`. The dedup `seen` set prevents the row-level `tag:tag-1` from emitting a second `ui.inspect`.

When the user selects "Inspect" from the palette, `executeSelectedCommand` (`command-palette.tsx:222`) dispatches `dispatch("ui.inspect", { target: "tag:tag-1.color" })`. The backend's `InspectCmd::execute` (`ui_commands.rs:40-44`) uses `ctx.target` first, pushing `"tag:tag-1.color"` onto the inspector stack. The frontend tries to fetch entity with id `tag-1.color` which doesn't exist → "not found".

**Fix — in `commands_for_scope` (`scope_commands.rs:193-194`):** When processing a moniker, detect field-qualified monikers (id contains `.`) and strip the field suffix for the `target`. A field-qualified moniker like `tag:tag-1.color` should produce `target: "tag:tag-1"`. This way the palette dispatches inspect with the correct entity moniker.

```rust
// In the scope chain walk (line 193-194):
for moniker in scope_chain {
    let Some((entity_type, entity_id)) = moniker.split_once(':') else {
        continue;
    };
    // Strip field suffix from cell monikers: "tag-1.color" → "tag-1"
    let base_id = entity_id.split('.').next().unwrap_or(entity_id);
    let entity_moniker = format!("{entity_type}:{base_id}");
    // Use entity_moniker for target and dedup key instead of raw moniker
```

This also fixes the dedup: `tag:tag-1.color` and `tag:tag-1` would both produce key `("ui.inspect", Some("tag:tag-1"))`, so only one inspect command appears.

**Files to modify:**
- `swissarmyhammer-kanban/src/scope_commands.rs` — `commands_for_scope()` (line 193-244): strip field suffix from moniker when building entity command target and dedup key

## Acceptance Criteria
- [ ] Command palette "Inspect" on a focused grid cell opens the inspector for that row's entity
- [ ] Command palette "Inspect" on a board-view card still works (no field suffix to strip)
- [ ] `commands_for_scope` deduplicates correctly — only one "Inspect Tag" appears, not two (one for cell, one for row)
- [ ] Context menu inspect on grid cell also works (same code path)

## Tests
- [ ] Add test in `scope_commands.rs`: scope chain `["tag:tag-1.color", "tag:tag-1", "board:board"]` → `ui.inspect` target is `"tag:tag-1"` (not `"tag:tag-1.color"`)
- [ ] Existing `scope_commands` tests still pass: `cargo test -p swissarmyhammer-kanban -- scope_commands`
- [ ] `cargo test -p swissarmyhammer-kanban` — full suite passes