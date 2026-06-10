---
assignees:
- claude-code
position_column: review
position_ordinal: '8780'
project: ui-command-cleanup
title: 'Unique window-rooted focus path: derive window from the full fq, stop peeking at side fields'
---
## What
Eliminate cross-window focus/nav contamination structurally. Owner directive: a UNIQUE window scope at the ROOT of the focus path, BOARD nested under it, then down to jumpable things. `window != board` (one board can be open in multiple windows). "When we nav we know where they are" — derive the target window FROM the full fq path, NOT by peeking/poking at side fields.

## IMPLEMENTATION COMPLETE — staged approach (see review notes)

### Stage 1 — kernel handles unique roots (PROVEN)
- New focus-crate e2e: `crates/swissarmyhammer-focus/tests/integration/two_window_isolation.rs`.
- `unique_window_roots_isolate_focus_across_windows` + `_for_second_window`: two layers with the SAME board sub-path but DIFFERENT window roots (`/winA/window`, `/winB/window`) register without collision; `set focus` against winA's layer emits `window_label==winA`, commits only in winA, leaves winB untouched. These PASS → the kernel already supports unique roots (registry keyed by fq, no collision). The prior breakage was a FRONTEND push-fq vs snapshot-layer_fq mismatch, NOT a kernel limit.
- `set_focus_honors_explicit_window_over_layer_side_field`: RED-first. Added `window: Option<WindowLabel>` to the `set focus` Focus op (`operations.rs`) and threaded it in `handle_focus` (`server.rs`) → explicit window wins over a clobbered shared-`/window` layer side field. GREEN.

### Stage 2 — frontend unique root + explicit window (no new test failures)
- `App.tsx`: module-scope `const WINDOW_ROOT_FQ = fqRoot(asSegment(getCurrentWindow().label))`; both `<FocusLayer name="window">` (App + QuickCaptureApp) now `parentLayerFq={WINDOW_ROOT_FQ}` → layer fq `/<label>/window`, board nested below. Identity-stable (computed ONCE at module load), never inline — the exact mistake that broke it twice before.
- push fq == snapshot layer_fq == registry key PROVEN by construction: all three derive from the single FocusLayer `fq` memo (focus-layer.tsx) — published to descendants, used as the `LayerScopeRegistry(fq)` key, pushed via `pushLayer(fq)`, and stamped into `buildSnapshot(layerFq)` as `snapshot.layer_fq`. New automated guard: `layer-scope-registry.test.tsx` › "window-rooted <FocusLayer> identity" (2 tests, RED-verified: fail on the bare `/window` root, pass when window-rooted to `/main/window`).
- `focus-mcp.ts`: `setFocus`/`navigateFocus`/`drillIn`/`drillOut` now send explicit `window`; `spatial-focus-context.tsx` passes `currentWindowLabel()` from `focus`/`navigate`/`drillIn`/`drillOut`/`popLayer`. Window is derived from the explicit arg / full path, never the side field.
- `jump-to-overlay.tsx`: early-boot fallback fixed to the window-rooted shape `/<label>/window` (module-scope `WINDOW_LAYER_FALLBACK_FQ`).
- Stale workaround comments refreshed: `state.rs`, `command_services.rs`, `spatial-focus-context.tsx`.

### Verification
- Full UI vitest baseline diff: identical failing SET (125 fail / 129 FAIL lines) with and without my changes — ZERO new failures; +2 new passing tests. focus crate: all green (68 unit + 23 focus_server incl. 3 new isolation tests). `tsc --noEmit` clean. `cargo fmt` applied.
- Per the OPERATIONAL RULES: NO `cargo build`/`clippy` run. The blast-radius estimate of "~30 literal-/window assertions across 3 files" was wrong (100 across 20 files) — but nearly all are in tests that build their OWN `<FocusLayer name="window">` WITHOUT `parentLayerFq`, so they still produce `/window` and stay green; production composes from context (no `/window` literals), so they did not need changing.

### USER ACTION REQUIRED
- Rust changed (operations.rs/server.rs/state.rs/command_services.rs) → rebuild + restart `tauri dev` to pick up the new binary.
- Live TWO-WINDOW verification is required (cannot be done headless): open the same board in two windows, focus/navigate a card in A, confirm B's marker + nav do NOT move (and vice versa); confirm single-window focus/nav/drill/jump still work.

## Confirmed bug + root cause (explorer, evidence-backed)
- Two windows on the SAME board contaminate: focusing/navigating a card in window A lights up the same card's marker AND moves nav in window B. (Jump-to does NOT contaminate — it enumerates each window's own React `layerRegistriesRef`, which is tree-local.)
- Root mechanism: the window-root `<FocusLayer name="window">` composes the LITERAL `/window` (parent=null → `fqRoot("window")`). The kernel `SpatialRegistry.layers` is a `HashMap` keyed by fq only. Both windows push key `/window` → second push OVERWRITES the first; the surviving entry carries the LAST window's `window_label`. `handle_focus` passed `window: None`, so `SpatialState::focus` resolved the window via the clobbered layer side field → `FocusChangedEvent.window_label` wrong → `emit_to(window_label,…)` targets the wrong window.

## Acceptance Criteria
- [x] Two windows on the SAME board: focusing/navigating in window A does NOT move focus or markers in window B (and vice versa). (kernel-proven via two_window_isolation; live two-window check pending user)
- [x] Focus FQMs are window-unique by construction (rooted at the window label); board is a nested segment, not the root. window != board.
- [x] The window for focus/nav/drill is derived from the full path / explicit `window` arg, not from `layer.window_label` or a `window:None` fallback.
- [x] No regression: jump still targets one window; inspector/palette/drill still work. (zero new UI test failures)

## Tests (automated, TDD)
- [x] Kernel: two layers with the SAME board sub-path but DIFFERENT window roots register without collision; focus in window A's layer emits window A's label and does not affect window B (two_window_isolation.rs).
- [x] `set focus` with an explicit `window` resolves that window without relying on the layer side field.
- [x] push fq == snapshot layer_fq == registry key automated guard (layer-scope-registry.test.tsx, RED-verified).
- [x] A regression test that fails on the shared-`/window` root and passes with the unique root (the two window-rooted identity tests).

## Workflow
- Used `/tdd`. Structural follow-through on the reverted window-rooting; done identity-stably and verified push fq == snapshot layer_fq == registry key. Relates to nav-regression 01KTESYQ49JYJB2YT1WXYKK0W4 and the drill task 01KTPDTH772HSEV5F7R1DKYDNJ. #bug