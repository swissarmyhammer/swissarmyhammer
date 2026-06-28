---
depends_on:
- 01KTEFP1RMM2G65GR1PH9YQWEZ
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffff8180
project: ui-command-cleanup
title: Card A blocker (≈ Card F2) — Kernel-authoritative focus + on-demand geometry via the F1 host→UI channel
---
## ⛔ OWNER CONSTRAINT (2026-06-06, authoritative)
**Geometry MUST be computed on demand. No caching, no partial/incremental build, no kernel-held geometry store, no push-on-mount/resize/scroll.** Owner quote: *"every time we've tried anything other than compute the geometry on demand — any kind of caching or partial build, you have failed to do it correctly."* The `getBoundingClientRect()` on-demand snapshot build in the webview **STAYS**; only *who triggers it and when* changes.

This INVALIDATES the original proposal's kernel-geometry-store mechanism (a per-window kernel store fed by React on mount/resize/scroll). Do NOT build that.

## ⚠️ CAUSES A NAV REGRESSION — see `01KTESYQ49JYJB2YT1WXYKK0W4`
The implementation of this card (host-driven nav, kernel-authoritative focus) **broke arrow-key navigation** in the working tree. AC #1 below ("kernel resolves current focus from `focus_by_window`") is the prime suspect: the frontend is the real source of truth for focus (`FRONTEND_AUTHORITATIVE_KINDS = { scope_chain }`), so `focus_by_window` goes stale (esp. on click-to-focus) and the nav op drops. The regression card proposes resolving `from` from the webview's `focus.current` (the provider already exposes it) instead of the kernel map. Reconcile that against AC #1 before marking this done. The geometry pull + lock discipline are correct and not implicated.

## Design (re-aligned 2026-06-06 to the F1/F2 approach)
This card is the **"Card F2 — focus geometry queries"** that **Card F1 `01KTEFP1RMM2G65GR1PH9YQWEZ`** (host→UI request/reply channel) is built for. It depends on F1.

The reconciliation of "nav.* server-side" + "geometry on-demand": the nav command runs **host-side**, and obtains geometry by **requesting a fresh snapshot from the webview over the F1 channel and awaiting it** — never from a cache.

1. **Focus identity → server-side.** The kernel already holds `focus_by_window: HashMap<WindowLabel, FullyQualifiedMoniker>` (`state.rs:198`). Change `navigate focus` / `drill_in layer` / `drill_out layer` to **drop the required `focused_fq` wire field** (`operations.rs:105/206/228`) and resolve current focus from `focus_by_window[window]`. **⚠️ See the regression note above — this exact choice is implicated; resolving from the webview's `focus.current` may be required instead.**
2. **Geometry → on-demand via F1, not cached, not webview-local.** When a nav op needs the `NavSnapshot`, the host issues an F1 `request_from_ui(window, "focus.geometry", {layer/focused})` and awaits the reply; the webview's F1 responder builds the snapshot ON DEMAND (`getBoundingClientRect()` at that instant via `LayerScopeRegistry.buildSnapshot`) and replies. The kernel runs `IndexedSnapshot::new(snapshot)` + `pick_target` on that fresh reply. Kernel holds NO geometry.
3. **nav.* are host-driven plugin commands** (NOT Card B webview handlers): their `execute` runs host-side, resolves `focused_fq` from the kernel, fetches the on-demand snapshot via F1, calls the op. Card A then wires the nav plugin `execute` straight to this path.
4. **Lock discipline (inherited from F1):** the host must DROP all `AppState`/spatial locks before awaiting the F1 reply, then re-acquire — see F1's deadlock rule.

