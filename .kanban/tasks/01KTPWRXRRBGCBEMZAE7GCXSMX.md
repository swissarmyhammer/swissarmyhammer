---
assignees:
- claude-code
position_column: done
position_ordinal: fffffffffffffffffffffffffffffffffffffc80
project: ui-command-cleanup
title: Derive focus window from the fq root segment, not currentWindowLabel()→main
---
## What
Regression from the window-rooting commit (86887857). After rooting focus FQMs at `/<window-label>/window/...`, the implementer added an explicit `window` arg to setFocus/drill and the kernel treats it as authoritative. BUT every action sources that arg from `spatial-focus-context.tsx::currentWindowLabel()` (line 71) which uses `require("@tauri-apps/api/window")` — that THROWS in the Vite ESM bundle and returns the `"main"` fallback. So in production setFocus/drillIn/drillOut/clearFocus send `window: "main"`.

## Resolution (path-derivation, authoritative)
- Kernel: added `FullyQualifiedMoniker::root_segment()` (crates/swissarmyhammer-focus/src/types.rs) — reads the first path component; the window-rooted root IS the window label.
- `SpatialState::focus` (crates/swissarmyhammer-focus/src/state.rs) now derives the owning window from the target fq root segment, preferring it over the explicit `window` arg and the layer side field (resolution order: fq root → explicit arg → layer label). All focus-mutating ops funnel their commit through `focus` (set focus directly; navigate via `focus`; drill_in/drill_out via `focus_from` → `focus`), so the rule is uniform.
- Added `SpatialState::reconcile_slot` so navigate/drill reconcile the prior-focus slot under the path-derived window (matching the commit), making prev_fq correct so the prior marker clears.
- navigate already derived from the path and is unchanged in behavior; set/drill now behave identically.
- clear focus has no fq in its payload (only an explicit window) — it is the documented path-less caller and keeps using its explicit window. Not a hack; the diagnosis's "derive for clear focus" cannot apply where there is no path.
- Frontend: `currentWindowLabel()` switched from the broken `require(...)` to the static `import { getCurrentWindow } from "@tauri-apps/api/window"` (same reliable accessor App.tsx/views-context/perspective-context use), so it returns the real label, not "main". Window resolution no longer depends on this value (kernel derives from path), but the side channel is no longer silently wrong.

## Acceptance Criteria
- [x] Jump sets focus AND clears prior focus in the originating window (kernel: prev_fq emitted under the path-derived window; covered by set_focus_following_prior_focus_clears_prior_marker). Live verify pending user restart.
- [x] Drill-in (Enter) / drill-out (Escape) commit + emit under the path-derived window (kernel tests green). Live verify pending user restart.
- [x] Works with the SAME board in two windows, isolated (unique_window_roots_isolate_focus_* still green; no 86887857 regression).
- [x] The owning window for every focus op is derived from the window-rooted fq path, not currentWindowLabel()/layer.window_label/explicit arg.

## Tests (TDD, RED→GREEN proven)
- [x] Kernel: set focus / drill_in / drill_out with a window-rooted fq derive window from the fq ROOT and emit window_label="winA" even with a wrong explicit window="main" (extended two_window_isolation.rs). RED on prior "explicit arg wins"; GREEN after path-derivation.
- [x] Regression: a setFocus following a prior focus emits prev_fq=old (clears prior marker) in the correct window.
- [x] Unit: FullyQualifiedMoniker::root_segment (types.rs) + focus_derives_window_from_fq_root_over_clobber_and_explicit_arg (state.rs, rewritten from the old explicit-arg-wins test).
- [x] Existing fixtures updated to window-rooted shape where they asserted the window (focus_lost.rs, focus_server_e2e.rs, ui_geometry_provider.rs).

## Status
- `cargo test -p swissarmyhammer-focus`: 120 passed, 0 failed, 0 warnings.
- `npx tsc --noEmit`: clean.
- Pre-existing UI vitest failures (entity-focus.kernel-projection 5, spatial 17) confirmed identical with/without the frontend change — NOT introduced here.
- USER MUST rebuild/restart the Rust side (tauri dev) and verify live: jump-clears-prior-focus + drill-in/out, with two windows on the same board still isolated.

## Workflow
- Use `/tdd`. Do NOT run `cargo build`/`clippy` (races tauri dev). Follow-up to 86887857 / task 01KTPT83K1ZQES8CHSBF45C52M. #bug