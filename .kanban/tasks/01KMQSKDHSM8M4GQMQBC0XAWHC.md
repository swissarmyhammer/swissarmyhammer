---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffe580
title: FocusScopeInner onClick type mismatch (TS2322)
---
**Severity: Medium / Correctness**

In `focus-scope.tsx` line 116, the `onClick` handler passed to `FocusScopeInner` has type `MouseEventHandler<HTMLElement>` but the inner component's prop is typed as `(e: React.MouseEvent) => void` (which defaults to `React.MouseEvent<Element>`). TypeScript reports:

```
Type 'MouseEventHandler<HTMLElement>' is not assignable to type '(e: MouseEvent<Element, MouseEvent>) => void'.
```

This is caused by the `...rest` spread from `React.HTMLAttributes<HTMLElement>` conflicting with the explicitly typed `onClick` prop on `FocusScopeInner`.

**Fix:** Tighten the `FocusScopeInner` `onClick` prop type to `React.MouseEventHandler<HTMLElement>`, or change the inner component's generic to match `HTMLElement`:
```ts
onClick: React.MouseEventHandler<HTMLElement>;
```

**File:** `kanban-app/ui/src/components/focus-scope.tsx` #review-finding