## Acceptance Criteria
- [x] `navigate focus` / `drill_in layer` / `drill_out layer` no longer require `focused_fq` on the wire — current focus resolved server-side. **(Reconcile kernel `focus_by_window` vs webview `focus.current` per the regression card before closing.)** ✅ Verified at HEAD 541e85ce3: `operations.rs` Navigate/DrillIn/DrillOut carry optional `focused_fq`/`snapshot`/`window`; `server.rs::resolve_nav_source`/`resolve_drill_source` resolve wire `focused_fq` → pulled webview `focus.current` (authoritative, gated by `ui_focus_owned_by_window` root-segment ownership check) → kernel `focus_by_window` slot as last-resort fallback. This is the regression-card reconciliation, landed in efbdf3783 + 4a3a2c780.
- [x] The nav op's `NavSnapshot` is obtained per call via the F1 host→UI request/reply channel, built on-demand in the webview (`getBoundingClientRect`) — NO kernel-held geometry, NO cache, NO push model, NO webview-local handler owning the nav logic. ✅ Verified: `TauriUiGeometryProvider::pull` (apps/kanban-app/src/command_services.rs) issues `focus.geometry` over `request_from_ui` per call; the webview responder (spatial-focus-context.tsx) builds via `buildSnapshotForFocused` → `getBoundingClientRect` at request time, nothing cached; the focus kernel state holds no geometry field. Owner constraint honored.
- [x] nav.* are host-driven plugin commands whose `execute` resolves focus + fetches geometry via F1; Card A wires them to this path. ✅ Verified: builtin/plugins/nav-commands/index.ts — up/down/left/right/first/last → `navigate focus` `{window, direction}`; drillIn/drillOut → `drill_in`/`drill_out layer` `{window}` (drillOut with `moved`-flag dismiss fallthrough); window derived from the dispatch scope-chain `window:` root; focus + geometry resolved kernel-side via the provider.
- [x] No `AppState`/spatial lock is held across the F1 await (deadlock-safe). ✅ Verified: `focused_in_window` and `ui_focus_owned_by_window` each acquire+release the spatial lock internally; every provider `.await` in `resolve_nav_source`/`resolve_drill_source` runs lock-free; the `with_spatial` mutation runs only after the pull completes.
- [x] Existing spatial-nav behavior (beam search, drill, focus-lost fallback) preserved — full spatial test suite green, AND a real-path (non-fake-provider) nav test passes (see regression card). ✅ Verified 2026-06-10: `cargo nextest run -p swissarmyhammer-focus` 121/121; `-p swissarmyhammer-command-service` 106/106 — including `drill_resolves_source_via_kernel_slot_when_ui_focus_is_absent` (GapProvider: UI reports no focus, kernel slot resolves) and the two-window focus-pollution test exercising the `ui_focus_owned_by_window` guard. User live-verified nav/drill/jump/focus across multi-window configs this session.

## Depends on
- `01KTEFP1RMM2G65GR1PH9YQWEZ` — Card F1 (host→UI request/reply channel). This card rides on it.

## Quality gate
- `01KTESYQ49JYJB2YT1WXYKK0W4` — host-driven nav regression. This card is NOT done until that is resolved.

---
## (ORIGINAL PROPOSAL — kept for context; its kernel-geometry-store mechanism is SUPERSEDED)

## Why (discovered while implementing Card A 01KTCQFH7AEQDZD0QETSMCMGP0)
Card A asks the nine `nav.*` to become PLUGIN commands whose `execute` routes to the focus-server ops. Verification came back **client-side**: the nav ops cannot be driven from a plugin command with just `(window, direction)` because —
1. **`focused_fq` is a required wire field, not read from the kernel** (`operations.rs::Navigate/DrillIn/DrillOut`), even though `SpatialState` holds `focus_by_window` (state.rs:516). → fixed by "Focus identity → server-side" above.
2. **`snapshot: NavSnapshot` is live DOM geometry** built React-side by `spatial-focus-context.tsx::buildSnapshotForFocused` → `LayerScopeRegistry.buildSnapshot` via `getBoundingClientRect()` at call time; the Rust kernel has no DOM. → this is WHY geometry stays on-demand and is fetched via the F1 channel rather than cached.

#bug

## Review Findings (2026-06-10 06:55)

Reviewed at HEAD 541e85ce3 (F1 dependency 01KTEFP1RMM2G65GR1PH9YQWEZ done). Zero blockers, zero warnings — all five ACs verified and checked above with evidence.

Quality gate (01KTESYQ49JYJB2YT1WXYKK0W4): resolved in substance at HEAD — the stale-`focus_by_window`-origin bug is gone by construction (provider-first `focus.current` resolution with the `ui_focus_owned_by_window` ownership guard, commits efbdf3783 → 4a3a2c780), the fixture-only-test gap is covered by the GapProvider fallback test and the two-window pollution test (both green), and the user live-verified nav/drill/jump/focus across multi-window configs. Note: that card still sits in `doing` on the board and should be closed on its own merits.

Non-blocking doc-drift nit (no action required to close): the directional-execute comment in builtin/plugins/nav-commands/index.ts and the `Navigate` doc in operations.rs still describe focus resolution as "from `focus_by_window[window]`" — at HEAD that map is only the last-resort fallback behind the provider-first `focus.current` pull (accurately documented in server.rs::resolve_nav_source).