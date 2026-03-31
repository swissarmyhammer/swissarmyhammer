---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffaf80
title: Board name autosave doesn't update window title or inspector while typing
---
## What

Bug: Editing the board name — the debounced autosave fires but the window title bar and inspector don't visually update until the editor is closed. The user expects to see the title and inspector reflect changes as they type (after the 1s debounce).

Save-on-exit works fine. The issue is that the autosave path doesn't trigger the same UI refresh cascade.

### Diagnosis chain

The autosave fires: `TextEditor onChange` → `Field.debouncedOnChange` → `useDebouncedSave` (1s) → `updateField` → `backendDispatch("entity.update_field")` → Rust `dispatch_command`.

After the Rust command completes:
1. **Window title**: `flush_and_emit_for_handle` → `board_display_name` → `update_window_title` (`commands.rs:1260-1271`)
2. **Frontend**: `flush_and_emit` emits "board-changed" Tauri event → entity store refreshes → `useFieldValue("board", ..., displayFieldName)` in `board-selector.tsx:60` re-renders

Investigate why one or both of these don't trigger during autosave but do trigger on commit-on-exit. Possible causes:
- The autosave `updateField` call is the same as commit's — both go through `backendDispatch`. So the Rust side should behave identically. Check if it does.
- The entity store refresh might be suppressed while the editor is open (e.g. the Field component's `useFieldValue` returns the stale value because it's in editing mode)
- The window title update in Rust might not fire for some reason on the autosave path

### Files to investigate
- `kanban-app/ui/src/lib/use-debounced-save.ts` — verify the debounced save actually fires (add logging)
- `kanban-app/ui/src/lib/field-update-context.tsx` — `updateField` is the same for both paths
- `kanban-app/ui/src/components/board-selector.tsx:60` — `useFieldValue` subscription for board name
- `kanban-app/ui/src/lib/entity-store-context.tsx` — does the entity store refresh when "board-changed" fires during editing?
- `kanban-app/src/commands.rs:1260` — window title refresh after dispatch

### Approach
1. Add `console.warn` instrumentation to confirm the debounced save fires
2. Check the macOS unified log (`log show --predicate 'subsystem == "com.swissarmyhammer.kanban"'`) for the Rust-side dispatch
3. Trace whether "board-changed" event is emitted and whether the entity store processes it
4. Fix whatever link in the chain is broken

## Acceptance Criteria
- [ ] While typing the board name (after 1s debounce), the window title bar updates to reflect the new name
- [ ] While typing the board name (after 1s debounce), the board-selector shows the updated name
- [ ] While typing the board name (after 1s debounce), if the inspector is open on the board entity, it shows the updated name
- [ ] Commit-on-exit behavior unchanged

## Tests
- [ ] Manual test: edit board name, wait 2s (longer than debounce), verify title bar updates without closing editor
- [ ] `pnpm --filter kanban-app test` passes