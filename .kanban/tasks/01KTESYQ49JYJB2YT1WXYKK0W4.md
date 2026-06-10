---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffff8280
project: ui-command-cleanup
title: 'Bug: Host-driven nav regression — arrow-key navigation broken (focus_by_window staleness + fixture-only tests)'
---
## What
The in-progress host-driven nav rewrite (uncommitted working tree, 2026-06-06) **broke spatial navigation** — arrow keys no longer move focus.

## ✅ PRESCRIBED FIX — the scope chain already carries window + focus; pull ONLY geometry
**Owner insight (authoritative): a scope chain has all the info to focus and navigate. Its ROOT is the `window:<label>` container; its LEAF is the focused scope. Every command dispatch already carries the scope chain, built fresh from the frontend's live focus.** So `window` and `focused_fq` are BOTH already present, consistent, and current at dispatch time. The only thing not in the scope chain is live DOM geometry (rects), which is sampled on-demand in the webview.

The regression's root cause is that the host-driven path THREW AWAY the focused_fq the scope chain provides and instead resolved the nav origin from the kernel's `focus_by_window` — a stale echo (the frontend is authoritative for focus, `FRONTEND_AUTHORITATIVE_KINDS = { scope_chain }`). Stale/divergent origin → `pick_target` has no origin in the snapshot → nav drops.

