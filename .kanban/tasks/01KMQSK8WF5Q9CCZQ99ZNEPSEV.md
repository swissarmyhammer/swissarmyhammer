---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffba80
title: UseInspectorNavReturn interface missing pill navigation properties (TS build broken)
---
**Severity: High / Correctness**

`UseInspectorNavReturn` in `use-inspector-nav.ts` (lines 9-22) does not declare `pillIndex`, `pillCount`, `setPillCount`, `movePillLeft`, or `movePillRight`, but the hook returns all five. This causes 30+ TypeScript errors (TS2339) in both the test file and in `inspector-focus-bridge.tsx` (lines 90, 99) where `navRef.current?.movePillLeft()` and `movePillRight()` are called through the typed ref.

The runtime tests pass because Vitest ignores type errors, but `tsc --noEmit` fails. This blocks CI type-checking.

**Fix:** Add the five missing members to the `UseInspectorNavReturn` interface:
```ts
pillIndex: number;
pillCount: number;
setPillCount: (n: number) => void;
movePillLeft: () => void;
movePillRight: () => void;
```

**File:** `kanban-app/ui/src/hooks/use-inspector-nav.ts` #review-finding