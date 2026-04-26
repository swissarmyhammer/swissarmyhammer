---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffea80
title: 'Fix: pressing : for command palette causes white screen'
---
## What

Pressing `:` (vim mode command palette trigger) causes the entire window to go white. This is a regression from the recent `window_label` removal and `backendDispatch()` centralization work.

## Root Cause

The `invoke` function from `@tauri-apps/api/core` was not imported in `command-palette.tsx`. When the palette opens, the `useEffect` on line 97 calls `invoke("list_commands_for_scope", ...)` which throws `ReferenceError: invoke is not defined`, crashing React and causing the white screen.

This happened because the `backendDispatch()` centralization replaced most direct `invoke("dispatch_command", ...)` calls, but the palette also uses `invoke` directly for two query commands (`list_commands_for_scope` and `search_entities`) that are NOT dispatch commands. When the import was removed (or never added after a refactor), those calls broke.

## Fix

Added `import { invoke } from "@tauri-apps/api/core"` to `command-palette.tsx`.

## Acceptance Criteria
- [x] Pressing `:` opens the command palette (no white screen)
- [x] Palette shows commands and is interactive
- [x] No errors in the browser console on palette open
- [x] `cargo nextest run` passes
- [x] `cd kanban-app && npx vitest run` passes (no new failures)

## Tests
- [x] `command-palette.test.tsx` — pre-existing failure due to `@/` alias resolution (not related to this fix)
- [x] `cargo nextest run` — 6946 tests pass
- [x] `cd kanban-app && npx vitest run` — no new failures (all failures are pre-existing)