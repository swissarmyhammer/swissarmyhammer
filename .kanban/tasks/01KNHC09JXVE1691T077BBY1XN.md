---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffff9880
title: Center loading spinner in full viewport using h-screen instead of flex-1
---
## What

The loading spinner in `BoardContainer` (`kanban-app/ui/src/components/board-container.tsx`, line 99-104) uses `flex-1` to fill available space, but its ancestors during loading (`CommandScopeProvider`, `AppModeContainer`) are context wrappers with no flex layout — so `flex-1` doesn't expand and the spinner isn't centered in the viewport.

The fix: use `h-screen w-screen` (or `h-dvh w-dvw`) instead of `flex-1` so the spinner fills the entire viewport regardless of parent layout.

### File to modify

**`kanban-app/ui/src/components/board-container.tsx`** — line 101:

Change:
```tsx
<main role="status" className="flex-1 flex items-center justify-center">
```

To:
```tsx
<main role="status" className="h-screen flex items-center justify-center">
```

Also apply the same fix to the "No board loaded" placeholder (line 110):
```tsx
<main className="flex-1 flex items-center justify-center">
```
→
```tsx
<main className="h-screen flex items-center justify-center">
```

## Acceptance Criteria

- [ ] Loading spinner is visually centered horizontally and vertically in the full window
- [ ] "No board loaded" placeholder is also centered in the full window
- [ ] Once board loads, normal layout (NavBar + content) renders correctly
- [ ] No layout shift when transitioning from loading → loaded

## Tests

- [ ] `kanban-app/ui/src/components/board-container.test.tsx` — existing `renders loading spinner when loading is true` test still passes
- [ ] `kanban-app/ui/src/components/board-container.test.tsx` — verify spinner container has `h-screen` class
- [ ] Run `npm test` in kanban-app — all pass

## Workflow

- Use `/tdd` — write failing tests first, then implement to make them pass.