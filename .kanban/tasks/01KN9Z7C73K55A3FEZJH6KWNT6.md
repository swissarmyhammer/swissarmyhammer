---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffc080
title: Fix ESC dismiss on unfocused window — palette opens without keyboard focus, ESC is dead
---
## What

On one of two windows, ESC doesn't dismiss the command palette until you click the window background first. You have to click the dimmed backdrop (which gives OS focus), then ESC again.

### Root cause

The palette is a custom portal overlay (not Radix Dialog) with no focus trap. It relies on `autoFocus` on the CodeMirror editor (`command-palette.tsx:370`). When the palette opens via a UIState change propagated from the backend, the window rendering it may not have OS focus. Without OS focus, `autoFocus` is a no-op — no element receives keyboard focus. ESC is handled purely through a `document` keydown listener (`app-shell.tsx:75` → `keybindings.ts`), not a native menu accelerator. So ESC only fires in the OS-focused webview.

### Fix

In `kanban-app/ui/src/components/command-palette.tsx`, when the palette becomes visible, explicitly request OS window focus before focusing the editor:

1. **Request window focus on open** — When `open` transitions to `true`, call `getCurrentWindow().setFocus()` from `@tauri-apps/api/window`. This brings the webview to the OS foreground, making `autoFocus` work. Add this in a `useEffect` that watches `open`:
   ```typescript
   useEffect(() => {
     if (open) getCurrentWindow().setFocus();
   }, [open]);
   ```
   This should go in `command-palette.tsx` or in `app-shell.tsx` next to where `paletteOpen` is derived (line 135).

2. **Verify CM6 autoFocus still works** — After `setFocus()`, the CM6 `autoFocus` should succeed because the window now has OS focus. If it still fails, add an explicit `editorView.focus()` in a `requestAnimationFrame` after `setFocus()` resolves.

### Files to modify
- `kanban-app/ui/src/components/command-palette.tsx` — add window focus on open
- OR `kanban-app/ui/src/components/app-shell.tsx` — add window focus when `paletteOpen` becomes true

## Acceptance Criteria
- [ ] Opening palette on window B (while window A has focus) immediately accepts ESC to dismiss
- [ ] No extra click needed — ESC works on first press after palette opens
- [ ] Window A's palette behavior is unaffected

## Tests
- [ ] `kanban-app/ui/src/components/command-palette.test.tsx` — verify ESC dismisses when palette is open
- [ ] `pnpm test` from `kanban-app/ui/` — all pass