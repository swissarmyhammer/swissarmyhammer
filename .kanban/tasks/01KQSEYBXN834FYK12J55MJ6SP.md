---
assignees:
- claude-code
position_column: todo
position_ordinal: d480
project: spatial-nav
title: Toolbar field focus indicators are invisible (board.name and percent_complete)
---
## What

Two `<Field>` instances in the navbar/toolbar register as focus zones, register correctly with the kernel, and CAN receive spatial focus — but their `<FocusIndicator>` is never visible to the user.

### Reproduce

1. Spatially focus the toolbar's board-name field (e.g. via geometric nav from a perspective tab) — the IPC log shows `cmd=ui.setFocus result={"ScopeChain":["field:board:board.name","engine"]}` — but no visible indicator paints anywhere on screen.
2. Same for the navbar's percent-complete progress field — focus lands, no visible indicator.

### Root cause (board.name)

`kanban-app/ui/src/components/board-selector.tsx:91-107`:

```tsx
<div className={`flex items-center gap-1.5 min-w-0 ${className ?? ""}`}>
  <div className="font-semibold truncate min-w-0 flex-1">
    {boardEntity && nameFieldDef ? (
      <Field fieldDef={nameFieldDef} entityType="board" ... />
    ) : (
      <span className="text-sm cursor-text truncate">{displayName}</span>
    )}
  </div>
```

`truncate` includes `overflow: hidden`. The `<Field>`'s focus indicator is positioned at `-left-2 w-1` (an 8px-wide bar 8px outside the field's left edge — the project-wide convention, see `nav-bar.tsx:36-51` for the layout contract). That position is OUTSIDE the wrapping div's box, so `overflow: hidden` clips it.

### Root cause (percent_complete) — investigate during implementation

`kanban-app/ui/src/components/nav-bar.tsx:139-147` renders the percent Field directly inside the navbar's `<FocusZone moniker="ui:navbar">` with no wrapping div. The navbar outer has no `overflow: hidden` (`flex h-12 items-center border-b px-4 gap-2`), so naively the indicator should show. But it doesn't.

Likely candidates:
- The `progress-ring` field display (custom visual rendering, see `kanban-app/ui/src/components/fields/registrations/progress-ring.tsx`) wraps its content in a way that clips or replaces the indicator host.
- The `mode="compact"` Field render path differs from the inspector's `mode="full"` path in a way that affects indicator paint position.
- Some sibling element (the inspect-button `<Pressable>` to its left, or the search wrapper to its right) sits in the indicator's `-left-2` slot and obscures it.
- The percent Field has `showFocusBar` defaulting to false (per `field.tsx`'s `showFocusBar = false` default for non-inspector callers), so the visible indicator never even mounts.

The third option is the most likely after a quick read of `field.tsx`'s prop defaults — `showFocusBar` defaults to false and the navbar caller doesn't override it. Verify during implementation.

### Fix shape

For board.name: drop `truncate` from the wrapping div in `board-selector.tsx:92`. Apply `truncate` (or `text-overflow: ellipsis`) to an inner element that doesn't host the indicator. Alternatively, give the wrapper enough left padding (`pl-2` or more) so the `-left-2` indicator falls inside the clip box.

For percent_complete: pass `showFocusBar` explicitly on the navbar's `<Field>` instance (`board-selector.tsx`'s name field already implicitly works because of how it's rendered — verify by greppping).

Both share the underlying contract: any Field that participates in spatial nav must show a visible indicator when focused, OR be excluded from spatial nav (not the choice here per user direction).

### Files to modify

- `kanban-app/ui/src/components/board-selector.tsx` — fix the `truncate`/`overflow:hidden` clipping on the wrapping div for the name field.
- `kanban-app/ui/src/components/nav-bar.tsx` — confirm `showFocusBar` is enabled on the percent_complete Field, OR fix whatever else is suppressing the indicator.
- `kanban-app/ui/src/components/fields/field.tsx` — possibly: review the default value of `showFocusBar` and whether navbar/toolbar Fields should opt in explicitly. If the default needs flipping, flip it and audit existing callers.

## Acceptance Criteria

- [ ] With spatial focus on the toolbar's board-name field, a `<FocusIndicator>` is visible to the user (test asserts the indicator element is in the DOM, has non-zero rendered width and height, and is not clipped — use `getBoundingClientRect()` on the indicator vs. its nearest `overflow: hidden` ancestor).
- [ ] Same for the navbar's percent_complete field.
- [ ] No regression: long board names still ellipsize correctly in the toolbar; the navbar layout still fits within `h-12`; perspective tabs and other navbar leaves continue to show their indicators.
- [ ] `pnpm -C kanban-app/ui test` passes (with new regression tests below).
- [ ] `pnpm -C kanban-app/ui typecheck` passes.

## Tests

- [ ] Add a regression test in `kanban-app/ui/src/components/board-selector.focus-indicator.browser.test.tsx` (new file): mount `<NavBar>` (or `<BoardSelector>`) inside the spatial provider stack, drive focus to the board-name field's FQM via simulated `focus-changed` event, and assert `[data-testid="focus-indicator"]` is present inside the field's DOM AND its `getBoundingClientRect()` returns non-zero width/height AND its left edge is >= the wrapping clip ancestor's left edge (no clip).
- [ ] Add a parallel test in `kanban-app/ui/src/components/nav-bar.percent-focus-indicator.browser.test.tsx` (new file) for the percent_complete field. Same assertion shape.
- [ ] Long-name regression test: with a 200-character board name, mount the BoardSelector and assert the name still truncates (text overflow ellipsis is applied to the inner span/element) AND the focus indicator is still visible.
- [ ] Run `pnpm -C kanban-app/ui test board-selector.focus-indicator nav-bar.percent-focus-indicator` and confirm green.

## Workflow

- Use `/tdd`. Write the indicator-visibility regressions first (they will fail against current code), then fix the wrapper / `showFocusBar` issues until green. Confirm long-name truncation still works in the same test file.
