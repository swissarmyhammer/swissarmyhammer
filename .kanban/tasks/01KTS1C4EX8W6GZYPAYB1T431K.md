---
assignees:
- claude-code
position_column: review
position_ordinal: '8280'
title: 'Pre-existing vitest browser-mode failures: focus-scope.test.tsx (9) + attachment-display "Open and Show in Finder" right-click (1)'
---
## What
While implementing 01KTCS1X9A7B5HT3H0GEDZJQ8R (context-menu ctx wiring, 2026-06-10), a baseline run against the git-HEAD versions of the touched files showed 10 vitest (browser mode, chromium) failures that PRE-DATE that card's changes — verified by restoring HEAD copies of every file the card touched and re-running: the failing set is identical before and after.

Failing set:
- `apps/kanban-app/ui/src/components/focus-scope.test.tsx` — 9 failures:
  - click sets entity focus to moniker
  - right-click sets entity focus and calls show_context_menu
  - nested FocusScope: inner click sets inner moniker
  - data-focused attribute set when focused
  - commands are provided to CommandScopeProvider
  - showFocus=false still fires context menu (handleEvents defaults true)
  - useIsFocused ancestor: column gets data-focused when card inside is focused
  - FocusScope re-renders exactly when its own moniker's focus state flips
  - spatial-context registration > click invokes spatial_focus with the primitive's key
- `apps/kanban-app/ui/src/components/fields/displays/attachment-display.test.tsx` — 1 failure:
  - AttachmentItem > shows context menu with Open and Show in Finder on right-click

Symptom shape: entity-focus assertions read `null` after a click (the synthetic `focus-changed` emission inside the mocked `invoke` doesn't reach the store), and several `waitFor`s time out. Repro: `cd apps/kanban-app/ui && npx vitest run src/components/focus-scope.test.tsx src/components/fields/displays/attachment-display.test.tsx`.

NOTE: the working tree at measurement time carried in-flight UNCOMMITTED changes in other ui files (command-scope.tsx, keybindings.ts, app-shell.tsx, jump-to-overlay.*, slide-panel.*, …). The failures may stem from that in-flight work rather than committed HEAD — first step is to re-run on a clean committed tree to decide whether this is a real regression or an artifact of the dirty tree, then bisect.

## Acceptance Criteria
- [x] Determine whether the failures reproduce on a clean committed tree; if not, close with that note.
- [x] If real: root-cause and fix; all 10 tests green in browser mode.

## Resolution (2026-06-12)
Reproduced: 10 failed / 55 passed. All 10 were stale-wire TEST expectations; production was correct — no production code changed.

Root causes:
1. **Focus emit seam moved**: the focus-scope mock emitted its synthetic `focus-changed` on `invoke("spatial_focus")`, but click/right-click now dispatch the plugin-owned `nav.focus` command whose webview bus handler commits via `command_tool_call { tool: "focus", op: "set focus", params: { fq, snapshot, window } }` (the legacy `spatial_focus` Tauri command is gone). The entity-focus store updates ONLY via the kernel `focus-changed` bridge, so the store stayed `null` → 7 failures (focus readers, data-focused, ancestor walk, selective re-render counter).
2. **`show context menu` params drift**: `openContextMenu` now sends `params: { items, window_label }` (explicit window targeting over the MCP wire); tests exact-matched `params: { items }` → 2 focus-scope waitFor timeouts + the attachment-display failure.
3. The "click invokes spatial_focus" test asserted the deleted Tauri command directly; updated to the `set focus` envelope (title now "click drives the focus server's set focus op with the primitive's key").

Fix: re-keyed the synthetic-emit invoke mock on the `set focus` envelope (with faithful `prev_fq` tracking), deduped `mockListCommands` onto it, and pinned `window_label: "main"` in the three context-menu assertions + attachment-display. Files: `apps/kanban-app/ui/src/components/focus-scope.test.tsx`, `apps/kanban-app/ui/src/components/fields/displays/attachment-display.test.tsx`.

Verified: both files in browser mode — 2 files passed, 65/65 tests passed; `npx tsc --noEmit` exit 0.

## Workflow
- `/tdd` for any production fix. #bug #tests