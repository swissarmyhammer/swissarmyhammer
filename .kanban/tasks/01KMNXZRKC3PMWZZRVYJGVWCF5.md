---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffd880
title: Unify all focus mechanisms under FocusScope
---
## What

FocusScope is supposed to be the **single focus decorator** — the only component that owns focus identity, command scope registration, and visual highlighting. But there are 7 violations where focus is managed outside FocusScope, causing:

- **Focus stealing**: Board's `useLayoutEffect` calls `setFocus` after inspector's `useEffect`, so keys meant for the inspector drive the board
- **Multiple data-focused**: Board cursor (FocusHighlight) and entity focus (FocusScope) can disagree, showing two elements focused

## Violations Found

### Critical (parallel focus lifecycles — must fix)

1. **BoardFocusBridge** (`board-view.tsx:603-633`) — renderless component manually calls `registerScope` + `setFocus` in `useLayoutEffect` to sync board cursor → entity focus. Completely bypasses FocusScope.

2. **GridFocusBridge** (`grid-view.tsx:344-396`) — identical pattern to BoardFocusBridge but uses `useEffect`, making it even more susceptible to timing bugs.

3. **InspectorFocusClaimer** (`inspector-focus-bridge.tsx:122-149`) — manually calls `registerScope` + `setFocus` in `useEffect`. Loses the race to BoardFocusBridge's `useLayoutEffect`, so the board steals focus from the inspector.

### Moderate (should fix)

4. **MentionPill right-click** (`mention-pill.tsx:151,157`) — calls `setFocus(scopeMoniker)` directly in onContextMenu handler. Redundant because MentionPill is already inside a FocusScope that does the same thing via FocusScopeInner. Remove the direct call.

5. **ColumnView column header** (`column-view.tsx:208-241`) — uses `<FocusHighlight focused={focusedCardIndex === -1}>` directly. This is a second, independent highlight driven by the board cursor, not entity focus. Can show focused simultaneously with the column's FocusScope.

### Minor (acceptable with documentation)

6. **EntityInspector field rows** (`entity-inspector.tsx:214-244`) — uses `<FocusHighlight>` directly for sub-entity field navigation. Already documented as acceptable in `focus-highlight.tsx`'s docstring.

7. **BoardView background click** (`board-view.tsx:531`) — calls `setFocus(null)` to clear focus. Low risk but should be a dedicated `clearFocus` pattern.

## Approach

The core insight: the \"FocusBridge\" components exist because keyboard navigation (board cursor, grid cursor, inspector cursor) needs to sync with entity focus. They can't use FocusScope directly because FocusScope claims focus on click, not on cursor movement.

### Option A: Make FocusScope support programmatic focus

Add a `claimFocus?: boolean` prop to FocusScope. When true, FocusScope calls `setFocus(moniker)` on mount/update (like the bridges do now). This keeps all focus lifecycle in one place.

### Option B: FocusScope.Claim — a companion component

A renderless `<FocusScope.Claim moniker={...} />` that can be dropped inside a CommandScopeProvider to programmatically claim focus for that scope. Same lifecycle as FocusScope's registerScope/setFocus, but without rendering a wrapper div.

### Option C: Dedicated FocusBridge that lives inside FocusScope

Refactor the bridges to be children of FocusScope, calling `setFocus` via context rather than importing it directly.

**Recommendation**: Option A is simplest. Each bridge becomes: wrap the existing CommandScopeProvider in `<FocusScope moniker={...} claimFocus>` and delete the manual bridge component.

## Acceptance Criteria

- [ ] Only FocusScope (or a blessed variant) calls `registerScope`/`unregisterScope`/`setFocus`
- [ ] At most ONE element has `data-focused` at any time
- [ ] Opening the inspector panel takes focus from the board; keys don't leak
- [ ] Closing the inspector restores focus to the board
- [ ] Board/grid cursor navigation updates entity focus without timing races
- [ ] Column header focus is driven by entity focus, not a parallel state
- [ ] All existing focus-bridge tests pass
- [ ] `pnpm vitest run` passes

## Tests

- [ ] Opening inspector → pressing j/k only moves inspector cursor, NOT board cursor
- [ ] One `data-focused` element at all times (no doubles)
- [ ] Board cursor navigation → correct card gets `data-focused`
- [ ] Closing inspector → board regains focus"