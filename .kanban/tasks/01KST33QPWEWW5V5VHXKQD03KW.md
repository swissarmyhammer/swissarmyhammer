---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffd480
title: Jump-to (`s`) overlay highlights all windows; restrict to the active window
---
RESOLVED. Root cause: spatial FQMs are not unique across Tauri windows — every window's root `<FocusLayer name="window">` is `/window` (no window-label qualifier), so a card is `/window/.../task:Z` in every window showing that board. The kernel's `emit_focus_changed` (apps/kanban-app/src/commands.rs) used the broadcast `window.emit(FOCUS_CHANGED_EVENT, …)`, which Tauri delivers to EVERY webview. Each window's claim registry matched the identically-keyed scope and lit it up — so jumping to (or focusing) any scope highlighted the same card in all windows. The React focus-changed listener deliberately did not filter by window_label (its comment wrongly assumed FQM uniqueness per window).

Fix (commands.rs): emit only to the target window — `window.emit_to(event.window_label.as_str(), FOCUS_CHANGED_EVENT, event)`. SpatialState::focus_by_window is per-window, so event.window_label is the sole correct recipient. Updated spatial-focus-context.tsx module doc to document the per-window-emit contract.

Considered + rejected a JS-side window_label filter in the provider: it required importing `@tauri-apps/api/window`, which broke tests that mock core but not window (`core.js` missing `SERIALIZE_TO_IPC_FN`). Fixing at the Rust emit site is cleaner and avoids widening the provider's import surface.

Verification:
- cargo check -p kanban-app: clean (emit_to compiles)
- vitest (spatial-nav-end-to-end, spatial-nav-jump-to, focus-on-click.regression, entity-focus-context, spatial-focus-context): 5 files / 103 tests pass
- tsc --noEmit: clean

NOTE: multi-window behavior cannot be exercised by the single-window ("main") test harness — automated tests only guard against single-window regression. Needs user verification in the real app with 2+ windows open on the same board: jumping/focusing in window A must highlight only A.

Files changed:
- apps/kanban-app/src/commands.rs (emit_to target window)
- apps/kanban-app/ui/src/lib/spatial-focus-context.tsx (doc only)

#bug #focus #spatial-nav #jump-to