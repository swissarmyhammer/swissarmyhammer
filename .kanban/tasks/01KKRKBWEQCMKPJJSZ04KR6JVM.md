---
assignees:
- claude-code
depends_on:
- 01KKRKA4D87K7VM73RN6A1FV2V
- 01KKRKB5QP73FDXZKMZ8ZSHJAK
position_column: done
position_ordinal: fffffff180
title: 'Quick Capture UI: board selector + CM6 input + submit flow'
---
## What

Build the React UI for the quick-capture window. When the window is shown, it displays a compact form with a board selector and a CM6 text input. Submitting creates a task in the first column of the selected board.

**Affected files:**
- `kanban-app/ui/src/components/quick-capture.tsx` — NEW: the quick-capture form component
- `kanban-app/ui/src/App.tsx` — detect `?window=quick-capture` and render `QuickCapture` instead of the main app
- `kanban-app/ui/src/lib/quick-capture-context.tsx` — NEW (optional): lightweight context for board list + submit

**Approach:**
- In `App.tsx`, check `window.location.search` for `?window=quick-capture`. If present, render a minimal provider tree (`KeymapProvider` → `QuickCapture`) instead of the full board UI
- `QuickCapture` component:
  - Calls `invoke("list_open_boards")` to get the list of open boards
  - If only 1 board: hide the selector, auto-select it
  - If multiple: show a `<select>` dropdown (re-use the board name extraction logic from `nav-bar.tsx` line 72-78)
  - Pre-selects last-used board from `localStorage.getItem("quick-capture-last-board")`, falls back to first
  - Renders `FieldPlaceholderEditor` with `onSubmit` and `onCancel`:
    - `onSubmit(text)`: get first column of selected board (needs `invoke("get_board_data")` or a lighter query), call `invoke("dispatch_command", { cmd: "task.add", args: { column: firstColId, title: text } })`, save selected board to localStorage, hide window via `getCurrentWindow().hide()`
    - `onCancel()`: hide window via `getCurrentWindow().hide()`
  - Auto-focuses the CM6 editor when shown
  - Quick fade+scale CSS animation (~150ms) via Tailwind `animate-in`

**Getting the first column:**
- The quick-capture window needs to know the first column of the selected board. Options:
  1. Call `invoke("get_board_data")` which returns columns — works but heavy
  2. Add a lightweight `invoke("get_first_column")` Tauri command — cleaner
  3. Have the main window emit the column list to the quick-capture window via Tauri events
- Recommend option 1 initially (simplest), optimize later if needed

**Window hide/show:**
- Use `@tauri-apps/api/window` → `getCurrentWindow().hide()` to hide
- The Rust side handles showing the window on hotkey

## Acceptance Criteria
- [ ] Quick-capture window renders board selector + CM6 input
- [ ] Board selector shows open boards, pre-selects last-used (localStorage)
- [ ] Board selector hidden when only one board is open
- [ ] Enter (or vim normal-mode Enter) submits: creates task in first column, hides window
- [ ] Escape (or vim normal-mode Escape) cancels: hides window without creating task
- [ ] Selected board is remembered in localStorage for next invocation
- [ ] Window appears with a quick fade+scale animation
- [ ] CM6 editor uses the user's configured keymap (vim/CUA/emacs)

## Tests
- [ ] Unit test: QuickCapture component renders board selector when multiple boards
- [ ] Unit test: QuickCapture component hides selector when single board
- [ ] Manual test: open quick-capture, type title, press Enter → task appears in board
- [ ] Manual test: open quick-capture, press Escape → window closes, no task created
- [ ] `npm run typecheck` passes in `kanban-app/ui/`