---
assignees:
- claude-code
position_column: done
position_ordinal: fffffffffffffffffffffff480
project: spatial-nav
title: 'Multi-window follow-up: frontend must listen for focus-changed on current webview, not app-wide'
---
## What

Now that `focus-changed` is emitted per-window via `window.emit_to(window.label(), …)` (see 01KPNXYZZJ99N9CGRRBX5ZD1GA), frontend listeners registered via `@tauri-apps/api/event`'s `listen()` (which uses `target: { kind: "Any" }`) will still fire in every window due to Tauri's `match_any_or_filter` shortcut — `Any` listeners fire on all emits regardless of target.

The single production listener lives in `kanban-app/ui/src/lib/entity-focus-context.tsx` (~line 271, `useFocusChangedEffect`). It imports `listen` from `@tauri-apps/api/event`. To actually get per-window isolation at the UI level, this must change to:

```ts
import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
// …
const window = getCurrentWebviewWindow();
const unlisten = window.listen<{ prev_key: string | null; next_key: string | null }>(
  "focus-changed",
  (event) => { /* … */ },
);
```

`getCurrentWebviewWindow().listen()` registers with `target: EventTarget::WebviewWindow { label }`, which only matches emits scoped to that window.

## Acceptance Criteria

- `entity-focus-context.tsx` uses `getCurrentWebviewWindow().listen()` rather than the app-wide `listen()` for `focus-changed`.
- Existing frontend tests still pass (the test shim in `kanban-app/ui/src/test/setup-spatial-shim.ts` may need a parallel update to mock the webview listen API).
- Manual: two kanban windows open; `h/j/k/l` in window A visibly moves focus in window A and never in window B.

## Tests

- Update `kanban-app/ui/src/lib/entity-focus-context.test.tsx` to exercise the webview-scoped listen path (the current test at line 487-488 asserts on `listen("focus-changed", …)` — port to assert on `getCurrentWebviewWindow().listen(…)`).
- Vitest suite must stay green.

## Context

This is the UI half of the fix landed by 01KPNXYZZJ99N9CGRRBX5ZD1GA (which refactored `AppState.spatial_state` → per-window map and switched emits to `emit_to(window.label(), …)`). Without this follow-up the Rust side is correctly scoped but the UI still pays cross-window rendering costs because `Any` listeners fire in every window.