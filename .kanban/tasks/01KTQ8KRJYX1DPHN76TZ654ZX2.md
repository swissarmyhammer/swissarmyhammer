---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffff9380
title: 'Stale drill wire-shape expectations: perspective-tab-bar.enter-rename test #3 and spatial-focus-context.test.tsx fail pre-existing'
---
## What

Two pre-existing vitest failures discovered while fixing 01KTQ6QZNB3VN4MAND7VPASM21 (verified to fail identically with the pre-fix `keybindings.ts`, so they are NOT caused by that card's change):

1. `apps/kanban-app/ui/src/components/perspective-tab-bar.enter-rename.spatial.test.tsx` — test #3 "Enter on a non-perspective focused leaf still dispatches spatial_drill_in" expects the webview to invoke `spatial_drill_in` / `command_tool_call(focus, drill_in layer)` with an `fq` param. After the nav-commands host-driven drill rework (drill executes in the plugin runtime with `{ window }` only; the webview just dispatches `nav.drillIn` via `dispatch_command`), no such webview IPC exists. The test should assert `dispatch_command` was called with `cmd: "nav.drillIn"` instead (its harness helper `spatialDrillInCalls()` is architecturally stale).

2. `apps/kanban-app/ui/src/lib/spatial-focus-context.test.tsx` — fails at IMPORT: its `vi.mock("@tauri-apps/api/core")` does not provide `SERIALIZE_TO_IPC_FN`, which the real `@tauri-apps/api/window` module (imported by `spatial-focus-context.tsx` for `getCurrentWindow`) needs at module-load time. Mirror the mock setup used by `spatial-focus-context.responders.test.tsx` (which also mocks `@tauri-apps/api/window`).

## Acceptance Criteria
- [ ] perspective-tab-bar.enter-rename test #3 asserts the NEW drill contract (Enter → `dispatch_command` `nav.drillIn`; no webview-side fq pre-resolution)
- [ ] spatial-focus-context.test.tsx loads and runs (window module mocked alongside core)
- [ ] Both files green under `npx vitest run`