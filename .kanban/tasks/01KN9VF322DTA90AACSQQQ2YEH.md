---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffc780
title: Fix command palette dismiss targeting wrong window — empty scope chain in closePalette
---
## What

Command palette dismiss is unreliable with multiple windows. Clicking a command or pressing Escape sometimes does nothing — the palette stays open and you have to click around to dismiss it.

### Root cause

Same pattern as the perspective.set bug: `closePalette` in `kanban-app/ui/src/components/app-shell.tsx:308` passes an empty scope chain via the old `dispatchCommand` object API:

```typescript
dispatchCommand({ id: "app.dismiss", name: "Dismiss" }, undefined, []);
```

The backend's `DismissCmd` (`swissarmyhammer-kanban/src/commands/app_commands.rs:147`) extracts the window label with `window_label_from_scope().unwrap_or("main")`. Empty scope chain → always targets "main". Secondary windows' palette never closes.

### Fix

In `kanban-app/ui/src/components/app-shell.tsx`:

1. **`closePalette` (line 308)** — replace the old `dispatchCommand` object call with `useDispatchCommand`. The hook reads scope from context automatically, so the window moniker is included without manual wiring:
   ```typescript
   const dispatch = useDispatchCommand("app.dismiss");
   // then in closePalette:
   dispatch();
   ```

2. **Also check `openPalette`** (lines 300-305) — `app.command` and `app.palette` dispatches likely have the same issue. Migrate those to `useDispatchCommand("app.command")` / `useDispatchCommand("app.palette")` respectively.

3. **Verify the palette's command execution path** — `command-palette.tsx:231` uses `useDispatchCommand()` which reads scope from context. Since the palette renders inside the `CommandScopeProvider` tree, this path should already get the window moniker. But verify.

## Acceptance Criteria
- [ ] Opening palette in window B and pressing Escape dismisses it in window B
- [ ] Clicking a command in window B's palette executes it and dismisses the palette in window B
- [ ] Window A's palette state is unaffected by window B's actions

## Tests
- [ ] Update `app-shell` or `command-palette` tests to verify scope chain includes window label on dismiss
- [ ] `pnpm test` from `kanban-app/ui/` — all pass