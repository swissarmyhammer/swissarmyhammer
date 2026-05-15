---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffc580
title: Resizable inspector — drag the left edge to change width (persisted per window)
---
## What

Inspector panels are currently fixed at 420 px wide. Make the left edge draggable so the user can resize the inspector, with the chosen width persisted per-window through the existing `WindowState` UI-state pipeline.

### Current shape

- `kanban-app/ui/src/components/slide-panel.tsx` — fixed `w-[420px] max-w-[85vw]` Tailwind class on the outer `<div>`. Renders the close button + scroll body. Knows nothing about its position in the stack.
- `kanban-app/ui/src/components/inspectors-container.tsx`
  - `const PANEL_WIDTH = 420;` (module constant)
  - `const rightOffset = (panelStack.length - 1 - index) * PANEL_WIDTH;` — each panel in the stack is offset by one panel width so adjacent panels tile horizontally.
- `swissarmyhammer-commands/src/ui_state.rs::WindowState` — per-window persisted state. Already carries `inspector_stack`, `active_view_id`, `x/y/width/height`, `maximized`. Mutators broadcast a `UIStateChange::*` variant the frontend listens for.

### Decisions (assume these unless changed)

- **Single width per window**, applied to every panel in the stack. Adjacent panels must keep tiling — they share the same width so the right-edge offset math stays valid.
- **Per-window persistence** via `WindowState.inspector_width: Option<u32>`. `None` falls back to the current 420 px default.
- **Bounds**: clamp to `[320, min(800, 0.85 * window.innerWidth)]`. The lower bound keeps the form readable; the upper bound preserves the existing `max-w-[85vw]` intent.
- **Drag handle**: a 6 px-wide invisible hit zone on the panel's left edge with `cursor-col-resize`, becoming visibly subtle on hover (the same muted-foreground stroke used by the column-resize handle in `data-table.tsx`, if applicable — otherwise a hairline `bg-border` indicator).
- **Persistence cadence**: update local React state on every mousemove (60 fps drag), but only dispatch the backend `ui.inspector.set_width` command on mouseup, mirroring the column-resize / window-geometry pattern.

### Files to change

1. `kanban-app/ui/src/components/slide-panel.tsx` — add an optional `width` prop and a left-edge drag handle. Replace `w-[420px]` with an inline `style={{ width }}` (still capped via CSS `maxWidth: '85vw'`). Drag emits `onResize(nextWidth)` continuously and `onResizeEnd(finalWidth)` once.
2. `kanban-app/ui/src/components/inspectors-container.tsx` — read `inspector_width` from `useUIState()` (with the `?? 420` fallback), pass it to every `<InspectorPanel>`, and use it in the `rightOffset` calculation. Wire `onResizeEnd` to a new `useDispatchCommand("ui.inspector.set_width")` callback. The transient drag state stays in React; only the final value round-trips through the backend.
3. `swissarmyhammer-commands/src/ui_state.rs` —
   - Add `inspector_width: Option<u32>` to `WindowState` (with `Default` returning `None`).
   - Add `set_inspector_width(&self, window_label: &str, width: u32) -> Option<UIStateChange>` and a getter `inspector_width(&self, window_label: &str) -> Option<u32>`.
   - Add a `UIStateChange::InspectorWidth { window_label, width: u32 }` variant; update the `ui_state_change_kind` mapping.
4. `swissarmyhammer-kanban/src/commands/ui_commands.rs` (or wherever `ui.inspector.open/close` live, mirror that file) — add `ui.inspector.set_width` command that calls `set_inspector_width` and broadcasts the change.
5. `kanban-app/src/commands.rs` — register the new variant in `ui_state_change_kind` (look for the existing `InspectorStack` branch around line 1870) so the React side hears `inspector_width` change events.

### Type plumbing

- Frontend: `UIWindowState` type (in `kanban-app/ui/src/lib/ui-state-context.tsx` or `types/kanban.ts` — find via grep) gets an optional `inspector_width?: number`. The `useUIState` selector exposes it; `InspectorsContainer` reads it as `winState?.inspector_width ?? 420`.

## Acceptance Criteria

- [x] Hovering the left edge of any inspector shows `cursor-col-resize`.
- [x] Mousedown + drag on the left edge changes the inspector width in real time, clamped to `[320, min(800, 0.85 * viewport)]`.
- [x] When two panels are stacked, dragging either one's left edge resizes both (single shared width per window) and they remain tiled — no overlap, no gap.
- [x] After a resize, the new width is dispatched once via `ui.inspector.set_width` on mouseup. Reloading the window restores the saved width.
- [x] When `WindowState.inspector_width` is `None`, the panel renders at the existing 420 px default.
- [x] Existing inspector snapshot/spatial-nav tests still pass — the resize handle is a visual+pointer addition only, no change to focus behavior or `<FocusZone>` structure.

## Tests

