---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffff9c80
title: Fix tag pill context menus missing on entity cards — decouple showFocusBar from event handling in FocusScope
---
## What

Tag pills on entity cards (board view) don't show right-click context menus (cut/copy/paste/inspect tag), while the same tag pills in the inspector panel do. The user expects identical behavior in both locations.

**Root cause:** `BadgeListDisplay` passes `showFocusBar={mode === "full"}` to `MentionPill` → `FocusScope`. When `mode="compact"` (entity cards), `showFocusBar` is `false`. Inside `FocusScopeInner` (`focus-scope.tsx`), `handleContextMenu` bails out immediately with `if (!showFocusBar) return;` (line ~179), so the tag-specific context menu never appears.

The problem is that `showFocusBar` controls **two separate concerns**:
1. The visual focus bar indicator (blue highlight on click)
2. Whether click/right-click/double-click events are handled

On compact cards, we want to suppress the visual focus bar (no blue highlight) but still handle right-click to show tag-specific commands.

**Fix:** Decouple event handling from the focus bar visual in `FocusScopeInner`. Add a separate `handleEvents` prop (default `true`) that controls whether click/contextmenu/dblclick are handled, independent of `showFocusBar`.

**Files to modify:**

1. `kanban-app/ui/src/components/focus-scope.tsx` (~line 179)
   - Add `handleEvents?: boolean` prop to `FocusScopeInner` (default: `true`)
   - Change the early returns in `handleContextMenu`, `handleClick`, `handleDoubleClick` from `if (!showFocusBar) return;` to `if (!handleEvents) return;`
   - `showFocusBar` continues to control only the visual indicator

2. `kanban-app/ui/src/components/fields/displays/badge-list-display.tsx` (~line 142)
   - Keep `showFocusBar={mode === "full"}` (visual only)
   - No change needed here if MentionPill always passes `handleEvents={true}` (the default)

3. `kanban-app/ui/src/components/mention-pill.tsx`
   - Verify it passes through to FocusScope correctly — may need no changes if default is `true`

## Acceptance Criteria
- [ ] Right-clicking a tag pill on an entity card shows the tag context menu (cut/copy/paste/inspect)
- [ ] Right-clicking a tag pill in the inspector still shows the same context menu
- [ ] Tag pills on entity cards do NOT show the blue focus bar on click (visual suppression preserved)
- [ ] Tag pills in the inspector continue to show the focus bar on click

## Tests
- [ ] `kanban-app/ui/src/components/focus-scope.test.tsx` — add test: FocusScope with `showFocusBar=false` still fires `onContextMenu` when `handleEvents=true` (default)
- [ ] `kanban-app/ui/src/components/focus-scope.test.tsx` — add test: FocusScope with `handleEvents=false` suppresses context menu
- [ ] `kanban-app/ui/src/components/fields/displays/badge-list-display.test.tsx` — add test: compact mode tag pill fires context menu on right-click
- [ ] `cd kanban-app/ui && pnpm vitest run focus-scope badge-list` — all pass

## Workflow
- Use `/tdd` — write failing tests first, then implement to make them pass.