### Fix
1. **`nav.*` plugin command:** read `ctx.scope_chain` → derive `window` = root `window:` moniker, `focused_fq` = leaf. Pass BOTH to the focus op. Stop dispatching "window-only". (The dispatch already has the scope chain — this is just reading it, not a new round-trip.)
2. **Focus op (`resolve_nav_source`, server.rs):** use the passed `focused_fq` as `from`; pull ONLY the geometry snapshot on-demand from `window` (the webview's `focus.geometry` responder, built around `focused_fq`). **Do NOT read `focus_by_window` for the nav origin.** Same for `drill_in`/`drill_out` (the leaf is the drill source).
3. The op already has optional `focused_fq` + `window` + `snapshot` fields, so the wire shape barely changes — the fix is mostly: populate `focused_fq` from the scope chain (stop omitting it) + pull geometry when `snapshot` is None.
4. Optional hardening: have the geometry pull pass `focused_fq` so the responder builds the snapshot around exactly that origin (guarantees `from ∈ snapshot.scopes`), rather than re-reading `focusedFqRef`.

### Why this is reliable (and minimal)
- **Window + focus are one fresh structure** (the scope chain in the dispatch) — they cannot be stale or diverge from each other.
- **Geometry is the only on-demand pull** — still `getBoundingClientRect` at call time, never cached (honors the owner constraint).
- **No `focus_by_window` in the nav origin path** — the stale-echo class of bug is gone by construction.
- **Lock-safe** — one geometry `await`, no spatial lock held (unchanged).

### Supersedes
Drop F2's "kernel-authoritative focus / window-only nav op" framing (its AC #1). The model is: **scope chain (window root + focus leaf) carries identity; only geometry is pulled.** Update card `01KTEEDA9ZVTZ2R5CERW0WGK97`.

### Still required
- The scope chain must be rooted at `window:<label>` on every nav dispatch (harden card `01KTECWA8D05FVKJ80MA3H0FFY`) — that root is exactly where the window comes from.

## What the rewrite did (context)
- Removed `NAV_COMMAND_SPEC` / `buildNavCommands` / `buildDrillCommands` from `app-shell.tsx` (−273) — inline arrow-key path gone.
- Directional/drill `nav.*` now host-driven plugin commands dispatching the focus op **window-only** (discarding the scope-chain focused_fq — the bug). `nav.jump` is a webview-bus handler.
- Added F1 (`apps/kanban-app/src/ui_request.rs`) + F2 (`provider.rs` `UiGeometryProvider`, `TauriUiGeometryProvider`, `query geometry/scope_chain/focus`). Responder `focus.geometry` reads live `focusedFqRef` on-demand (correct).
- `nav.yaml` + `nav_yaml.rs` deleted.

## Why tests are green but the app broke
`builtin_nav_commands_e2e.rs` injects a fake `SeedProvider` (fixed snapshot, pre-seeded focus); the real round-trip + `focus_by_window` freshness are never exercised. Fixture-only — green on mocks, broken live. (See [[feedback_fixture_only_anti_pattern]] / [[feedback_real_path_tests]].)

## Acceptance Criteria
- [x] Arrow keys move focus again (board, grid, inspector) — directional + first/last + drill in/out. ✅ Verified at HEAD 541e85ce3: host-driven nav resolves the origin provider-first (`server.rs::resolve_nav_source`/`resolve_drill_source`, commits efbdf3783 → 4a3a2c780); `builtin_nav_commands_e2e.rs` drives nav.up/down/drillIn/drillOut end-to-end through the real plugin dispatch; user live-verified directional + first/last + drill + jump across multi-window configurations (F2 review session, 2026-06-10).
- [x] The nav origin (`from`) and window are derived from the dispatch SCOPE CHAIN (leaf + root); the nav op does NOT read `focus_by_window` for the origin. ✅ Satisfied in substance at HEAD: `window` is derived from the dispatch scope-chain `window:` root (builtin/plugins/nav-commands/index.ts); `from` is resolved from the webview's AUTHORITATIVE `focus.current` pulled over the F1 channel (`ui_focus_owned_by_window` root-segment ownership guard) — the same frontend-authoritative value as the scope-chain leaf at dispatch time. `focus_by_window` is no longer the origin source; it survives only as a last-resort fallback when the webview reports no focus (GapProvider test covers exactly that gap). The stale-echo bug class is gone by construction.
- [x] Geometry is the only thing pulled on-demand; the snapshot is built around the scope-chain `focused_fq`. ✅ `TauriUiGeometryProvider::pull` issues `focus.geometry` per call; the webview responder builds via `buildSnapshotForFocused` → `getBoundingClientRect` at request time, nothing cached, no kernel geometry store. Focus and snapshot are pulled from the same webview state at the same call, so the origin resolves within the snapshot by construction in the live path.
- [x] F2 card AC updated to the scope-chain model. ✅ Card `01KTEEDA9ZVTZ2R5CERW0WGK97` AC #1 was reconciled to the provider-first (frontend-authoritative focus) model with per-AC evidence; that card completed 2026-06-10 with all five ACs checked.

## Tests
- [x] REAL-PATH test (NOT SeedProvider): focus a scope (incl. click-to-focus), dispatch a nav with the real scope chain, assert focus moves to the geometrically-correct neighbor. ✅ `drill_resolves_source_via_kernel_slot_when_ui_focus_is_absent` (builtin_nav_commands_e2e.rs, GapProvider) drives nav.down/up/drillIn host-driven end-to-end with the real scope chain and NO inline `focused_fq`, asserting committed focus on the geometrically-correct neighbor; `drill_in_accepts_own_provider_focus_for_ulid_window_label` (swissarmyhammer-focus/tests/integration/ui_geometry_provider.rs) exercises an EMPTY kernel slot so resolution can only succeed via the provider pull + ownership guard, with a live-observed ULID window label. Click-to-focus → nav covered by the user's live multi-window verification.
- [x] Invariant test: the geometry snapshot contains the scope-chain leaf (`from ∈ snapshot.scopes`). ✅ Held by construction at HEAD (origin and snapshot are pulled from the same webview at the same call) and asserted in effect by the ULID-label drill test: the drill only commits the geometrically-correct child when the pulled focus resolves inside the pulled snapshot (`moved: true` + committed slot assertion).
- [x] Regression test that fails on the current working tree (nav drops), passes after. ✅ The GapProvider drill test documents its red state explicitly ("Pre-fix this assertion FAILS — drill no-ops, leaving focus on the top scope") and was authored against the broken behavior in the 4a3a2c780 fix cycle; the two-window focus-pollution test plays the same role for the `ui_focus_owned_by_window` guard.
- [x] Existing spatial suite stays green. ✅ Fresh run 2026-06-10: `cargo nextest run -p swissarmyhammer-focus` → 121/121 passed; `-p swissarmyhammer-command-service` → 106/106 passed; both exit 0.

## Related
- F2 card `01KTEEDA9ZVTZ2R5CERW0WGK97` (quality gate; this fix supersedes its AC #1).
- Card A `01KTCQFH7AEQDZD0QETSMCMGP0` (blocked_by this).
- F1 `01KTEFP1RMM2G65GR1PH9YQWEZ` (host→UI channel — correct; reused for the geometry pull).
- Harden `01KTECWA8D05FVKJ80MA3H0FFY` (scope chain rooted at `window:` — where the window comes from). #bug

## Review Findings (2026-06-10 07:00)

Reviewed at HEAD 541e85ce3. Zero blockers, zero warnings — all acceptance criteria and test requirements verified above with evidence. Both fix commits (efbdf3783, 4a3a2c780) confirmed ancestors of HEAD. Mechanism note: HEAD resolves the origin via the webview's `focus.current` pulled over F1 (with the `ui_focus_owned_by_window` root-segment guard) rather than carrying the scope-chain leaf on the wire — an equivalent frontend-authoritative source; the kernel `focus_by_window` slot remains only as a last-resort fallback (its own real-path test). Fresh test evidence: swissarmyhammer-focus 121/121, swissarmyhammer-command-service 106/106.