- [x] **Drag interaction (browser test)** in a new `kanban-app/ui/src/components/inspector-resize.browser.test.tsx`: mount `<InspectorsContainer>` with one panel open, simulate `mousedown` on the left-edge handle, `mousemove` shifting -120 px (wider) and `mouseup`. Assert (a) the panel's `style.width` after mousemove is the new value, (b) `ui.inspector.set_width` was dispatched once on mouseup with the final value.
- [x] **Clamp test (unit)** in `slide-panel.test.tsx` (create if absent): a drag that would compute < 320 px is clamped to 320 px; a drag that would exceed `min(800, 0.85*viewport)` is clamped at the upper bound.
- [x] **Stack offset test** in `kanban-app/ui/src/components/inspectors-container.test.tsx`: when two panels are open and `inspector_width` is 600, panel 0 has `right: 600` and panel 1 has `right: 0` (tiled with the new width, not 420).
- [x] **Backend round-trip** in `swissarmyhammer-commands/src/ui_state.rs` (next to the existing `set_inspector_stack_restores` test): `set_inspector_width("main", 540)` then `inspector_width("main")` returns `Some(540)`; serialize to YAML and reload, value survives.
- [x] Run `pnpm -C kanban-app/ui test inspector-resize slide-panel inspectors-container` and `cargo test -p swissarmyhammer-commands ui_state` — all green.

## Workflow

- Use `/tdd` — start with the backend `set_inspector_width` round-trip test, then the inspectors-container offset assertion, then the frontend drag-interaction test. Implement until all three are green.

## Review Findings (2026-05-09 17:10)

### Warnings
- [x] `kanban-app/ui/src/components/slide-panel.tsx` (`handleMouseDown` / `endDrag`) — Tap-without-movement (mousedown → mouseup with zero `mousemove`) on a never-resized panel still calls `onResizeEnd(width)` with the React default 420. `inspectors-container.tsx::handleResizeEnd` then dispatches `ui.inspector.set_width { width: 420 }`, which flips `WindowState.inspector_width` from `None` to `Some(420)` and writes to disk. Backend's `set_inspector_width` only no-ops when the new width *equals* the existing value, so `None → Some(420)` is treated as a real change and persists. A "tap with no drag" should not mutate persisted state. Suggested fix: track a `moved` flag on `dragRef` set by `handleMouseMove`, and skip `onResizeEnd` in `endDrag` when `!moved`.
- [x] `swissarmyhammer-kanban/src/commands/ui_commands.rs::InspectorSetWidthCmd` — No unit tests. Compare with `SetAppModeCmd` which has three (`set_app_mode_changes_ui_state`, `set_app_mode_noop_returns_null`, `set_app_mode_uses_window_from_scope`). Missing coverage means the negative-`width` path (which falls into `MissingArg` because `serde_json::Value::as_u64()` returns `None` for negatives), the out-of-range-`u32` path, the no-op echo path (`set_inspector_width` returning `None` on equal value), and the window-label-from-scope path are all uncovered. Suggested fix: add a `tests` block in `ui_commands.rs` next to the `SetAppMode` tests pinning each path.
- [x] `kanban-app/ui/src/components/slide-panel.tsx` (`useEffect` binding `mousemove`/`mouseup` on `window`) — listeners are bound for the entire lifetime of every mounted `SlidePanel`, regardless of whether a drag is active. With multiple panels stacked (the spec calls for stacking) every panel installs its own pair, and every pointer movement anywhere on the window walks all of them (the early-return on `!drag.active` is cheap but still runs n times per move event). This works, but contradicts the "wire mouse listeners only while dragging" pattern used elsewhere in the codebase. Suggested fix: bind the listeners inside `handleMouseDown` (after setting `dragRef.current`) and remove them in `endDrag`. The body of `handleMouseDown` already has the moment to do this; `endDrag` already has the cleanup point.
- [x] `kanban-app/ui/src/components/inspectors-container.tsx` (`useEffect(() => setLiveWidth(null), [persistedWidth])`) — if a `ui-state-changed` `inspector_width` echo arrives mid-drag (e.g. another window in the same session persisted a different width through some future code path, or a stale event from a prior drag races back), the effect resets `liveWidth` to `null` and the panel snaps to the new persisted value while the drag is still in progress. The drag continues correctly (drag-ref logic in `slide-panel.tsx` is untouched), but the visual width flickers. Today there is no realistic way to trigger this — the window only updates its own `inspector_width` and dispatch happens on mouseup — but the contract is fragile. Suggested fix: gate the reset effect with a "not currently dragging" predicate (e.g. track `isDragging` and skip the reset while it's true), or move the persisted/live reconciliation into `slide-panel.tsx` so the drag path owns the source of truth end-to-end.

### Nits
- [x] `swissarmyhammer-kanban/src/commands/ui_commands.rs::InspectorSetWidthCmd::execute` — When `width` is a negative integer, `v.as_u64()` returns `None` and the user sees `MissingArg("width")`. The error label is misleading: the arg is present, just not coercible. Suggested fix: use `v.as_i64()` plus an explicit range check, or two separate `ok_or_else` arms — one for "missing", one for "not an integer / out of range".
- [x] `swissarmyhammer-kanban/src/commands/ui_commands.rs::InspectorSetWidthCmd` — There is no lower-bound check in the command (the `[320, …]` clamp lives only in the frontend). A direct dispatch with `width: 1` from the palette/CLI/test harness is silently persisted and reloaded as a 1 px panel (which `slide-panel.tsx`'s `style.maxWidth: "85vw"` does not protect against on the lower end — the component just renders 1 px wide). Suggested fix: clamp or reject in `InspectorSetWidthCmd::execute` so the command itself enforces the contract documented in the task spec.
- [x] `kanban-app/ui/src/components/inspectors-container.tsx::InspectorPanel` (line citing `useFirstFieldFocus`) — the docstring contains escaped unicode literals (`—` for em-dash) where surrounding code uses real em-dashes. Suggested fix: replace `—` with `—` to match the rest of the file.