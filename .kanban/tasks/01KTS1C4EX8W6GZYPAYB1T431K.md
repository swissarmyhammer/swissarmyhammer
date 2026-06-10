---
assignees:
- claude-code
position_column: todo
position_ordinal: e580
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
- [ ] Determine whether the failures reproduce on a clean committed tree; if not, close with that note.
- [ ] If real: root-cause and fix; all 10 tests green in browser mode.

## Workflow
- `/tdd` for any production fix. #bug #tests