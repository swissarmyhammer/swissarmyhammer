---
assignees:
- claude-code
position_column: todo
position_ordinal: '8e80'
title: Replace native `title` tooltip on perspective + button with shadcn Tooltip
---
## What

The "Add perspective" button in `kanban-app/ui/src/components/perspective-tab-bar.tsx` (line 157–164) uses a native HTML `title="New perspective"` attribute for its tooltip. Every other tooltipped element in the app uses the shadcn `Tooltip` / `TooltipTrigger` / `TooltipContent` components from `@/components/ui/tooltip`. The native `title` produces an OS-styled tooltip that looks inconsistent with the rest of the UI.

**File to modify**: `kanban-app/ui/src/components/perspective-tab-bar.tsx`

**Approach**:
1. Import `Tooltip`, `TooltipTrigger`, `TooltipContent` from `@/components/ui/tooltip`
2. Wrap the `<button>` (lines 157–164) in `<Tooltip>` / `<TooltipTrigger asChild>` / `<TooltipContent>`
3. Remove the `title` attribute from the button
4. Keep the existing `aria-label` for accessibility

Follow the same pattern used in `entity-inspector.tsx` and `left-nav.tsx`.

## Acceptance Criteria
- [ ] The + button shows a shadcn-styled tooltip on hover, not a native OS tooltip
- [ ] The `title` attribute is removed from the button element
- [ ] `aria-label` remains for screen reader accessibility
- [ ] Tooltip text reads "New perspective"
- [ ] No visual regression on other perspective tab bar elements

## Tests
- [ ] Update `kanban-app/ui/src/components/perspective-tab-bar.test.tsx` — add a test that hovers the + button and asserts the shadcn tooltip content appears (role="tooltip" with text "New perspective")
- [ ] Run `pnpm vitest run perspective-tab-bar` — all tests pass