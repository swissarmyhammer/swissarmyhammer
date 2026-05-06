---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffa180
project: spatial-nav
title: 'focus-indicator: show dashed border on perspective tabs and board name field'
---
## Bug

The dashed-border focus indicator does not render on perspective tabs or on the board name field, but it does render on cards, columns, and inspector fields. The user wants visual focus parity across these scopes.

## Root cause

Not a plumbing bug — the indicator is intentionally suppressed at three sites via hardcoded `showFocusBar={false}`:

- `kanban-app/ui/src/components/perspective-tab-bar.tsx::PerspectiveTabFocusable` (~line 490)
- `kanban-app/ui/src/components/perspective-tab-bar.tsx::PerspectiveBarSpatialZone` (~line 317)
- `kanban-app/ui/src/components/board-selector.tsx` (`<Field>` callsite renders the board name; `<Field>` defaults `showFocusBar = false` and the callsite doesn't override)
- `kanban-app/ui/src/components/nav-bar.tsx` (`ui:navbar` zone and `ui:navbar.board-selector` zone — both `showFocusBar={false}`)

Existing tests pin the no-indicator behavior — see Tests below.

## Fix

1. **`perspective-tab-bar.tsx::PerspectiveTabFocusable`** — drop `showFocusBar={false}` (or set it to `true`) so the per-tab wrapper paints the indicator when focused.
2. **`board-selector.tsx`** — pass `showFocusBar` to the `<Field>` rendering the board name.
3. **Re-evaluate the navbar zones** (`ui:navbar`, `ui:navbar.board-selector`) — these are container zones, not user-targeted scopes. Probably leave their `showFocusBar={false}` alone unless there's evidence the indicator on the navbar zone itself is wanted. If unsure, leave navbar zones as-is and only flip the leaf surfaces (perspective tabs + board name field).
4. **Re-evaluate `PerspectiveBarSpatialZone`** — this is the bar container; the per-tab wrappers are inside it. Leave the bar zone alone for now. Only flip per-tab.

## Tests to update

These currently assert `expect(queryByTestId("focus-indicator")).toBeNull()` on focused perspective tabs / focused board-name field — invert their assertions:

- `kanban-app/ui/src/components/perspective-tab-bar.focus-indicator.browser.test.tsx`
- `kanban-app/ui/src/components/focus-on-click.regression.spatial.test.tsx` — guards `"perspective tab wrapper opts out of the visible bar"` etc.
- `kanban-app/ui/src/components/nav-bar.spatial-nav.test.tsx` (board-selector path, if applicable to the field-leaf vs zone distinction)

## New tests to add

- Positive browser test: focus a perspective tab → assert `[data-testid="focus-indicator"]` is present inside the focused `perspective_tab:{id}` wrapper.
- Positive browser test: focus the board name field → assert `[data-testid="focus-indicator"]` is present inside the focused field's host box.
- Pattern after the existing `column-view.spatial.test.tsx` indicator coverage.

## Acceptance criteria

- Dashed-border focus indicator renders on the focused perspective tab.
- Dashed-border focus indicator renders on the focused board name field.
- Other surfaces unchanged (cards, columns, inspector fields keep current indicator).
- `cargo test -p swissarmyhammer-focus` and `pnpm -C kanban-app/ui test` green.

## Files

- `kanban-app/ui/src/components/perspective-tab-bar.tsx`
- `kanban-app/ui/src/components/board-selector.tsx`
- `kanban-app/ui/src/components/perspective-tab-bar.focus-indicator.browser.test.tsx`
- `kanban-app/ui/src/components/focus-on-click.regression.spatial.test.tsx`
- `kanban-app/ui/src/components/nav-bar.spatial-nav.test.tsx`