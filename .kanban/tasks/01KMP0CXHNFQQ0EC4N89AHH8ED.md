---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffd980
title: Replace FocusHighlight with FocusScope in inspector field rows
---
## What

EntityInspector field rows use `<FocusHighlight focused={...}>` directly, driven by the inspector nav cursor — a parallel focus system that bypasses FocusScope. This violates the 'FocusScope is the single focus decorator' principle.

## Current

Each FieldRow wraps in `<FocusHighlight focused={index === nav.focusedIndex}>`. The `focused` prop comes from local inspector state, not entity focus.

## Fix

Each field row should be a `<FocusScope moniker={fieldMoniker(entityType, entityId, fieldName)}>`. The inspector's `FocusClaim` should update its moniker to include the focused field name. When the cursor moves between fields, `FocusClaim` updates to e.g. `task:id.tags`, and the matching field row's FocusScope shows `data-focused`.

This also unblocks pill navigation: when the inspector focuses a tags field (`task:id.tags`), pill FocusScopes inside that field are children of the field's scope. Pressing h/l can `setFocus` on individual pill monikers while inspector commands stay in the scope chain.

## Approach

1. In `entity-inspector.tsx`: Replace `<FocusHighlight>` with `<FocusScope moniker={fieldMoniker(...)}>` for each FieldRow
2. In `inspector-focus-bridge.tsx`: Change `FocusClaim` moniker from `inspector:type:id` to `type:id.fieldName` based on the focused field
3. Remove `focused` prop threading through FieldRow/Field/Display (no longer needed — FocusScope handles it)

## Acceptance Criteria

- [ ] Inspector field navigation shows `data-focused` via FocusScope, not FocusHighlight
- [ ] No direct `<FocusHighlight>` in entity-inspector.tsx
- [ ] Pill FocusScopes are children of the field FocusScope in the scope chain
- [ ] Inspector j/k navigation still works
- [ ] `pnpm vitest run` passes"