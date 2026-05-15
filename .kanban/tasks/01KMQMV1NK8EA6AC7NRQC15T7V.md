---
assignees:
- claude-code
depends_on:
- 01KMQMTAMHZHA79PTZAB453KYT
position_column: done
position_ordinal: ffffffffffffffc380
title: Migrate inspector field navigation to claimWhen
---
## What

Replace the inspector's push-based cursor (`useInspectorNav.focusedIndex` + `inspector.moveUp/Down`) with pull-based `claimWhen` on each field row FocusScope.

### How it works

Each field row FocusScope declares: \"claim focus on `nav.down` if the field above me is focused, claim on `nav.up` if the field below me is focused.\" EntityInspector computes prev/next field monikers and passes `claimWhen` to each FieldRow's FocusScope.

### Files to modify

- **`kanban-app/ui/src/components/entity-inspector.tsx`** — compute prev/next field monikers per row, pass `claimWhen` to each FieldRow's FocusScope. Add `nav.first` (claim if any sibling field is focused and I'm first) and `nav.last`.
- **`kanban-app/ui/src/components/inspector-focus-bridge.tsx`** — remove `inspector.moveUp`, `inspector.moveDown`, `inspector.moveToFirst`, `inspector.moveToLast`, `inspector.nextField`, `inspector.prevField` commands. Keep `inspector.edit`, `inspector.editEnter`, `inspector.exitEdit` (mode commands, not navigation).
- **`kanban-app/ui/src/hooks/use-inspector-nav.ts`** — remove `focusedIndex`, `moveUp`, `moveDown`, `moveToFirst`, `moveToLast`, `setFocusedIndex`, `pillIndex`, `pillCount`, `setPillCount`, `movePillLeft`, `movePillRight`. Keep only `mode` and `enterEdit`/`exitEdit`.
- **`kanban-app/ui/src/components/entity-inspector.tsx`** — remove FocusClaim (no longer needed — each field row's claimWhen handles focus). The initial field gets focused because it claims on mount when no inspector field is focused (or on `nav.down` from outside).

### Entry into inspector

When the inspector opens, the first field should claim focus. Add a `claimWhen` on the first field: `{ command: \"nav.down\", when: (f) => f is not any inspector field moniker }` — or simpler, the FocusClaim already claims focus for the first field on mount. Keep FocusClaim for the initial mount case, but subsequent navigation is all claimWhen.

## Acceptance Criteria

- [ ] j/k (vim) and ArrowUp/ArrowDown (CUA) move between inspector fields via claimWhen
- [ ] Home/End and `g g`/G jump to first/last field
- [ ] Tab/Shift+Tab still work (CUA field navigation)
- [ ] Edit mode (i/Enter) still works
- [ ] `data-focused` moves between field rows correctly
- [ ] No `focusedIndex` state — navigation is purely claim-based

## Tests

- [ ] `inspector-focus-bridge.test.tsx` — nav.down from first field focuses second field
- [ ] `inspector-focus-bridge.test.tsx` — nav.up from second field focuses first field
- [ ] `inspector-focus-bridge.test.tsx` — nav.down from last field stays on last (clamp)
- [ ] `entity-inspector.test.tsx` — first field has data-focused on mount
- [ ] `pnpm vitest run` passes"