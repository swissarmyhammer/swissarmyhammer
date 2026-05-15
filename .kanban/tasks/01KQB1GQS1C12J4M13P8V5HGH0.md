---
assignees:
- claude-code
depends_on: []
position_column: done
position_ordinal: fffffffffffffffffffffffffffffffff380
project: spatial-nav
title: 'Make the navbar keyboard-navigable: register `ui:navbar` zone and its leaves with the spatial kernel in production'
---
## What

Restore keyboard navigation, focus indicators, and spatial-nav debug overlays to the navbar. The navbar (`kanban-app/ui/src/components/nav-bar.tsx`) is **not** registering with the spatial-nav kernel in the running app. The user observation: with `<FocusDebugProvider enabled>` in `App.tsx:72`, no dashed border appears around the navbar zone or its three button scopes, no focus indicator shows when a navbar leaf is clicked, and arrow keys do not traverse the bar.

A non-registered zone produces all four symptoms together because the Rust kernel never learns the navbar exists, so beam search ignores it, focus-claim subscriptions on its keys never fire (`data-focused` never flips), `<FocusIndicator>` never mounts, and `<FocusDebugOverlay>` never paints. The four symptoms are one bug.

## REOPENED 2026-05-01 — first attempt produced ZERO production code changes and the bug is STILL present

A first pass on this card concluded "no production bug to fix" because the new `nav-bar.production-tree.browser.test.tsx` mounts `<App />` in vitest browser mode and reports the spatial branch renders correctly. **The user manually verified the running `cargo tauri dev` app and the bug is still there**: no debug overlays on the navbar, no focus indicator, no keyboard nav. The test is giving false confidence — production behavior diverges from the test.

## Investigation results — 2026-05-01

Adding `tracing::info!` to `register_zone` / `register_scope` / `unregister_scope` in `swissarmyhammer-focus/src/registry.rs` and reading oslog showed the kernel-side state is **correct after the StrictMode dance**. The `cargo tauri dev` app's three observed startup runs all produced the same log pattern:

1. **Mount-1 (StrictMode first mount):** `register_zone(/window/ui:navbar)` + 3 `register_scope` calls for navbar leaves + 2 `register_zone` calls for the percent-complete and board-name field zones. All `re_register=false`.
2. **Cleanup-1 (StrictMode cleanup):** All 6 entries `unregister_scope` with `was_present=true`.
3. **Mount-2 (StrictMode second mount):** Same 6 registrations re-fire, all `re_register=false`.

After mount-2, no further unregister fired for the navbar tree, and querying the kernel registry confirms `/window/ui:navbar` is registered as a zone and each of the three leaf scopes is present. **Hypothesis 1 (StrictMode race undoes registration) is disproved.**

The `parent_zone` and `layer_fq` fields on every navbar registration are correct (`parent_zone=None` for the navbar zone itself; `parent_zone=/window/ui:navbar` for the leaves). **Hypothesis 3 (spatial branch never runs) is also disproved** — if the React side were dropping into the fallback branch, no `register_*` IPC would fire.

Existing vitest tests assert the navbar's host div carries `[data-debug="zone"]` when `<FocusDebugProvider enabled>` — they pass. So the React JSX renders the overlay element. The bug must therefore be **CSS-side suppression of the rendered overlay**.

## Root cause

The inspector backdrop in `kanban-app/ui/src/components/inspectors-container.tsx` was permanently in the DOM with `fixed inset-0 z-20 opacity-0 pointer-events-none` even when no panel was open, so the `transition-opacity` could play on subsequent mounts. Even though the backdrop was visually transparent, **`position: fixed` + numeric `z-index` always creates a stacking context** (per CSS spec), and the always-mounted z-20 layer covering the entire viewport sat above the navbar's window-layer focus-debug overlays at z-15 (set by `7143e0bfc`'s layer-aware z-index table).

In the Tauri WebKit webview, the always-on z-20 transparent stacking context suppressed sibling overlays at lower z-indices in the closed-inspector state — exactly the user-observed symptom (no dashed border, no focus indicator, no visible focus claim). The vitest browser project does not exercise this seam because it does not run inside WebKit and does not test computed visibility.

## Fix

`kanban-app/ui/src/components/inspectors-container.tsx`: Conditionally mount the backdrop only when `hasPanels` is true. The fade-in on open is preserved by `transition-opacity` plus the initial render at `opacity-100`; the fade-out on close is intentionally dropped (the SlidePanel's slide-out animation is the user-visible signal; the dim-backdrop fade-out was masked by the panel's transition and was never load-bearing).

Diagnostic tracing on `register_zone` / `register_scope` / `unregister_scope` in `swissarmyhammer-focus/src/registry.rs` is left in place — it is load-bearing for diagnosing exactly this class of bug (kernel registry state vs. observed UI behavior).

## Acceptance Criteria

- [x] Empirical evidence in oslog confirms `register_zone` for `ui:navbar` and `register_scope` for each of the three leaves fires successfully on app startup, with no subsequent `unregister_scope` that leaves the navbar absent. (Verified via `log show --predicate 'subsystem == "com.swissarmyhammer.kanban"'` after diagnostic tracing was added.)
- [ ] In `cargo tauri dev` (the running app), clicking any navbar button moves focus to it: a 4px-wide vertical cursor-bar appears in the gap to the LEFT of the focused button. **Requires user manual verification.**
- [ ] In `cargo tauri dev`, ArrowRight / ArrowLeft traverse navbar leaves. ArrowDown moves focus out of the navbar. **Requires user manual verification.**
- [ ] In `cargo tauri dev` with `<FocusDebugProvider enabled>` (already in App.tsx:72), the navbar host `<div>` shows a blue dashed `[data-debug="zone"]` border, and each of the three `<FocusScope>` leaves shows an emerald dashed `[data-debug="scope"]` border. **Requires user manual verification.**
- [x] All existing tests pass except 6 pre-existing failures unrelated to this card (3 in `entity-inspector.field-enter-drill`, 2 in `board-view.enter-drill-in`, 1 in `inspectors-container.test.tsx > opening a second panel does not push another inspector layer`). Verified by stashing changes and re-running on main.

## #blocker #frontend #spatial-nav #kanban-app